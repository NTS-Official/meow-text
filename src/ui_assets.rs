//! 前端资源（`dist/`）和文案表（`gen/i18n.rs`）的生成。
//!
//! 这段逻辑本来只写在 `build.rs` 里，但那样有个坑：
//! **`tauri build` 在编译之前就会检查 `frontendDist` 指向的目录是否存在**，
//! 而 `dist/` 是编译期产物 —— 干净检出（比如 CI）上根本没有这个目录，
//! `tauri build` 会直接报 “Unable to find your web assets” 退出，连编译都不开始。
//!
//! 所以这里提供一份**独立的**实现（`ui_assets` 模块，不属于 build script），
//! 由 `meow-text --export-ui` 调用，在 `tauri build` 之前把两份产物铺好。
//! `build.rs` 里是等价的第一版实现：build script 不能依赖主 crate，
//! 而 `include!` 共享源码又会踩到 clippy 的 `duplicate_mod`。
//! 两边的一致性由测试 `build_artifacts_match_sources` 兜住。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 生成产物、读文件失败时给人的报错（调用方决定是 panic 还是打印）。
pub type Result<T> = std::result::Result<T, String>;

/// 一种语言的文案表：`a.b.c` -> 文案。
pub type TextTable = BTreeMap<String, String>;

/// 一种语言：`(标签, 文案表)`。
pub type Locale = (String, TextTable);

/// `ui/` 里不需要参与占位符替换的文件（它本身就是生成物）。
const GENERATED: [&str; 1] = ["locale.js"];

/// 把 `ui/` 按默认语言渲染进 `dist/`，并写出 `dist/locale.js`。
pub fn export(root: &Path, build_default: &str, keys: &[String], locales: &[Locale]) -> Result<()> {
    let ui_dir = root.join("ui");
    let dist_dir = root.join("dist");

    if dist_dir.exists() {
        std::fs::remove_dir_all(&dist_dir).map_err(|err| format!("清理 {} 失败：{err}", dist_dir.display()))?;
    }
    std::fs::create_dir_all(&dist_dir).map_err(|err| format!("创建 {} 失败：{err}", dist_dir.display()))?;

    // 首屏按「本次编译的默认语言」静态替换；运行时切语言由 locale.js 负责
    let default_table = locales
        .iter()
        .find(|(tag, _)| tag == build_default)
        .map(|(_, table)| table)
        .or_else(|| locales.first().map(|(_, table)| table))
        .ok_or_else(|| "一个语言包都没有".to_string())?;

    let mut missing: BTreeSet<String> = BTreeSet::new();
    let entries = std::fs::read_dir(&ui_dir).map_err(|err| format!("读取 {} 失败：{err}", ui_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if GENERATED.contains(&name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
        let rendered = substitute(&text, default_table, name, &mut missing);
        std::fs::write(dist_dir.join(name), rendered).map_err(|err| format!("写入 {name} 失败：{err}"))?;
    }

    if !missing.is_empty() {
        return Err(format!(
            "ui/ 里有 {} 个文案键在 locales/{build_default}.toml 中不存在：\n  {}",
            missing.len(),
            missing.into_iter().collect::<Vec<_>>().join("\n  ")
        ));
    }

    std::fs::write(dist_dir.join("locale.js"), locale_js(build_default, keys, locales))
        .map_err(|err| format!("写入 locale.js 失败：{err}"))?;
    Ok(())
}

/// 把 `{{ key }}` 替换成文案；替换不了的记进 missing。
fn substitute(text: &str, table: &TextTable, file: &str, missing: &mut BTreeSet<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = after[..end].trim();
        match table.get(key) {
            Some(value) => out.push_str(value),
            None => {
                missing.insert(format!("{file}: {key}"));
                out.push_str(&rest[start..start + 2 + end + 2]);
            }
        }
        rest = &after[end + 2..];
    }

    out.push_str(rest);
    out
}

/// 前端用的多语言表：全部语言 + 默认语言。切语言就是换一张表重画，不再重新加载页面。
fn locale_js(build_default: &str, keys: &[String], locales: &[Locale]) -> String {
    let mut list: Vec<serde_json::Value> = Vec::new();
    for (tag, table) in locales {
        let mut strings = serde_json::Map::new();
        for key in keys {
            strings.insert(
                key.clone(),
                serde_json::Value::String(table.get(key).cloned().unwrap_or_default()),
            );
        }
        list.push(serde_json::json!({
            "tag": tag,
            "name": table.get("locale.name").cloned().unwrap_or_else(|| tag.clone()),
            "strings": serde_json::Value::Object(strings),
        }));
    }

    let json = match serde_json::to_string_pretty(&serde_json::Value::Array(list)) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("[meow] 序列化语言包失败：{err}");
            "[]".to_string()
        }
    };

    format!(
        "// 由 build.rs 从 locales/*.toml 生成，请勿手改。\nwindow.MEOW_DEFAULT_LOCALE = {build_default:?};\nwindow.MEOW_LOCALES = {json};\n"
    )
}

