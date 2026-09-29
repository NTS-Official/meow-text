//! 本地化文案：**运行时可切换语言**。
//!
//! 所有语言包都在编译期由 `build.rs` 塞进 `$OUT_DIR/i18n.rs`（`LOCALES` 静态表），
//! 运行时只改一个下标，不需要读盘、也不需要重新加载页面：
//!
//! ```ignore
//! i18n::set_locale("en-US");
//! i18n::SETTINGS_TITLE.text()                     // -> "Basics"
//! i18n::LOG_APPENDED.fill(&[("suffix", "喵~")])   // -> "Appended “喵~”"
//! ```
//!
//! 每条文案在生成期都会得到一个 `Key` 常量，写错键名是编译错误 —— 这点和以前一样。

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 一条文案的句柄。只能由 build.rs 生成的常量构造，所以键名不会写错。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key(usize);

include!(concat!(env!("OUT_DIR"), "/i18n.rs"));

/// 当前语言在 `LOCALES` 里的下标；`usize::MAX` 表示还没选过（用编译默认语言）。
static CURRENT: AtomicUsize = AtomicUsize::new(usize::MAX);

impl Key {
    /// 当前语言下的文案。
    pub fn text(self) -> &'static str {
        let locale = &LOCALES[current_index()];
        locale
            .strings
            .get(self.0)
            .copied()
            .or_else(|| default_locale().strings.get(self.0).copied())
            // 理论上到不了这里（build.rs 会强制各语言键一致），兜底显示键名便于定位
            .unwrap_or_else(|| self.name())
    }

    /// 文案里的 `{name}` 占位符替换。
    pub fn fill(self, vars: &[(&str, &str)]) -> String {
        let mut text = String::from(self.text());
        for (name, value) in vars {
            let needle = format!("{{{name}}}");
            if text.contains(&needle) {
                text = text.replace(&needle, value);
            }
        }
        text
    }

    /// 这条文案的键名（调试/兜底用）。
    pub fn name(self) -> &'static str {
        KEYS.get(self.0).copied().unwrap_or("")
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}

impl From<Key> for String {
    fn from(key: Key) -> String {
        key.text().to_string()
    }
}

/* ------------------------------------------------------------ 语言选择 */

fn default_index() -> usize {
    LOCALES
        .iter()
        .position(|locale| locale.tag == DEFAULT_LOCALE)
        .unwrap_or(0)
}

fn default_locale() -> &'static Locale {
    &LOCALES[default_index()]
}

fn current_index() -> usize {
    let index = CURRENT.load(Ordering::Relaxed);
    if index < LOCALES.len() { index } else { default_index() }
}

/// 全部可用语言（按语言标签排序）。
pub fn available() -> &'static [Locale] {
    LOCALES
}

/// 当前语言标签。
pub fn current_tag() -> &'static str {
    LOCALES[current_index()].tag
}

/// 当前语言的自称，例如「简体中文」/「English」。
pub fn current_name() -> &'static str {
    LOCALES[current_index()].name
}

/// 编译时的默认语言标签。
pub fn default_tag() -> &'static str {
    default_locale().tag
}

/// 切换语言；标签不认识时返回 false（保持原样）。
pub fn set_locale(tag: &str) -> bool {
    match LOCALES.iter().position(|locale| locale.tag.eq_ignore_ascii_case(tag)) {
        Some(index) => {
            CURRENT.store(index, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// 把配置里的语言偏好（`system` 或具体标签）解析成实际可用的标签。
pub fn resolve_tag(preference: &str) -> &'static str {
    let preference = preference.trim();
    if preference.is_empty() || preference.eq_ignore_ascii_case("system") {
        return system_tag();
    }
    LOCALES
        .iter()
        .find(|locale| locale.tag.eq_ignore_ascii_case(preference))
        .map(|locale| locale.tag)
        .unwrap_or_else(system_tag)
}

/// 系统界面语言对应的标签（没有对应语言包就回落到编译默认）。
pub fn system_tag() -> &'static str {
    let prefix = system_language_prefix();
    if prefix.is_empty() {
        return default_tag();
    }
    LOCALES
        .iter()
        .find(|locale| locale.tag.to_ascii_lowercase().starts_with(&prefix))
        .map(|locale| locale.tag)
        .unwrap_or_else(default_tag)
}

/// 解析并应用一个语言偏好，返回实际生效的标签。
pub fn apply_preference(preference: &str) -> &'static str {
    let tag = resolve_tag(preference);
    set_locale(tag);
    tag
}

#[cfg(windows)]
fn system_language_prefix() -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    let langid = unsafe { GetUserDefaultUILanguage() } as u32;
    match langid & 0x03ff {
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
    .to_string()
}

#[cfg(not(windows))]
fn system_language_prefix() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_index_aligned() {
        assert!(!LOCALES.is_empty(), "至少要有一个语言包");
        assert!(!KEYS.is_empty(), "至少要有一条文案");
        for locale in LOCALES {
            assert_eq!(
                locale.strings.len(),
                KEYS.len(),
                "{} 的文案条数和键表对不上（会串行）",
                locale.tag
            );
            assert!(!locale.name.is_empty(), "{} 缺少语言自称", locale.tag);
        }
        assert!(LOCALES.iter().any(|l| l.tag == DEFAULT_LOCALE));
    }

    #[test]
    fn keys_are_unique_and_non_empty() {
        let unique: std::collections::BTreeSet<&&str> = KEYS.iter().collect();
        assert_eq!(unique.len(), KEYS.len(), "键表里有重复项");
        assert!(KEYS.iter().all(|key| !key.is_empty()));
    }

    /// 运行时切换：同一把 Key 在不同语言下取到不同文案，切回来也正常。
    #[test]
    fn switching_locale_changes_text() {
        let original = current_tag();
        let index = KEYS
            .iter()
            .position(|key| *key == "settings.title")
            .expect("缺少 settings.title");

        let mut texts = Vec::new();
        for locale in LOCALES {
            assert!(set_locale(locale.tag));
            assert_eq!(current_tag(), locale.tag);
            texts.push(Key(index).text());
        }
        assert!(
            texts.iter().any(|text| *text != texts[0]),
            "两种语言取到同一串文案，切换可能没生效：{texts:?}"
        );

        assert!(set_locale(original));
        assert_eq!(current_tag(), original);

        // 不认识的语言不该改动当前选择
        assert!(!set_locale("xx-XX"));
        assert_eq!(current_tag(), original);
    }

    #[test]
    fn fill_replaces_placeholders() {
        let template = i18n_key("log.appended");
        let text = template.fill(&[("suffix", "喵~")]);
        assert!(text.contains("喵~"), "{text}");
        assert!(!text.contains("{suffix}"), "{text}");
    }

    #[test]
    fn resolve_tag_falls_back_to_something_available() {
        assert!(LOCALES.iter().any(|l| l.tag == resolve_tag("system")));
        assert!(LOCALES.iter().any(|l| l.tag == resolve_tag("")));
        assert_eq!(resolve_tag("en-US"), "en-US");
        assert_eq!(resolve_tag("EN-us"), "en-US");
        // 未知标签 -> 系统语言（或默认），反正得是存在的
        assert!(LOCALES.iter().any(|l| l.tag == resolve_tag("kl-GL")));
    }

    fn i18n_key(key: &str) -> Key {
        Key(KEYS.iter().position(|candidate| *candidate == key).expect("键不存在"))
    }
}
