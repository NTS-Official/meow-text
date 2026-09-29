//! 纯逻辑层：不碰任何 Win32 API，方便单元测试。
//!
//! 这里回答一个问题：**这一次回车键，应该被喵化吗？该用哪个后缀？**

use crate::hotkey;
use crate::i18n;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SUFFIX: &str = "喵~";

/// 语言偏好为「跟随系统」时的取值。
pub const LANGUAGE_SYSTEM: &str = "system";

fn default_language() -> String {
    LANGUAGE_SYSTEM.to_string()
}

/// 热键的默认值：Ctrl+Alt+'。
fn default_hotkey() -> String {
    hotkey::DEFAULT_HOTKEY.to_string()
}

/// 把配置里的语言偏好规范成 `system` 或某个真实存在的语言标签。
pub fn canonical_language(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case(LANGUAGE_SYSTEM) {
        return LANGUAGE_SYSTEM.to_string();
    }
    i18n::available()
        .iter()
        .find(|locale| locale.tag.eq_ignore_ascii_case(raw))
        .map(|locale| locale.tag.to_string())
        .unwrap_or_else(|| LANGUAGE_SYSTEM.to_string())
}

/// 发送键（把消息发出去的那个键）。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    #[default]
    Enter,
    CtrlEnter,
}

impl Trigger {
    pub fn label(self) -> &'static str {
        match self {
            Trigger::Enter => i18n::TRIGGER_ENTER.text(),
            Trigger::CtrlEnter => i18n::TRIGGER_CTRL_ENTER.text(),
        }
    }

    /// 当前修饰键状态是否符合该触发键。
    pub fn matches(self, ctrl_down: bool) -> bool {
        match self {
            // Enter 发送时，Ctrl+Enter 通常是「换行」，不要动它
            Trigger::Enter => !ctrl_down,
            Trigger::CtrlEnter => ctrl_down,
        }
    }
}

/// 界面主题。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// 跟着操作系统的浅色/深色设置走
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => i18n::THEME_SYSTEM.text(),
            ThemeMode::Light => i18n::THEME_LIGHT.text(),
            ThemeMode::Dark => i18n::THEME_DARK.text(),
        }
    }
}

/// 点窗口关闭按钮（×）时的行为。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloseAction {
    /// 每次弹提示问一下
    #[default]
    Ask,
    /// 隐藏到任务栏，继续在后台跑
    Hide,
    /// 直接退出
    Quit,
}

impl CloseAction {
    pub fn label(self) -> &'static str {
        match self {
            CloseAction::Ask => i18n::CLOSE_ACTION_ASK.text(),
            CloseAction::Hide => i18n::CLOSE_ACTION_HIDE.text(),
            CloseAction::Quit => i18n::CLOSE_ACTION_QUIT.text(),
        }
    }

    /// 隐藏窗口时要不要往日志里记一笔（退出时进程都没了，记也没用）。
    pub fn is_hide(self) -> bool {
        matches!(self, CloseAction::Hide)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectMode {
    /// 逐字模拟 Unicode 输入，最不打扰用户
    #[default]
    Type,
    /// 走剪贴板 + Ctrl+V，作为 Electron / Chromium 类应用的兜底
    Clipboard,
}

impl InjectMode {
    pub fn label(self) -> &'static str {
        match self {
            InjectMode::Type => i18n::MODE_TYPE.text(),
            InjectMode::Clipboard => i18n::MODE_CLIPBOARD.text(),
        }
    }
}

/// 一个「场景」= 一类要被喵化的应用。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    /// 小写进程名（含 .exe）
    pub processes: Vec<String>,
    pub enabled: bool,
    /// 覆盖全局后缀（留空表示用全局）
    #[serde(default)]
    pub suffix: Option<String>,
    /// 覆盖全局触发键
    #[serde(default)]
    pub trigger: Option<Trigger>,
    /// 内置场景不允许删除
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub note: String,
}

impl Scenario {
    pub fn matches_process(&self, process: &str) -> bool {
        self.processes.iter().any(|p| p.eq_ignore_ascii_case(process))
    }

    pub fn suffix_or<'a>(&'a self, global: &'a str) -> &'a str {
        self.suffix
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(global)
    }