/// 生成 `gen/i18n.rs`：`Locale` / `KEYS` / `LOCALES` + 每个键一个 `Key` 常量。
///
/// 产出目录就放在 `src/` 的兄弟目录 `gen/`，这样 rust-analyzer 能直接索引到，
/// 也不用再跟 `OUT_DIR` 那种神奇路径打交道。
pub fn write_rust_module(path: &Path, build_default: &str, keys: &[String], locales: &[Locale]) -> Result<()> {
    let mut out = String::new();
    out.push_str("// 由 `meow-text --export-ui`（或 build.rs）从 locales/*.toml 生成，请勿手改。\n\n");

    out.push_str("/// 本次编译的语言包列表。\n");
    out.push_str("pub struct Locale {\n");
    out.push_str("    pub tag: &'static str,\n");
    out.push_str("    /// 该语言的自称（任何语言下都显示这个）\n");
    out.push_str("    pub name: &'static str,\n");
    out.push_str("    /// 与 `KEYS` 一一对应的文案\n");
    out.push_str("    pub strings: &'static [&'static str],\n");
    out.push_str("}\n\n");

    out.push_str("/// 全部文案键，索引即 `Key` 的内层值。\n");
    out.push_str("pub const KEYS: &[&str] = &[\n");
    for key in keys {
        out.push_str(&format!("    {key:?},\n"));
    }
    out.push_str("];\n\n");

    out.push_str(&format!(
        "/// 编译时的默认语言（首屏用它，找不到配置时也用它）。\npub const DEFAULT_LOCALE: &str = {build_default:?};\n\n"
    ));

    out.push_str("pub static LOCALES: &[Locale] = &[\n");
    for (tag, table) in locales {
        let name = table.get("locale.name").map(String::as_str).unwrap_or(tag.as_str());
        out.push_str("    Locale {\n");
        out.push_str(&format!("        tag: {tag:?},\n"));
        out.push_str(&format!("        name: {name:?},\n"));
        out.push_str("        strings: &[\n");
        for key in keys {
            let value = table.get(key).ok_or_else(|| format!("{tag} 缺少文案键 {key}"))?;
            out.push_str(&format!("            {value:?},\n"));
        }
        out.push_str("        ],\n    },\n");
    }
    out.push_str("];\n\n");

    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for (index, key) in keys.iter().enumerate() {
        let name = const_name(key);
        if let Some(previous) = owners.insert(name.clone(), key.clone()) {
            return Err(format!(
                "文案键 {previous} 与 {key} 生成的常量名都是 {name}，请改掉其中一个键名"
            ));
        }
        out.push_str(&format!("/// `{key}`\npub const {name}: Key = Key({index});\n"));
    }

    std::fs::write(path, out).map_err(|err| format!("写入 {} 失败：{err}", path.display()))
}

/// 文案键 -> Rust 常量名：整条键路径大写、点换成下划线。
///
/// `status.hook_mounted` -> `STATUS_HOOK_MOUNTED`，`scenario.qq.name` -> `SCENARIO_QQ_NAME`。
/// 用整条路径而不是只取最后一段，是因为最后一段会撞车：`close.hide` /
/// `close_action.hide` / `footer.hide` 都想叫 `HIDE`。名字长一点，但没特例、不会撞。
pub fn const_name(key: &str) -> String {
    key.replace('.', "_").to_ascii_uppercase()
}

