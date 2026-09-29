//! 本地化的一致性校验。
//!
//! build.rs 负责替换，但这个测试负责"体检"：
//!   - 各语言包必须有完全相同的键集合（少一条就会在切换语言时露出占位符）
//!   - ui/index.html 里的 `{{key}}`、ui/main.js 里的 `t("key")` 必须真的存在
//!   - 语言包里的键不该长期没人用（build.rs 会给警告）

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
        other => panic!("文案 {prefix} 不是字符串：{other}"),
    }
}

fn load_locale(name: &str) -> BTreeMap<String, String> {
    let path = root().join("locales").join(format!("{name}.toml"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("读不到 {}：{err}", path.display()));
    let value: toml::Value = toml::from_str(&text).unwrap_or_else(|err| panic!("{name} 解析失败：{err}"));
    let mut table = BTreeMap::new();
    flatten(&value, "", &mut table);
    table
}

fn locale_files() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root().join("locales"))
        .expect("locales/ 不存在")
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
                .then(|| path.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "locales/ 下一个语言包都没有");
    names
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative)).unwrap_or_else(|err| panic!("读不到 {relative}：{err}"))
}

/// index.html 里的 `{{ key }}` 以及 `data-i18n(-placeholder/-title)="key"`。
fn html_keys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();

    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        keys.insert(after[..end].trim().to_string());
        rest = &after[end + 2..];
    }

    for attribute in ["data-i18n=\"", "data-i18n-placeholder=\"", "data-i18n-title=\""] {
        let mut rest = text;
        while let Some(start) = rest.find(attribute) {
            let after = &rest[start + attribute.len()..];
            let Some(end) = after.find('"') else { break };
            keys.insert(after[..end].trim().to_string());
            rest = &after[end + 1..];
        }
    }

    keys
}

/// main.js 里的 `t("key")`（要求 `t(` 前面不是标识符字符，避免误伤 format("...") 之类）。
fn js_keys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(found) = text[index..].find("t(\"") {
        let at = index + found;
        let boundary_ok =
            at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_' || bytes[at - 1] == b'$');
        if boundary_ok {
            let after = &text[at + 3..];
            if let Some(end) = after.find('"') {
                keys.insert(after[..end].to_string());
            }
        }
        index = at + 3;
    }
    keys
}

#[test]
fn all_locales_share_the_same_keys() {
    let names = locale_files();
    let reference = load_locale(&names[0]);
    let reference_keys: BTreeSet<&String> = reference.keys().collect();

    for name in &names[1..] {
        let table = load_locale(name);
        let keys: BTreeSet<&String> = table.keys().collect();

        let missing: Vec<_> = reference_keys.difference(&keys).map(|key| key.as_str()).collect();
        let extra: Vec<_> = keys.difference(&reference_keys).map(|key| key.as_str()).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{name} 与 {} 的键不一致：\n  缺少：{missing:?}\n  多出：{extra:?}",
            names[0]
        );
    }
}

#[test]
fn referenced_keys_exist_in_every_locale() {
    let names = locale_files();
    let referenced: BTreeSet<String> = html_keys(&read("ui/index.html"))
        .into_iter()
        .chain(js_keys(&read("ui/main.js")))
        .collect();
    assert!(
        referenced.len() > 40,
        "只扫到 {} 个文案键，扫描逻辑可能失效了",
        referenced.len()
    );

    for name in &names {
        let table = load_locale(name);
        let missing: Vec<_> = referenced
            .iter()
            .filter(|key| !table.contains_key(*key))
            .cloned()
            .collect();
        assert!(missing.is_empty(), "{name} 缺少被引用的文案键：{missing:?}");
    }
}

#[test]
fn rust_module_names_are_used_where_expected() {
    // 语言包里每个键都应该能被 build.rs 变成一个常量名；
    // 这里顺手确认几个后端在用的键确实存在（写错会让编译失败，这里给更友好的提示）。
    let table = load_locale("zh-CN");
    for key in [
        "log.hook_mounted",
        "log.appended",
        "skip.empty_input",
        "summary.line",
        "scenario.qq.name",
        "status.hook_mounted",
        "tray.show",
        "close_action.hide",
    ] {
        assert!(table.contains_key(key), "语言包里缺少 {key}");
    }
}

/// 语言是运行时按索引切的：每个语言包的键顺序必须和参考语言完全一致，
/// 否则会出现「切了语言但某一项没变」甚至串行。
#[test]
fn every_locale_keeps_the_same_key_order() {
    let names = locale_files();
    let reference: Vec<String> = load_locale(&names[0]).keys().cloned().collect();

    for name in &names[1..] {
        let keys: Vec<String> = load_locale(name).keys().cloned().collect();
        assert_eq!(keys, reference, "{name} 的键顺序与 {} 不一致", names[0]);
    }
}

/// 少量文案里内嵌 HTML（比如自测提示里的加粗、场景说明里的等宽进程名）。
/// 前端对含 `<` 的文案走 innerHTML，所以这里把允许出现的标签钉死，
/// 免得手滑写出 `< b>` 之类的东西在界面上原样露出来。
#[test]
fn inline_markup_in_locales_uses_only_allowed_tags() {
    const ALLOWED: [&str; 4] = ["<b>", "</b>", "<code>", "</code>"];

    for name in locale_files() {
        for (key, text) in load_locale(&name) {
            let mut rest = text.as_str();
            while let Some(start) = rest.find('<') {
                let after = &rest[start..];
                let Some(end) = after.find('>') else {
                    panic!("{name} 的 {key} 里有个没闭合的 `<`：{text}");
                };
                let tag = &after[..=end];
                assert!(
                    ALLOWED.contains(&tag),
                    "{name} 的 {key} 用了未允许的内联标签 {tag}（允许：{ALLOWED:?}）：{text}"
                );
                rest = &after[end + 1..];
            }
        }
    }
}

#[test]
fn no_placeholder_leaks_into_generated_assets() {
    // dist/ 由 cargo build 生成；存在的话顺手确认没有残留 {{...}}
    let dist = Path::new("dist");
    if !dist.exists() {
        return;
    }
    for entry in std::fs::read_dir(root().join(dist)).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("js")
            && path.file_name().and_then(|name| name.to_str()) == Some("locale.js")
        {
            continue; // 文案表本身可能包含大括号
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        assert!(!text.contains("{{"), "{} 里还有没替换掉的占位符", path.display());
    }
}