    pub fn trigger_or(&self, global: Trigger) -> Trigger {
        self.trigger.unwrap_or(global)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Stats {
    pub total: u64,
    pub today: u64,
    /// 用于跨天重置 today
    pub day: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// 总开关
    pub enabled: bool,
    /// 追加在句尾的文本
    pub suffix: String,
    pub trigger: Trigger,
    pub inject_mode: InjectMode,
    /// 注入前的等待时间，给输入框一点反应时间
    pub inject_delay_ms: u64,
    /// 只在输入框非空时追加（启发式判断，避免发出只有「喵~」的消息）
    pub require_content: bool,
    /// 输入法正在组词时不动回车
    pub skip_when_composing: bool,
    /// 演练模式：只记录日志，不真的注入
    pub dry_run: bool,
    /// 界面主题（老配置文件里没有这一项，默认跟随系统）
    #[serde(default)]
    pub theme: ThemeMode,
    /// 点 × 时的行为（老配置文件里没有这一项，默认每次询问）
    #[serde(default)]
    pub close_action: CloseAction,
    /// 界面语言：`system` 表示跟随系统，否则是 `locales/` 下的标签（运行时可切）
    #[serde(default = "default_language")]
    pub language: String,
    /// 开机自启（默认关；真正的开关在 HKCU 的启动项里，这里只是用户的意愿）
    #[serde(default)]
    pub autostart: bool,
    /// 开/关总开关的全局热键（默认 Ctrl+Alt+'；空字符串 = 不启用热键）
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    pub scenarios: Vec<Scenario>,
    #[serde(default)]
    pub stats: Stats,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            suffix: DEFAULT_SUFFIX.to_string(),
            trigger: Trigger::Enter,
            inject_mode: InjectMode::Type,
            inject_delay_ms: 40,
            require_content: true,
            skip_when_composing: true,
            dry_run: false,
            theme: ThemeMode::System,
            close_action: CloseAction::Ask,
            language: default_language(),
            autostart: false,
            hotkey: default_hotkey(),
            scenarios: Vec::new(),
            stats: Stats::default(),
        }
    }
}

impl Config {
    /// 兜底修正来自界面 / 磁盘的脏数据。
    pub fn normalize(&mut self) {
        self.suffix = self.suffix.trim().to_string();
        if self.suffix.is_empty() {
            self.suffix = DEFAULT_SUFFIX.to_string();
        }
        self.inject_delay_ms = self.inject_delay_ms.min(2000);
        self.language = canonical_language(&self.language);
        // 热键写坏了就当没配（留着只会注册失败，界面也没法显示）
        self.hotkey = hotkey::canonical(&self.hotkey).unwrap_or_default();

        for scenario in &mut self.scenarios {
            let mut cleaned: Vec<String> = Vec::new();
            for raw in &scenario.processes {
                let name = raw.trim().to_ascii_lowercase();
                if !name.is_empty() && !cleaned.contains(&name) {
                    cleaned.push(name);
                }
            }
            scenario.processes = cleaned;

            scenario.suffix = scenario
                .suffix
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        }
    }