/// 读 `locales/*.toml`，返回 `(全部键, 各语言表)`；键序按 tag 排序、键按字典序，保证可复现。
pub fn load_locales(locales_dir: &Path) -> Result<(Vec<String>, Vec<Locale>)> {
    let entries =
        std::fs::read_dir(locales_dir).map_err(|err| format!("读取 {} 失败：{err}", locales_dir.display()))?;

    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let Some(tag) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        files.push((tag.to_string(), path));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        return Err(format!("{} 下没有找到任何 xx-YY.toml 语言包", locales_dir.display()));
    }

    let mut locales: Vec<Locale> = Vec::new();
    for (tag, path) in files {
        let text = std::fs::read_to_string(&path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
        let value: toml::Value = toml::from_str(&text).map_err(|err| format!("解析 {tag} 失败：{err}"))?;
        let mut table = TextTable::new();
        flatten(&value, "", &mut table);
        locales.push((tag, table));
    }

    let keys: Vec<String> = locales[0].1.keys().cloned().collect();
    Ok((keys, locales))
}

/// 判断该用哪个语言包当「首屏默认」：精确匹配 > 语言前缀匹配 > `zh-CN` > 第一个。
///
/// 和 `build.rs` 里的 `pick_locale_tag` 同义（build script 不能依赖主 crate，
/// 所以各留一份；行为不一致时黄金文件测试会拦下来）。
pub fn pick_default<'a>(tags: impl Iterator<Item = &'a str>, hint: &str) -> Option<String> {
    let tags: Vec<&str> = tags.collect();
    let hint = hint.trim();
    if !hint.is_empty() {
        if let Some(exact) = tags.iter().find(|tag| tag.eq_ignore_ascii_case(hint)) {
            return Some((*exact).to_string());
        }
        let lower = hint.to_ascii_lowercase();
        let prefix = lower.split(['-', '_']).next().unwrap_or("");
        if !prefix.is_empty()
            && let Some(matched) = tags.iter().find(|tag| tag.to_ascii_lowercase().starts_with(prefix))
        {
            return Some((*matched).to_string());
        }
    }
    if let Some(fallback) = tags.iter().find(|tag| tag.eq_ignore_ascii_case("zh-CN")) {
        return Some((*fallback).to_string());
    }
    tags.first().map(|tag| (*tag).to_string())
}

