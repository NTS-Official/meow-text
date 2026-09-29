//! 构建期本地化 + Tauri 构建脚本。
//!
//! 现在**所有**语言包都会编译进产物，运行时可切换（见 `src/i18n.rs`）：
//!   1. 读 `locales/*.toml` 全部语言包，拍平成 `a.b.c` 这样的键，并强制各语言键集合一致
//!   2. 生成 `$OUT_DIR/i18n.rs`：键表 + 全部语言 + 每个键一个 `Key` 常量（写错键名编译不过）
//!   3. 生成 `dist/locale.js`：整张多语言表交给前端，前端切语言不用重新加载
//!   4. `ui/` 里的 `{{key}}` 按「本次编译的默认语言」替换，保证首屏就是对的
//!
//! 默认语言（首屏 + 兜底）：`MEOW_LOCALE` 环境变量 > 系统界面语言 > 回落 zh-CN。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 找不到匹配语言时用它。
const FALLBACK_LOCALE: &str = "zh-CN";

fn main() {
    println!("cargo:rerun-if-changed=locales");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-env-changed=MEOW_LOCALE");

    let root = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env("OUT_DIR"));

    let files = discover_locales(&root.join("locales"));
    assert!(!files.is_empty(), "locales/ 下没有找到任何 xx-YY.toml 语言包");

    // 全部语言包：tag -> (键 -> 文案)
    let mut locales: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for (tag, path) in &files {
        locales.push((tag.clone(), load_locale(path)));
    }
    check_same_keys(&locales);

    let keys: Vec<String> = locales[0].1.keys().cloned().collect();

    let build_default = {
        let override_locale = std::env::var("MEOW_LOCALE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let hint = match &override_locale {
            Some(value) => value.trim().to_string(),
            None => system_language(),
        };
        let picked = pick_locale(&files, &hint);
        println!(
            "cargo:info=meow-text 本地化：{} 种语言（{} 条文案），首屏默认 {picked}（来源：{}）",
            locales.len(),
            keys.len(),
            match &override_locale {
                Some(_) => "MEOW_LOCALE".to_string(),
                None if hint.is_empty() => "默认".to_string(),
                None => format!("系统语言 {hint}"),
            }
        );
        picked
    };

    let src_text = read_all(&root.join("src"));
    let ui_text = read_all(&root.join("ui"));
    warn_unused_keys(&keys, &src_text, &ui_text);

    write_rust_module(&out_dir.join("i18n.rs"), &build_default, &keys, &locales);
    write_web_assets(&root.join("ui"), &root.join("dist"), &build_default, &keys, &locales);

    // 让 `meow-text --export-ui` 用上同一个默认语言：CI 的 windows runner 是英文系统，
    // 不告诉它一声的话，静态首屏会被烘成英文，而程序自己的兜底语言还是 zh-CN。
    println!("cargo:rustc-env=MEOW_UI_DEFAULT_LOCALE={build_default}");

    // 这两处是**第一次**生成 i18n.rs 和 dist/ 的地方；之后由 `--export-ui` 负责保持同步，
    // 测试 `ui_assets::tests::build_artifacts_match_sources` 会在它们过期时拦下来。
    tauri_build::build();
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("缺少环境变量 {name}"))
}

/* -------------------------------------------------------------- 语言包 */

fn discover_locales(dir: &Path) -> BTreeMap<String, PathBuf> {
    let mut found = BTreeMap::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|err| panic!("读取 {} 失败：{err}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            found.insert(stem.to_string(), path);
        }
    }
    found
}

/// 系统界面语言：Windows 直接问 GetUserDefaultUILanguage，其它平台看 LANG。
#[cfg(windows)]
fn system_language() -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    let langid = unsafe { GetUserDefaultUILanguage() } as u32;
    language_prefix(langid & 0x03ff).to_string()
}

#[cfg(not(windows))]
fn system_language() -> String {
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .map(|value| {
            value
                .split(['.', '@', '_'])
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase()
        })
        .unwrap_or_default()
}