    /// 找到前台进程对应的场景。
    pub fn scenario_for(&self, process: Option<&str>) -> Option<&Scenario> {
        let process = process?;
        self.scenarios.iter().find(|s| s.matches_process(process))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Disabled,
    NotTriggerKey,
    Injected,
    NoScenario,
    ScenarioDisabled,
    ModifierHeld,
    Composing,
    EmptyInput,
}

impl SkipReason {
    pub fn label(self) -> &'static str {
        match self {
            SkipReason::Disabled => i18n::SKIP_DISABLED.text(),
            SkipReason::NotTriggerKey => i18n::SKIP_NOT_TRIGGER.text(),
            SkipReason::Injected => i18n::SKIP_INJECTED.text(),
            SkipReason::NoScenario => i18n::SKIP_NO_SCENARIO.text(),
            SkipReason::ScenarioDisabled => i18n::SKIP_SCENARIO_DISABLED.text(),
            SkipReason::ModifierHeld => i18n::SKIP_MODIFIER.text(),
            SkipReason::Composing => i18n::SKIP_COMPOSING.text(),
            SkipReason::EmptyInput => i18n::SKIP_EMPTY_INPUT.text(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeowPlan {
    pub suffix: String,
    /// 实际生效的触发键（场景可覆盖全局）
    pub trigger: Trigger,
    pub scenario_id: String,
    pub scenario_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Meow(MeowPlan),
    Skip {
        reason: SkipReason,
        /// 只有匹配到场景时的跳过才值得写进日志
        scenario: Option<String>,
    },
}

/// 钩子采集到的一次按键上下文。
pub struct KeyEvent<'a> {
    /// 是否是发送键的按下事件
    pub is_enter_down: bool,
    pub ctrl_down: bool,
    pub alt_down: bool,
    /// 这个按键是不是别的程序（含我们自己）模拟出来的
    pub injected: bool,
    /// 前台窗口进程名（小写）
    pub process: Option<&'a str>,
    pub is_own_process: bool,
    /// 控制面板自测区是否聚焦
    pub selftest_active: bool,
    /// 输入法是否正在组词
    pub composing: bool,
    /// 输入框是否可能是空的（启发式）
    pub input_maybe_empty: bool,
}

/// 核心判定。
///
/// `accept_injected` 仅供测试使用：正常情况下模拟按键一律忽略，避免自己触发自己。
pub fn decide(cfg: &Config, ev: &KeyEvent<'_>, accept_injected: bool) -> Decision {
    let skip = |reason: SkipReason, scenario: Option<String>| Decision::Skip { reason, scenario };

    if !cfg.enabled {
        return skip(SkipReason::Disabled, None);
    }
    if !ev.is_enter_down {
        return skip(SkipReason::NotTriggerKey, None);
    }
    if ev.injected && !accept_injected {
        return skip(SkipReason::Injected, None);
    }
    // Alt+Enter 之类的组合键留给应用自己处理
    if ev.alt_down {
        return skip(SkipReason::ModifierHeld, None);
    }

    // 控制面板自己的自测框：用全局配置，不需要额外场景
    let selftest = ev.is_own_process && ev.selftest_active;
    let scenario = if selftest { None } else { cfg.scenario_for(ev.process) };

    if !selftest && scenario.is_none() {
        return skip(SkipReason::NoScenario, None);
    }

    let scenario_name = if selftest {
        Some("自测".to_string())
    } else {
        scenario.map(|s| s.name.clone())
    };

    if let Some(s) = scenario
        && !s.enabled {
            return skip(SkipReason::ScenarioDisabled, scenario_name);
        }

    let trigger = scenario.map(|s| s.trigger_or(cfg.trigger)).unwrap_or(cfg.trigger);
    if !trigger.matches(ev.ctrl_down) {
        return skip(SkipReason::ModifierHeld, scenario_name);
    }

    if ev.composing && cfg.skip_when_composing {
        return skip(SkipReason::Composing, scenario_name);
    }
    if cfg.require_content && ev.input_maybe_empty {
        return skip(SkipReason::EmptyInput, scenario_name);
    }

    let suffix = scenario
        .map(|s| s.suffix_or(&cfg.suffix))
        .unwrap_or(&cfg.suffix)
        .to_string();

    Decision::Meow(MeowPlan {
        suffix,
        trigger,
        scenario_id: scenario
            .map(|s| s.id.clone())
            .unwrap_or_else(|| "selftest".to_string()),
        scenario_name: scenario_name.unwrap_or_else(|| "自测".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qq_scenario(enabled: bool) -> Scenario {
        Scenario {
            id: "qq".into(),
            name: "QQ".into(),
            processes: vec!["qq.exe".into()],
            enabled,
            suffix: None,
            trigger: None,
            builtin: true,
            note: String::new(),
        }
    }

    fn base_config() -> Config {
        Config {
            enabled: true,
            suffix: "喵~".into(),
            scenarios: vec![qq_scenario(true)],
            ..Config::default()
        }
    }

    fn enter_in_qq() -> KeyEvent<'static> {
        KeyEvent {
            is_enter_down: true,
            ctrl_down: false,
            alt_down: false,
            injected: false,
            process: Some("qq.exe"),
            is_own_process: false,
            selftest_active: false,
            composing: false,
            input_maybe_empty: false,
        }
    }

    #[test]
    fn appends_suffix_when_qq_sends_with_enter() {
        let cfg = base_config();
        match decide(&cfg, &enter_in_qq(), false) {
            Decision::Meow(plan) => {
                assert_eq!(plan.suffix, "喵~");
                assert_eq!(plan.trigger, Trigger::Enter);
                assert_eq!(plan.scenario_name, "QQ");
            }
            other => panic!("应该喵化，实际是 {other:?}"),
        }
    }

    #[test]
    fn disabled_master_switch_does_nothing() {
        let cfg = Config { enabled: false, ..base_config() };
        assert!(matches!(
            decide(&cfg, &enter_in_qq(), false),
            Decision::Skip { reason: SkipReason::Disabled, .. }
        ));
    }

    #[test]
    fn non_enter_key_is_ignored() {
        let cfg = base_config();
        let ev = KeyEvent { is_enter_down: false, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &ev, false),
            Decision::Skip { reason: SkipReason::NotTriggerKey, .. }
        ));
    }

    #[test]
    fn injected_keys_are_ignored_unless_testing() {
        let cfg = base_config();
        let ev = KeyEvent { injected: true, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &ev, false),
            Decision::Skip { reason: SkipReason::Injected, .. }
        ));
        assert!(matches!(decide(&cfg, &ev, true), Decision::Meow(_)));
    }