fn flatten(value: &toml::Value, prefix: &str, out: &mut TextTable) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let full = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(child, &full, out);
            }
        }
        toml::Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn substitute_replaces_known_keys_and_reports_missing() {
        let mut table = TextTable::new();
        table.insert("a.b".to_string(), "喵".to_string());
        let mut missing = BTreeSet::new();

        let out = substitute("<p>{{a.b}}</p><p>{{ nope }}</p>", &table, "index.html", &mut missing);
        assert_eq!(out, "<p>喵</p><p>{{ nope }}</p>");
        assert_eq!(missing.len(), 1);
        assert!(missing.iter().next().unwrap().starts_with("index.html"));
    }

    #[test]
    fn pick_default_prefers_exact_then_prefix_then_fallback() {
        let tags = ["en-US", "zh-CN", "ja-JP"];
        assert_eq!(pick_default(tags.into_iter(), "en-US").as_deref(), Some("en-US"));
        assert_eq!(pick_default(tags.into_iter(), "EN-us").as_deref(), Some("en-US"));
        assert_eq!(pick_default(tags.into_iter(), "ja").as_deref(), Some("ja-JP"));
        // 不认识的提示 -> 回落 zh-CN
        assert_eq!(pick_default(tags.into_iter(), "kl-GL").as_deref(), Some("zh-CN"));
        assert_eq!(pick_default(tags.into_iter(), "").as_deref(), Some("zh-CN"));
    }

    /// 常量名不能再撞车：之前只取最后一段，`close.hide` 和 `close_action.hide` 都成了 `HIDE`。
    #[test]
    fn const_names_are_unique_and_valid_identifiers() {
        assert_eq!(const_name("close.hide"), "CLOSE_HIDE");
        assert_eq!(const_name("close_action.hide"), "CLOSE_ACTION_HIDE");
        assert_eq!(const_name("footer.hide"), "FOOTER_HIDE");
        assert_eq!(const_name("scenario.qq.name"), "SCENARIO_QQ_NAME");
        assert_eq!(const_name("title"), "TITLE"); // 只带一段的键

        let (keys, _) = load_locales(&root().join("locales")).expect("读语言包失败");
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for key in &keys {
            let name = const_name(key);
            // 生成的是 Rust 标识符，字母数字下划线以外的东西一个都不能有
            assert!(
                !name.is_empty()
                    && !name.starts_with(|c: char| c.is_ascii_digit())
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{key} 生成的 `{name}` 不是合法的 Rust 标识符"
            );
            if let Some(previous) = seen.insert(name.clone(), key.clone()) {
                panic!("{previous} 与 {key} 都会生成 {name}");
            }
        }
    }

    /// 仓库里的 `dist/`（`cargo build` 铺的）和 `gen/i18n.rs` 必须等于
    /// 从当前源码重新生成的结果。
    ///
    /// 为什么要这条：`tauri build` 用的是 `--export-ui` 铺出来的 `dist/`，
    /// 而程序里的文案表来自 `gen/i18n.rs` —— 两份产物要是对不上，
    /// 就会出现「界面上的静态文字」和「运行时切语言的结果」来自不同源码的局面。
    /// 改了 `ui/` 或 `locales/` 却忘了重新生成时，这条测试会直接告诉你
    /// （顺手 `cargo build` 一次也会重新生成）。
    ///
    /// 比对时统一换行符：Windows 上 git 会把文本文件检成 CRLF，行尾差异不该算过期。
    #[test]
    fn build_artifacts_match_sources() {
        let root = root();

        let (keys, locales) = load_locales(&root.join("locales")).expect("读语言包失败");
        // 用编译时的默认语言，和 build.rs 挑的那个保持一致
        let default = pick_default(
            locales.iter().map(|(tag, _)| tag.as_str()),
            option_env!("MEOW_UI_DEFAULT_LOCALE").unwrap_or("zh-CN"),
        )
        .expect("挑默认语言失败");

        // 生成到临时目录（把 ui/ 一起拷过去，export 是从 <root>/ui 读的）
        let scratch = std::env::temp_dir().join("meow-export-ui-check");
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        copy_dir(&root.join("ui"), &scratch.join("ui"));
        export(&scratch, &default, &keys, &locales).expect("导出失败");

        // dist/：ui/ 里的每个文件 + 生成出来的 locale.js
        let mut names: Vec<String> = std::fs::read_dir(root.join("ui"))
            .expect("读不到 ui/")
            .flatten()
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        names.push("locale.js".to_string());
        names.sort();

        for name in &names {
            let committed = root.join("dist").join(name);
            let produced = scratch.join("dist").join(name);
            assert!(produced.exists(), "dist/{name} 没有被生成");
            assert!(
                committed.exists(),
                "dist/{name} 不存在 —— 先跑一次 `cargo run -- --export-ui`"
            );
            let expected = std::fs::read_to_string(&committed).unwrap();
            let actual = std::fs::read_to_string(&produced).unwrap();
            assert_eq!(
                normalize(&expected),
                normalize(&actual),
                "dist/{name} 和当前 ui/{name} 对不上 —— 跑一次 `cargo run -- --export-ui`"
            );
        }

        // gen/i18n.rs：Rust 侧那张文案表
        let i18n_committed = root.join("gen").join("i18n.rs");
        if i18n_committed.exists() {
            let produced = scratch.join("i18n.rs");
            write_rust_module(&produced, &default, &keys, &locales).expect("生成 i18n.rs 失败");
            let expected = std::fs::read_to_string(&i18n_committed).unwrap();
            let actual = std::fs::read_to_string(&produced).unwrap();
            assert_eq!(
                normalize(&expected),
                normalize(&actual),
                "gen/i18n.rs 和当前 locales/ 对不上 —— 跑一次 `cargo run -- --export-ui`"
            );
        }

        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// CRLF/CR -> LF：只用于比对，不影响落盘内容。
    fn normalize(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let path = entry.path();
            let target = to.join(entry.file_name());
            if path.is_dir() {
                copy_dir(&path, &target);
            } else {
                std::fs::copy(&path, &target).unwrap();
            }
        }
    }
}