/// 主语言 ID -> 语言前缀（只映射常见的几种）。
fn language_prefix(primary: u32) -> &'static str {
    match primary {
        0x04 => "zh",
        0x07 => "de",
        0x09 => "en",
        0x0a => "es",
        0x0c => "fr",
        0x10 => "it",
        0x11 => "ja",
        0x12 => "ko",
        0x16 => "pt",
        0x19 => "ru",
        _ => "",
    }
}

fn pick_locale(files: &BTreeMap<String, PathBuf>, hint: &str) -> String {
    pick_locale_tag(files.keys().map(String::as_str), hint).expect("语言包为空")
}

/// 和 `ui_assets::pick_default` 同义（build script 不能依赖主 crate，只好各留一份；
/// 两边行为不一致时 `ui_assets` 的黄金文件测试会把 CI 拦下来）。
fn pick_locale_tag<'a>(tags: impl Iterator<Item = &'a str>, hint: &str) -> Option<String> {
    let tags: Vec<&str> = tags.collect();
    let wanted = hint.trim().to_ascii_lowercase();

    if !wanted.is_empty() {
        if let Some(key) = tags.iter().find(|key| key.eq_ignore_ascii_case(&wanted)) {
            return Some((*key).to_string());
        }
        if let Some(key) = tags.iter().find(|key| key.to_ascii_lowercase().starts_with(&wanted)) {
            return Some((*key).to_string());
        }
    }

    if let Some(fallback) = tags.iter().find(|key| key.eq_ignore_ascii_case(FALLBACK_LOCALE)) {
        return Some((*fallback).to_string());
    }
    tags.first().map(|key| (*key).to_string())
}

fn load_locale(path: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|err| panic!("读取 {} 失败：{err}", path.display()));
    let value: toml::Value = toml::from_str(&text).unwrap_or_else(|err| panic!("解析 {} 失败：{err}", path.display()));

    let mut table = BTreeMap::new();
    flatten(&value, "", &mut table, path);
    table
}

fn flatten(value: &toml::Value, prefix: &str, out: &mut BTreeMap<String, String>, path: &Path) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                let full = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(child, &full, out, path);
            }
        }
        toml::Value::String(text) => {
            out.insert(prefix.to_string(), text.clone());
        }
        other => panic!("{}：文案键 {prefix} 必须是字符串，现在是 {other}", path.display()),
    }
}

/// 各语言包必须键集合完全一致：语言是运行时按索引切的，缺一条就会串行。
fn check_same_keys(locales: &[(String, BTreeMap<String, String>)]) {
    let (reference_tag, reference) = &locales[0];
    for (tag, table) in &locales[1..] {
        let missing: Vec<&str> = reference
            .keys()
            .filter(|key| !table.contains_key(*key))
            .map(String::as_str)
            .collect();
        let extra: Vec<&str> = table
            .keys()
            .filter(|key| !reference.contains_key(*key))
            .map(String::as_str)
            .collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{tag} 与 {reference_tag} 的文案键不一致：\n  缺少 {missing:?}\n  多出 {extra:?}"
        );
    }
}

/// 文案键 -> Rust 常量名，例如 `status.hook_mounted` -> `STATUS_HOOK_MOUNTED`。
fn const_name(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/* -------------------------------------------------------------- 生成 Rust */

fn write_rust_module(
    path: &Path,
    build_default: &str,
    keys: &[String],
    locales: &[(String, BTreeMap<String, String>)],
) {
    let mut out = String::new();
    out.push_str("// 由 build.rs 从 locales/*.toml 生成，请勿手改。\n\n");

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
            let value = table.get(key).unwrap_or_else(|| panic!("{tag} 缺少文案键 {key}"));
            out.push_str(&format!("            {value:?},\n"));
        }
        out.push_str("        ],\n    },\n");
    }
    out.push_str("];\n\n");

    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for (index, key) in keys.iter().enumerate() {
        let name = const_name(key);
        if let Some(previous) = owners.insert(name.clone(), key.clone()) {
            panic!("文案键 {previous} 与 {key} 生成的常量名都是 {name}，请改掉其中一个键名");
        }
        out.push_str(&format!("/// `{key}`\npub const {name}: Key = Key({index});\n"));
    }

    std::fs::write(path, out).unwrap_or_else(|err| panic!("写入 {} 失败：{err}", path.display()));
}

