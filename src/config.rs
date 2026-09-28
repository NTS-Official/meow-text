//! 配置的默认值、磁盘读写与内置场景。

use crate::i18n;
use crate::meow::{Config, InjectMode, Scenario, Stats, ThemeMode, Trigger, DEFAULT_SUFFIX};
use std::path::Path;

/// 内置场景的显示名与说明来自语言包（**每次取用都按当前语言**，切语言后自动跟着变）。
fn builtin_text(id: &str) -> Option<(&'static str, &'static str)> {
    match id {
        "qq" => Some((i18n::SCENARIO_QQ_NAME.text(), i18n::SCENARIO_QQ_NOTE.text())),
        "tim" => Some((i18n::SCENARIO_TIM_NAME.text(), i18n::SCENARIO_TIM_NOTE.text())),
        "wechat" => Some((
            i18n::SCENARIO_WECHAT_NAME.text(),
            i18n::SCENARIO_WECHAT_NOTE.text(),
        )),
        _ => None,
    }
}

/// 内置场景。QQ 默认开启，其余默认关闭，用户可在面板里改。
pub fn default_scenarios() -> Vec<Scenario> {
    let mut scenarios = vec![
        Scenario {
            id: "qq".into(),
            name: String::new(),
            processes: vec!["qq.exe".into()],
            enabled: true,
            suffix: None,
            trigger: None,
            builtin: true,
            note: String::new(),
        },
        Scenario {
            id: "tim".into(),
            name: String::new(),
            processes: vec!["tim.exe".into()],
            enabled: false,
            suffix: None,
            trigger: None,
            builtin: true,
            note: String::new(),
        },
        Scenario {
            id: "wechat".into(),
            name: String::new(),
            processes: vec!["wechat.exe".into(), "weixin.exe".into()],
            enabled: false,
            suffix: None,
            trigger: None,
            builtin: true,
            note: String::new(),
        },
    ];
    for scenario in &mut scenarios {
        if let Some((name, note)) = builtin_text(&scenario.id) {
            scenario.name = name.to_string();
            scenario.note = note.to_string();
        }
    }
    scenarios
}

pub fn default_config() -> Config {
    Config {
        suffix: DEFAULT_SUFFIX.to_string(),
        theme: ThemeMode::System,
        scenarios: default_scenarios(),
        ..Config::default()
    }
}

/// 读配置；文件不存在或损坏时回落到默认值，并补齐缺失的内置场景。
pub fn load_or_default(path: &Path) -> Config {
    let mut cfg = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Config>(&text).unwrap_or_else(|err| {
            eprintln!("[meow] 配置文件解析失败，改用默认配置：{err}");
            default_config()
        }),
        Err(_) => default_config(),
    };
    ensure_builtin_scenarios(&mut cfg);
    cfg.normalize();
    cfg
}

/// 内置场景即使被手工删掉也会补回来；名字与说明每次都按当前语言刷新
/// （这样换语言重新编译后，内置场景的显示名会跟着变，自定义场景保持原样）。
pub fn ensure_builtin_scenarios(cfg: &mut Config) {
    for builtin in default_scenarios() {
        match cfg.scenarios.iter_mut().find(|s| s.id == builtin.id) {
            Some(existing) => {
                existing.builtin = true;
                if let Some((name, note)) = builtin_text(&builtin.id) {
                    existing.name = name.to_string();
                    existing.note = note.to_string();
                }
            }
            None => cfg.scenarios.push(builtin),
        }
    }
}

pub fn save(path: &Path, cfg: &Config) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化配置失败：{e}"))?;
    std::fs::write(path, text).map_err(|e| format!("写入配置失败：{e}"))
}

/// 面板展示用摘要（跟随当前语言）。
pub fn summarize(cfg: &Config) -> String {
    let enabled: Vec<&str> = cfg
        .scenarios
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.as_str())
        .collect();
    let scenarios = if enabled.is_empty() {
        i18n::SUMMARY_NO_SCENARIOS.text().to_string()
    } else {
        enabled.join("、")
    };

    i18n::SUMMARY_LINE.fill(&[
        (
            "state",
            if cfg.enabled {
                i18n::SUMMARY_ENABLED.text()
            } else {
                i18n::SUMMARY_DISABLED.text()
            },
        ),
        ("suffix", cfg.suffix.as_str()),
        ("trigger", cfg.trigger.label()),
        ("mode", InjectMode::label(cfg.inject_mode)),
        ("scenarios", scenarios.as_str()),
    ])
}

/// 统计：跨天时重置今日计数。
pub fn bump_stats(cfg: &mut Config, today: &str) {
    if cfg.stats.day != today {
        cfg.stats.day = today.to_string();
        cfg.stats.today = 0;
    }
    cfg.stats.total = cfg.stats.total.saturating_add(1);
    cfg.stats.today = cfg.stats.today.saturating_add(1);
}