    #[test]
    fn ctrl_enter_is_left_alone_when_enter_is_the_trigger() {
        let cfg = base_config();
        let ev = KeyEvent { ctrl_down: true, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &ev, false),
            Decision::Skip { reason: SkipReason::ModifierHeld, scenario: Some(_) }
        ));
    }

    #[test]
    fn ctrl_enter_trigger_requires_ctrl() {
        let cfg = Config { trigger: Trigger::CtrlEnter, ..base_config() };

        let with_ctrl = KeyEvent { ctrl_down: true, ..enter_in_qq() };
        assert!(matches!(decide(&cfg, &with_ctrl, false), Decision::Meow(_)));

        assert!(matches!(
            decide(&cfg, &enter_in_qq(), false),
            Decision::Skip { reason: SkipReason::ModifierHeld, .. }
        ));
    }

    #[test]
    fn alt_enter_is_ignored() {
        let cfg = base_config();
        let ev = KeyEvent { alt_down: true, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &ev, false),
            Decision::Skip { reason: SkipReason::ModifierHeld, scenario: None }
        ));
    }

    #[test]
    fn unknown_process_is_ignored_without_logging() {
        let cfg = base_config();
        let ev = KeyEvent { process: Some("notepad.exe"), ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &ev, false),
            Decision::Skip { reason: SkipReason::NoScenario, scenario: None }
        ));
    }

    #[test]
    fn disabled_scenario_is_skipped_but_logged() {
        let cfg = Config { scenarios: vec![qq_scenario(false)], ..base_config() };
        assert!(matches!(
            decide(&cfg, &enter_in_qq(), false),
            Decision::Skip { reason: SkipReason::ScenarioDisabled, scenario: Some(_) }
        ));
    }

    #[test]
    fn composing_and_empty_input_are_skipped() {
        let cfg = base_config();

        let composing = KeyEvent { composing: true, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &composing, false),
            Decision::Skip { reason: SkipReason::Composing, .. }
        ));

        let empty = KeyEvent { input_maybe_empty: true, ..enter_in_qq() };
        assert!(matches!(
            decide(&cfg, &empty, false),
            Decision::Skip { reason: SkipReason::EmptyInput, .. }
        ));

        // 关掉这两个保护后应该继续喵化
        let loose = Config { skip_when_composing: false, require_content: false, ..base_config() };
        assert!(matches!(decide(&loose, &composing, false), Decision::Meow(_)));
        assert!(matches!(decide(&loose, &empty, false), Decision::Meow(_)));
    }

    #[test]
    fn scenario_suffix_and_trigger_override_global() {
        let cfg = Config {
            suffix: "喵~".into(),
            scenarios: vec![Scenario {
                suffix: Some("嗷呜~".into()),
                trigger: Some(Trigger::CtrlEnter),
                ..qq_scenario(true)
            }],
            ..base_config()
        };
        let ev = KeyEvent { ctrl_down: true, ..enter_in_qq() };
        match decide(&cfg, &ev, false) {
            Decision::Meow(plan) => {
                assert_eq!(plan.suffix, "嗷呜~");
                assert_eq!(plan.trigger, Trigger::CtrlEnter);
            }
            other => panic!("应该喵化，实际是 {other:?}"),
        }
    }

    #[test]
    fn selftest_window_uses_global_config() {
        let cfg = base_config();
        let ev = KeyEvent {
            is_enter_down: true,
            ctrl_down: false,
            alt_down: false,
            injected: false,
            process: Some("meow-text.exe"),
            is_own_process: true,
            selftest_active: true,
            composing: false,
            input_maybe_empty: false,
        };
        match decide(&cfg, &ev, false) {
            Decision::Meow(plan) => {
                assert_eq!(plan.suffix, "喵~");
                assert_eq!(plan.scenario_id, "selftest");
            }
            other => panic!("自测区应该喵化，实际是 {other:?}"),
        }

        // 自测开关没开时，本应用窗口不该被动
        let quiet = KeyEvent { selftest_active: false, ..ev };
        assert!(matches!(
            decide(&cfg, &quiet, false),
            Decision::Skip { reason: SkipReason::NoScenario, .. }
        ));
    }

    #[test]
    fn normalize_cleans_dirty_values() {
        let mut cfg = Config {
            suffix: "   ".into(),
            inject_delay_ms: 9999,
            scenarios: vec![Scenario {
                processes: vec![" QQ.exe ".into(), "qq.EXE".into(), "".into()],
                suffix: Some("  ".into()),
                ..qq_scenario(true)
            }],
            ..Config::default()
        };
        cfg.normalize();
        assert_eq!(cfg.suffix, "喵~");
        assert_eq!(cfg.inject_delay_ms, 2000);
        assert_eq!(cfg.scenarios[0].processes, vec!["qq.exe".to_string()]);
        assert_eq!(cfg.scenarios[0].suffix, None);
    }
}