/// 只是提示：语言包里没人用的键，多半是改名后忘了删。
fn warn_unused_keys(keys: &[String], src_text: &str, ui_text: &str) {
    for key in keys {
        // locale.* 是语言包自己的元信息（语言名），由 build.rs 直接消费
        if key.starts_with("locale.") {
            continue;
        }
        // 常量名有两种写法都要认：`scenarios.title` → `TITLE`，
        // 但键名本身已经带下划线时（`preferences.hotkey_error_taken`）
        // 源码里更容易写成全路径 `PREFERENCES_HOTKEY_ERROR_TAKEN`。
        let full = key.replace('.', "_").to_ascii_uppercase();
        let used_in_rust = src_text.contains(&const_name(key)) || src_text.contains(&full);
        let used_in_ui = ui_text.contains(&format!("{{{{{key}}}}}"))
            || ui_text.contains(&format!("\"{key}\""))
            || ui_text.contains(&format!("'{key}'"));
        if !used_in_rust && !used_in_ui {
            println!("cargo:warning=文案键 {key} 没有被任何地方引用");
        }
    }
}

/* ------------------------------------------------------------ 生成前端资源 */

fn write_web_assets(
    ui_dir: &Path,
    dist_dir: &Path,
    build_default: &str,
    keys: &[String],
    locales: &[(String, BTreeMap<String, String>)],
) {
    if dist_dir.exists() {
        std::fs::remove_dir_all(dist_dir).unwrap_or_else(|err| panic!("清理 {} 失败：{err}", dist_dir.display()));
    }
    std::fs::create_dir_all(dist_dir).unwrap_or_else(|err| panic!("创建 {} 失败：{err}", dist_dir.display()));

    // 首屏按「本次编译的默认语言」静态替换；运行时切语言由 locale.js 负责
    let default_table = locales
        .iter()
        .find(|(tag, _)| tag == build_default)
        .map(|(_, table)| table)
        .unwrap_or(&locales[0].1);

    let mut missing: BTreeSet<String> = BTreeSet::new();
    let entries = std::fs::read_dir(ui_dir).unwrap_or_else(|err| panic!("读取 {} 失败：{err}", ui_dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("读取 {} 失败：{err}", path.display()));
        let rendered = substitute(&text, default_table, name, &mut missing);
        std::fs::write(dist_dir.join(name), rendered).unwrap_or_else(|err| panic!("写入 {name} 失败：{err}"));
    }

    if !missing.is_empty() {
        panic!(
            "ui/ 里有 {} 个文案键在 locales/{build_default}.toml 中不存在：\n  {}",
            missing.len(),
            missing.into_iter().collect::<Vec<_>>().join("\n  ")
        );
    }

    std::fs::write(dist_dir.join("locale.js"), locale_js(build_default, keys, locales))
        .unwrap_or_else(|err| panic!("写入 locale.js 失败：{err}"));
}

/// 把 `{{ key }}` 替换成文案；替换不了的记进 missing。
fn substitute(text: &str, table: &BTreeMap<String, String>, file: &str, missing: &mut BTreeSet<String>) -> String {
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
fn locale_js(build_default: &str, keys: &[String], locales: &[(String, BTreeMap<String, String>)]) -> String {
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

    let json = serde_json::to_string_pretty(&serde_json::Value::Array(list))
        .unwrap_or_else(|err| panic!("序列化语言包失败：{err}"));

    format!(
        "// 由 build.rs 从 locales/*.toml 生成，请勿手改。\nwindow.MEOW_DEFAULT_LOCALE = {build_default:?};\nwindow.MEOW_LOCALES = {json};\n"
    )
}

fn read_all(dir: &Path) -> String {
    let mut text = String::new();
    collect_text(dir, &mut text);
    text
}

fn collect_text(dir: &Path, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_text(&path, out);
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            out.push_str(&text);
            out.push('\n');
        }
    }
}