pub fn reset_stats(cfg: &mut Config, today: &str) {
    cfg.stats = Stats {
        total: 0,
        today: 0,
        day: today.to_string(),
    };
}

/// 面板下拉框用的触发键列表。
pub fn trigger_options() -> [Trigger; 2] {
    [Trigger::Enter, Trigger::CtrlEnter]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meow::{CloseAction, Config as MeowConfig};

    /// 控制面板拿到的 JSON 就是它写回来的 JSON：这里把线格式钉住，
    /// 免得哪天改了 serde 命名，前端静默失效。
    #[test]
    fn ui_wire_format_round_trips() {
        let mut cfg = default_config();
        cfg.enabled = true;
        cfg.trigger = Trigger::CtrlEnter;
        cfg.inject_mode = InjectMode::Clipboard;
        cfg.theme = ThemeMode::Dark;
        cfg.scenarios[1].suffix = Some("嗷呜~".into());
        cfg.normalize();

        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"trigger\":\"ctrl_enter\""), "{json}");
        assert!(json.contains("\"inject_mode\":\"clipboard\""), "{json}");
        assert!(json.contains("\"theme\":\"dark\""), "{json}");
        assert!(json.contains("\"suffix\":\"嗷呜~\""), "{json}");
        assert!(json.contains("\"processes\":[\"qq.exe\"]"), "{json}");

        let back: MeowConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    /// 老版本写下的配置里没有 theme 字段，必须还能读。
    #[test]
    fn config_without_theme_still_loads() {
        let mut cfg = default_config();
        cfg.enabled = true;
        let mut json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("theme");

        let back: MeowConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.theme, ThemeMode::System);
    }

    /// 同理：老配置里也没有 close_action，默认应当是「每次询问」。
    #[test]
    fn config_without_close_action_defaults_to_ask() {
        let mut cfg = default_config();
        cfg.close_action = CloseAction::Quit;
        let mut json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("close_action");

        let back: MeowConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.close_action, CloseAction::Ask);
        assert!(CloseAction::Hide.is_hide());
        assert!(!CloseAction::Quit.is_hide());
    }

    /// 开机自启也是后加的字段：老配置里没有它，默认必须是关（不能替用户擅自加启动项）。
    #[test]
    fn config_without_autostart_defaults_to_off() {
        let mut cfg = default_config();
        cfg.autostart = true;
        let mut json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("autostart");

        let back: MeowConfig = serde_json::from_value(json).unwrap();
        assert!(!back.autostart, "老配置读回来应当是「不开机自启」");
        assert!(!MeowConfig::default().autostart, "默认配置也必须是不自启");
    }

    /// 热键也是后加的字段：老配置没有它，默认应当是 Ctrl+Alt+'。
    #[test]
    fn config_without_hotkey_defaults_to_ctrl_alt_quote() {
        let mut cfg = default_config();
        cfg.hotkey = "ctrl+shift+m".into();
        let mut json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("hotkey");

        let back: MeowConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.hotkey, crate::hotkey::DEFAULT_HOTKEY);
    }

    /// 坏掉的热键写法不该留在配置里（留着只会注册失败）。
    #[test]
    fn broken_hotkey_is_normalized_away() {
        let mut cfg = default_config();
        cfg.hotkey = "这不是热键".into();
        cfg.normalize();
        assert_eq!(cfg.hotkey, "", "无法解析的热键应当被清空（= 不启用）");

        let mut cfg = default_config();
        cfg.hotkey = "ALT + ctrl + '".into();
        cfg.normalize();
        assert_eq!(cfg.hotkey, "ctrl+alt+'", "合法写法要规范成统一形式");
    }

    #[test]
    fn save_and_reload_round_trips() {
        let path = std::env::temp_dir().join("meow-text-config-test.json");
        let _ = std::fs::remove_file(&path);

        let mut cfg = default_config();
        cfg.enabled = true;
        save(&path, &cfg).unwrap();
        assert_eq!(load_or_default(&path), cfg);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn deleted_builtin_scenarios_come_back() {
        let mut cfg = Config {
            scenarios: Vec::new(),
            ..default_config()
        };
        ensure_builtin_scenarios(&mut cfg);
        assert!(cfg.scenarios.iter().any(|s| s.id == "qq" && s.enabled));
        assert!(cfg.scenarios.iter().any(|s| s.id == "wechat" && !s.enabled));
        assert!(cfg.scenarios.iter().all(|s| !s.name.is_empty()));
    }

    #[test]
    fn stats_count_up_and_reset_per_day() {
        let mut cfg = default_config();
        bump_stats(&mut cfg, "2026-01-01");
        bump_stats(&mut cfg, "2026-01-01");
        assert_eq!((cfg.stats.today, cfg.stats.total), (2, 2));

        bump_stats(&mut cfg, "2026-01-02");
        assert_eq!((cfg.stats.today, cfg.stats.total), (1, 3));
    }
}
