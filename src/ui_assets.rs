//! 前端资源（`dist/`）的生成。
//!
//! 这段逻辑本来只写在 `build.rs` 里，但那样有个坑：
//! **`tauri build` 在编译之前就会检查 `frontendDist` 指向的目录是否存在**，
//! 而 `dist/` 是编译期产物 —— 干净检出（比如 CI）上根本没有这个目录，
//! `tauri build` 会直接报 “Unable to find your web assets” 退出，连编译都不开始。
//!
//! 所以这里提供一份**独立的**实现（`ui_assets` 模块，不属于 build script），
//! 由 `meow-text --export-ui` 调用，在 `tauri build` 之前先把 `dist/` 铺好。
//! `build.rs` 里保留了一份等价逻辑：build script 不能依赖主 crate，
//! 而 `include!` 共享源码又会踩到 clippy 的 `duplicate_mod`。
//! 两边的一致性由测试 `dist_matches_ui_assets_output` 兜住。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 生成产物、读文件失败时给人的报错（调用方决定是 panic 还是打印）。
pub type Result<T> = std::result::Result<T, String>;

/// `dist/` 里不参与占位符替换的生成文件（它们自己就是文案表）。
const GENERATED: [&str; 1] = ["locale.js"];

/// 一种语言的文案表：`a.b.c` -> 文案。
pub type TextTable = BTreeMap<String, String>;

/// 一种语言：`(标签, 文案表)`。
pub type Locale = (String, TextTable);

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

    let mut locales: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for (tag, path) in files {
        let text = std::fs::read_to_string(&path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
        let value: toml::Value = toml::from_str(&text).map_err(|err| format!("解析 {tag} 失败：{err}"))?;
        let mut table = BTreeMap::new();
        flatten(&value, "", &mut table);
        locales.push((tag, table));
    }

    let keys: Vec<String> = locales[0].1.keys().cloned().collect();
    Ok((keys, locales))
}

/// 判断该用哪个语言包当「首屏默认」：`MEOW_LOCALE` > 语言前缀匹配 > `zh-CN` > 第一个。
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

fn flatten(value: &toml::Value, prefix: &str, out: &mut BTreeMap<String, String>) {
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
        let mut table = BTreeMap::new();
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

    /// `--export-ui` 生成的 dist/ 必须和仓库里那份一致，
    /// 否则 CI 上 tauri build 用的资源就和本地 `cargo build` 的不是一回事了。
    #[test]
    fn dist_matches_ui_assets_output() {
        let root = root();
        let dist = root.join("dist");
        if !dist.exists() {
            return; // 还没构建过，跳过
        }

        let (keys, locales) = load_locales(&root.join("locales")).expect("读语言包失败");
        let default = pick_default(locales.iter().map(|(tag, _)| tag.as_str()), "zh-CN").expect("挑默认语言失败");

        // 渲染到临时目录再逐个文件比对
        let scratch = std::env::temp_dir().join("meow-dist-check");
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        copy_dir(&root.join("ui"), &scratch.join("ui"));
        export(&scratch, &default, &keys, &locales).expect("导出失败");

        let mut compared = 0;
        for entry in std::fs::read_dir(&dist).unwrap().flatten() {
            let name = entry.file_name();
            let produced = scratch.join("dist").join(&name);
            assert!(produced.exists(), "dist/{name:?} 没有被 --export-ui 生成");
            let expected = std::fs::read_to_string(entry.path()).unwrap();
            let actual = std::fs::read_to_string(&produced).unwrap();
            assert_eq!(expected, actual, "dist/{name:?} 与 --export-ui 的输出不一致");
            compared += 1;
        }
        assert!(compared > 0, "dist/ 里一个文件都没有");

        let _ = std::fs::remove_dir_all(&scratch);
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
