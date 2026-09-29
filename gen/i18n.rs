// 由 `meow-text --export-ui`（或 build.rs）从 locales/*.toml 生成，请勿手改。

/// 本次编译的语言包列表。
pub struct Locale {
    pub tag: &'static str,
    /// 该语言的自称（任何语言下都显示这个）
    pub name: &'static str,
    /// 与 `KEYS` 一一对应的文案
    pub strings: &'static [&'static str],
}

/// 全部文案键，索引即 `Key` 的内层值。
pub const KEYS: &[&str] = &[
    "advanced.hint",
    "app.brand",
    "app.html_lang",
    "app.logo",
    "app.switch_off",
    "app.switch_on",
    "app.switch_title",
    "app.tagline",
    "app.window_title",
    "close.body",
    "close.hide",
    "close.quit",
    "close.remember",
    "close.title",
    "close_action.ask",
    "close_action.hide",
    "close_action.quit",
    "footer.close_ask",
    "footer.close_hide",
    "footer.close_quit",
    "footer.hide",
    "footer.quit",
    "footer.ready",
    "footer.save_failed",
    "footer.saved",
    "footer.saving",
    "language.label",
    "language.system",
    "locale.name",
    "log.already_running",
    "log.appended",
    "log.appended_clipboard",
    "log.autostart_failed",
    "log.autostart_off",
    "log.autostart_on",
    "log.autostart_started",
    "log.autostart_synced",
    "log.clipboard_failed",
    "log.dry_run",
    "log.enter_failed",
    "log.focus_lost",
    "log.hidden",
    "log.hook_failed",
    "log.hook_mounted",
    "log.hook_unmounted",
    "log.hotkey_disabled",
    "log.hotkey_enabled",
    "log.hotkey_failed",
    "log.hotkey_off",
    "log.hotkey_on",
    "log.hotkey_ready",
    "log.selftest",
    "log.selftest_dry_run",
    "log.selftest_failed",
    "log.selftest_ok",
    "log.skip",
    "log.system",
    "log.type_failed",
    "logs.clear",
    "logs.config_path",
    "logs.empty",
    "logs.hint",
    "logs.inject_once",
    "logs.locale",
    "logs.title",
    "main.open_logs",
    "main.recent",
    "main.recent_hint",
    "main.status",
    "main.status_hint",
    "main.toggle_hint",
    "mode.clipboard",
    "mode.type",
    "nav.advanced",
    "nav.collapse",
    "nav.expand",
    "nav.logs",
    "nav.main",
    "nav.preferences",
    "nav.section",
    "preferences.appearance",
    "preferences.autostart",
    "preferences.autostart_hint",
    "preferences.autostart_off",
    "preferences.autostart_on",
    "preferences.hint",
    "preferences.hotkey",
    "preferences.hotkey_error_format",
    "preferences.hotkey_error_taken",
    "preferences.hotkey_hint",
    "preferences.hotkey_off",
    "preferences.hotkey_recording",
    "preferences.hotkey_reset",
    "preferences.language",
    "preferences.window",
    "scenario.qq.name",
    "scenario.qq.note",
    "scenario.tim.name",
    "scenario.tim.note",
    "scenario.wechat.name",
    "scenario.wechat.note",
    "scenarios.add",
    "scenarios.builtin",
    "scenarios.col_processes",
    "scenarios.col_suffix",
    "scenarios.col_trigger",
    "scenarios.delete",
    "scenarios.hint",
    "scenarios.name_placeholder",
    "scenarios.new_name",
    "scenarios.processes_placeholder",
    "scenarios.suffix_placeholder",
    "scenarios.title",
    "scenarios.trigger_default",
    "scenarios.untitled",
    "selftest.hint",
    "selftest.placeholder",
    "selftest.title",
    "settings.close_action",
    "settings.delay",
    "settings.dry_run",
    "settings.inject_mode",
    "settings.require_content",
    "settings.skip_composing",
    "settings.suffix",
    "settings.suffix_placeholder",
    "settings.title",
    "settings.trigger",
    "skip.composing",
    "skip.disabled",
    "skip.empty_input",
    "skip.injected",
    "skip.modifier",
    "skip.no_scenario",
    "skip.not_trigger",
    "skip.scenario_disabled",
    "status.foreground",
    "status.foreground_empty",
    "status.foreground_hit",
    "status.foreground_label",
    "status.hook_checking",
    "status.hook_failed",
    "status.hook_label",
    "status.hook_missing",
    "status.hook_mounted",
    "status.hook_preview",
    "status.no_tauri",
    "status.reset",
    "status.theme",
    "status.theme_system",
    "status.times",
    "status.today",
    "status.total",
    "summary.disabled",
    "summary.enabled",
    "summary.line",
    "summary.no_scenarios",
    "theme.dark",
    "theme.group_label",
    "theme.light",
    "theme.system",
    "tray.quit",
    "tray.show",
    "tray.tooltip",
    "trigger.ctrl_enter",
    "trigger.enter",
];

/// 编译时的默认语言（首屏用它，找不到配置时也用它）。
pub const DEFAULT_LOCALE: &str = "zh-CN";

pub static LOCALES: &[Locale] = &[
    Locale {
        tag: "en-US",
        name: "English",
        strings: &[
            "Trigger, injection and scenario matching. Defaults are fine unless you know you need more.",
            "meow-text",
            "en",
            "M",
            "Off",
            "On",
            "Master switch",
            "Appends “喵~” to messages you send in QQ",
            "meow-text · Meow Control Panel",
            "“Hide to taskbar” keeps meowing alive in the background; reopen this panel from the tray icon on the taskbar afterwards.",
            "Hide to taskbar",
            "Quit",
            "Don't ask again",
            "Close meow-text?",
            "Ask every time",
            "Hide to taskbar",
            "Quit",
            "The × button asks first: hide to the taskbar or quit",
            "The × button hides to the taskbar; meowing keeps running",
            "The × button quits immediately and meowing stops",
            "Hide to background",
            "Quit meow-text",
            "Ready",
            "Save failed: {error}",
            "Saved {time}",
            "Saving…",
            "Language",
            "System ({name})",
            "English",
            "Already running in the background — brought the panel back",
            "Appended “{suffix}”",
            "Appended “{suffix}” via the clipboard",
            "Failed to change the startup entry: {error}",
            "Removed from startup",
            "Added to startup: {path}",
            "Launched at sign-in, so the panel stays in the tray",
            "The startup entry pointed somewhere else; updated it to this exe",
            "Clipboard injection failed: {error}",
            "Dry run: send key detected, did not inject “{suffix}”",
            "Replaying the send key failed: {error}",
            "Cancelled: focus moved elsewhere",
            "Hidden to the taskbar; meowing keeps running in the background",
            "Failed to install the keyboard hook: {error}",
            "Keyboard hook mounted, waiting for Enter in QQ",
            "Keyboard hook removed",
            "Hotkey switched meowing off",
            "Hotkey switched meowing on",
            "Failed to register the hotkey: {error}",
            "Hotkey turned off",
            "Hotkey set to {hotkey}",
            "Hotkey ready: {hotkey} toggles the master switch",
            "Self-test",
            "Dry run: skipped the real injection",
            "Injection failed: {error}",
            "Injected “{suffix}”",
            "Skipped: {reason}",
            "System",
            "Typing the suffix failed: {error}",
            "Clear",
            "Config: {path} · Process: {process}",
            "No events yet. Type something in QQ and press Enter.",
            "Every trigger, skip and failure lands here, newest first, with no length limit.",
            "Inject into focused window",
            "Locale: {locale}",
            "Logs",
            "Open full log",
            "Recent events",
            "Only the newest few entries; the full list lives under “Logs”.",
            "Current state",
            "The master switch decides whether meowing happens at all; turn it off and every scenario stops.",
            "With the master switch off the hook stays installed but no input is touched.",
            "Paste via clipboard (fallback)",
            "Type text (recommended)",
            "Advanced",
            "Collapse navigation",
            "Expand navigation",
            "Logs",
            "Main",
            "Preferences",
            "Navigate",
            "Appearance",
            "Start at sign-in",
            "Runs in the background after you sign in to Windows, sitting in the taskbar tray without opening the panel.",
            "Not registered",
            "Registered: {command}",
            "Appearance, interface language and window behaviour. Everything applies immediately and saves itself.",
            "Toggle hotkey",
            "A hotkey needs at least one modifier (Ctrl / Alt / Shift / Win)",
            "Another program already owns that combination — try another one",
            "Click, then press a combination (needs at least one modifier, e.g. Ctrl+Alt+'). Esc cancels.",
            "Not set",
            "Recording: press the combination…",
            "Reset to default",
            "Interface language",
            "Window",
            "QQ",
            "Main process of QQNT / classic QQ",
            "TIM",
            "TIM shares QQ's engine; enable it separately",
            "WeChat",
            "WeChat 4.x runs as weixin.exe",
            "+ Add app",
            "Built-in",
            "Process names",
            "Suffix",
            "Send key",
            "Remove",
            "A scenario is one kind of app to meow: it matches by process name (the name you see in Task Manager, e.g. <code>qq.exe</code>).",
            "Name",
            "New scenario",
            "Process names, comma separated",
            "Default {suffix}",
            "Scenarios",
            "Use default",
            "Untitled",
            "Type a few characters in the box below and press <b>Enter</b>: “喵~” should show up before the newline. This never touches QQ, it only proves the hook and the injection path work.",
            "Click here → type → press Enter",
            "Self-test",
            "On window close",
            "Delay (ms)",
            "Dry run (log only, no injection)",
            "Injection",
            "Skip when the input box is empty",
            "Leave Enter alone while the IME is composing",
            "Suffix",
            "喵~",
            "Advanced settings",
            "Send key",
            "the IME is composing",
            "master switch is off",
            "the input box looks empty",
            "synthetic key from another program",
            "modifier state does not match the send key",
            "foreground process is not in any scenario",
            "not the send key",
            "that scenario is disabled",
            "{process}",
            "—",
            "{process} (scenario match)",
            "Foreground window",
            "Checking…",
            "Failed ({error})",
            "Keyboard hook",
            "Not mounted",
            "Mounted",
            "Browser preview (Tauri not connected)",
            "Tauri not connected",
            "Reset",
            "Theme: {name}",
            "Theme: {name} (system)",
            "",
            "Today",
            "Total",
            "disabled",
            "enabled",
            "{state} · suffix {suffix} · {trigger} · {mode} · scenarios: {scenarios}",
            "(none enabled)",
            "Dark",
            "Theme",
            "Light",
            "System",
            "Quit meow-text",
            "Show panel",
            "meow-text · meowing is running",
            "Ctrl+Enter",
            "Enter",
        ],
    },
    Locale {
        tag: "zh-CN",
        name: "简体中文",
        strings: &[
            "触发方式、注入方式与场景匹配。不确定时保持默认就好。",
            "meow-text",
            "zh-CN",
            "喵",
            "已停用",
            "已启用",
            "总开关",
            "在 QQ（其实不止） 里发消息，句尾自动补上「喵~」",
            "喵化输出 · 配置",
            "「隐藏到任务栏」会让喵化继续在后台生效，之后可以从任务栏的托盘图标重新打开这个面板。",
            "隐藏到任务栏",
            "关闭程序",
            "不再提示",
            "要关闭“喵化输出”吗？",
            "每次询问",
            "隐藏到任务栏",
            "退出程序",
            "隐藏到任务栏，或是关闭程序",
            "隐藏到任务栏，喵化将继续在后台运行",
            "直接退出程序",
            "隐藏到后台",
            "退出程序",
            "已就绪",
            "保存失败：{error}",
            "已保存 {time}",
            "保存中…",
            "界面语言",
            "跟随系统（{name}）",
            "简体中文",
            "已经在后台运行，忽略",
            "已补上「{suffix}」",
            "已通过剪贴板补上「{suffix}」",
            "设置开机自启失败：{error}",
            "已取消开机自启",
            "已加入开机自启：{path}",
            "这次是开机自启拉起来的，面板收在托盘里",
            "开机自启登记的路径和现在不一致，已改成当前这个 exe",
            "剪贴板注入失败：{error}",
            "演练：检测到发送键，未注入「{suffix}」",
            "发送键重放失败：{error}",
            "已取消：焦点已经切走",
            "已隐藏到任务栏，喵化继续在后台运行",
            "安装键盘钩子失败：{error}",
            "键盘钩子已挂载，等待 QQ 里的回车",
            "键盘钩子已卸载",
            "热键关闭了总开关",
            "热键打开了总开关",
            "热键注册失败：{error}",
            "热键已关闭",
            "热键已设为 {hotkey}",
            "热键已就绪：{hotkey} 可以开/关总开关",
            "自测",
            "演练模式：跳过真实注入",
            "注入失败：{error}",
            "已注入「{suffix}」",
            "跳过：{reason}",
            "系统",
            "逐字注入失败：{error}",
            "清空",
            "配置文件：{path} · 本进程：{process}",
            "还没有事件。去打几个字按回车试试。",
            "每次触发、跳过或失败都记在这里，最新的位于顶上，没有条数上限。",
            "往当前焦点试注入",
            "语言：{locale}",
            "日志记录",
            "查看完整日志",
            "最近事件",
            "只显示最近几条；完整记录在左侧的「日志记录」里。",
            "当前状态",
            "总开关控制喵化是否生效，关掉之后所有场景都会停下来。",
            "关掉总开关后，钩子仍在，但不再改动任何输入。",
            "剪贴板粘贴",
            "逐字输入",
            "高级",
            "收起导航",
            "展开导航",
            "日志记录",
            "主面板",
            "偏好",
            "导航",
            "外观",
            "开机自动启动",
            "登录 Windows 后自动在后台运行，将不会弹出配置面板。",
            "当前未登记",
            "已登记：{command}",
            "外观、界面语言与窗口行为。这些设置都立刻生效，并会自动保存。",
            "开关热键",
            "热键至少要带一个修饰键（Ctrl / Alt / Shift / Win）",
            "这个组合键被别的程序占用了，换一个试试",
            "点一下再按组合键（至少要带一个修饰键，例如 Ctrl+Alt+'）。按 Esc 取消。",
            "未设置",
            "正在录制：请按下组合键…",
            "恢复默认",
            "界面语言",
            "窗口",
            "QQ",
            "QQNT / 经典版 QQ 的主进程",
            "TIM",
            "TIM 与 QQ 同源，需要单独开启",
            "微信",
            "微信 4.x 的进程名是 weixin.exe",
            "+ 添加应用",
            "内置",
            "进程名",
            "后缀",
            "触发键",
            "删除",
            "一个场景，即一类要喵化的应用，靠进程名匹配（任务管理器/详细信息中查看，例如 qq.exe）。",
            "名称",
            "新场景",
            "进程名，逗号分隔",
            "默认 {suffix}",
            "场景",
            "跟随后缀默认",
            "未命名",
            "在下面的文本框中输入后再按 Enter，预期将先出现「喵~」再换行。\n用于确认钩子与注入链路是通的。",
            "请输入文本",
            "自测",
            "关闭窗口时",
            "注入延迟（毫秒）",
            "测试模式（此模式下，不执行实际注入）",
            "注入方式",
            "输入框为空时不追加",
            "输入法组词时不动回车",
            "句尾后缀",
            "喵~",
            "高级设置",
            "发送键",
            "输入法正在组词",
            "总开关未开启",
            "输入框似乎是空的",
            "其他程序模拟的按键",
            "修饰键状态与触发键不匹配",
            "前台进程不在场景列表",
            "不是发送键",
            "该场景已关闭",
            "{process}",
            "—",
            "{process}（命中场景）",
            "前台窗口",
            "检测中…",
            "挂载失败（{error}）",
            "键盘钩子",
            "未挂载",
            "已挂载",
            "浏览器预览（未连接 Tauri）",
            "未连接 Tauri",
            "清零",
            "主题：{name}",
            "主题：{name}（跟随系统）",
            " 次",
            "今日",
            "累计",
            "已停用",
            "已启用",
            "{state} · 后缀 {suffix} · {trigger} · {mode} · 场景：{scenarios}",
            "（没有启用任何场景）",
            "黑夜",
            "主题",
            "白天",
            "跟随系统",
            "退出喵化",
            "显示控制面板",
            "喵化输出",
            "Ctrl+Enter",
            "Enter",
        ],
    },
];

/// `advanced.hint`
pub const ADVANCED_HINT: Key = Key(0);
/// `app.brand`
pub const APP_BRAND: Key = Key(1);
/// `app.html_lang`
pub const APP_HTML_LANG: Key = Key(2);
/// `app.logo`
pub const APP_LOGO: Key = Key(3);
/// `app.switch_off`
pub const APP_SWITCH_OFF: Key = Key(4);
/// `app.switch_on`
pub const APP_SWITCH_ON: Key = Key(5);
/// `app.switch_title`
pub const APP_SWITCH_TITLE: Key = Key(6);
/// `app.tagline`
pub const APP_TAGLINE: Key = Key(7);
/// `app.window_title`
pub const APP_WINDOW_TITLE: Key = Key(8);
/// `close.body`
pub const CLOSE_BODY: Key = Key(9);
/// `close.hide`
pub const CLOSE_HIDE: Key = Key(10);
/// `close.quit`
pub const CLOSE_QUIT: Key = Key(11);
/// `close.remember`
pub const CLOSE_REMEMBER: Key = Key(12);
/// `close.title`
pub const CLOSE_TITLE: Key = Key(13);
/// `close_action.ask`
pub const CLOSE_ACTION_ASK: Key = Key(14);
/// `close_action.hide`
pub const CLOSE_ACTION_HIDE: Key = Key(15);
/// `close_action.quit`
pub const CLOSE_ACTION_QUIT: Key = Key(16);
/// `footer.close_ask`
pub const FOOTER_CLOSE_ASK: Key = Key(17);
/// `footer.close_hide`
pub const FOOTER_CLOSE_HIDE: Key = Key(18);
/// `footer.close_quit`
pub const FOOTER_CLOSE_QUIT: Key = Key(19);
/// `footer.hide`
pub const FOOTER_HIDE: Key = Key(20);
/// `footer.quit`
pub const FOOTER_QUIT: Key = Key(21);
/// `footer.ready`
pub const FOOTER_READY: Key = Key(22);
/// `footer.save_failed`
pub const FOOTER_SAVE_FAILED: Key = Key(23);
/// `footer.saved`
pub const FOOTER_SAVED: Key = Key(24);
/// `footer.saving`
pub const FOOTER_SAVING: Key = Key(25);
/// `language.label`
pub const LANGUAGE_LABEL: Key = Key(26);
/// `language.system`
pub const LANGUAGE_SYSTEM: Key = Key(27);
/// `locale.name`
pub const LOCALE_NAME: Key = Key(28);
/// `log.already_running`
pub const LOG_ALREADY_RUNNING: Key = Key(29);
/// `log.appended`
pub const LOG_APPENDED: Key = Key(30);
/// `log.appended_clipboard`
pub const LOG_APPENDED_CLIPBOARD: Key = Key(31);
/// `log.autostart_failed`
pub const LOG_AUTOSTART_FAILED: Key = Key(32);
/// `log.autostart_off`
pub const LOG_AUTOSTART_OFF: Key = Key(33);
/// `log.autostart_on`
pub const LOG_AUTOSTART_ON: Key = Key(34);
/// `log.autostart_started`
pub const LOG_AUTOSTART_STARTED: Key = Key(35);
/// `log.autostart_synced`
pub const LOG_AUTOSTART_SYNCED: Key = Key(36);
/// `log.clipboard_failed`
pub const LOG_CLIPBOARD_FAILED: Key = Key(37);
/// `log.dry_run`
pub const LOG_DRY_RUN: Key = Key(38);
/// `log.enter_failed`
pub const LOG_ENTER_FAILED: Key = Key(39);
/// `log.focus_lost`
pub const LOG_FOCUS_LOST: Key = Key(40);
/// `log.hidden`
pub const LOG_HIDDEN: Key = Key(41);
/// `log.hook_failed`
pub const LOG_HOOK_FAILED: Key = Key(42);
/// `log.hook_mounted`
pub const LOG_HOOK_MOUNTED: Key = Key(43);
/// `log.hook_unmounted`
pub const LOG_HOOK_UNMOUNTED: Key = Key(44);
/// `log.hotkey_disabled`
pub const LOG_HOTKEY_DISABLED: Key = Key(45);
/// `log.hotkey_enabled`
pub const LOG_HOTKEY_ENABLED: Key = Key(46);
/// `log.hotkey_failed`
pub const LOG_HOTKEY_FAILED: Key = Key(47);
/// `log.hotkey_off`
pub const LOG_HOTKEY_OFF: Key = Key(48);
/// `log.hotkey_on`
pub const LOG_HOTKEY_ON: Key = Key(49);
/// `log.hotkey_ready`
pub const LOG_HOTKEY_READY: Key = Key(50);
/// `log.selftest`
pub const LOG_SELFTEST: Key = Key(51);
/// `log.selftest_dry_run`
pub const LOG_SELFTEST_DRY_RUN: Key = Key(52);
/// `log.selftest_failed`
pub const LOG_SELFTEST_FAILED: Key = Key(53);
/// `log.selftest_ok`
pub const LOG_SELFTEST_OK: Key = Key(54);
/// `log.skip`
pub const LOG_SKIP: Key = Key(55);
/// `log.system`
pub const LOG_SYSTEM: Key = Key(56);
/// `log.type_failed`
pub const LOG_TYPE_FAILED: Key = Key(57);
/// `logs.clear`
pub const LOGS_CLEAR: Key = Key(58);
/// `logs.config_path`
pub const LOGS_CONFIG_PATH: Key = Key(59);
/// `logs.empty`
pub const LOGS_EMPTY: Key = Key(60);
/// `logs.hint`
pub const LOGS_HINT: Key = Key(61);
/// `logs.inject_once`
pub const LOGS_INJECT_ONCE: Key = Key(62);
/// `logs.locale`
pub const LOGS_LOCALE: Key = Key(63);
/// `logs.title`
pub const LOGS_TITLE: Key = Key(64);
/// `main.open_logs`
pub const MAIN_OPEN_LOGS: Key = Key(65);
/// `main.recent`
pub const MAIN_RECENT: Key = Key(66);
/// `main.recent_hint`
pub const MAIN_RECENT_HINT: Key = Key(67);
/// `main.status`
pub const MAIN_STATUS: Key = Key(68);
/// `main.status_hint`
pub const MAIN_STATUS_HINT: Key = Key(69);
/// `main.toggle_hint`
pub const MAIN_TOGGLE_HINT: Key = Key(70);
/// `mode.clipboard`
pub const MODE_CLIPBOARD: Key = Key(71);
/// `mode.type`
pub const MODE_TYPE: Key = Key(72);
/// `nav.advanced`
pub const NAV_ADVANCED: Key = Key(73);
/// `nav.collapse`
pub const NAV_COLLAPSE: Key = Key(74);
/// `nav.expand`
pub const NAV_EXPAND: Key = Key(75);
/// `nav.logs`
pub const NAV_LOGS: Key = Key(76);
/// `nav.main`
pub const NAV_MAIN: Key = Key(77);
/// `nav.preferences`
pub const NAV_PREFERENCES: Key = Key(78);
/// `nav.section`
pub const NAV_SECTION: Key = Key(79);
/// `preferences.appearance`
pub const PREFERENCES_APPEARANCE: Key = Key(80);
/// `preferences.autostart`
pub const PREFERENCES_AUTOSTART: Key = Key(81);
/// `preferences.autostart_hint`
pub const PREFERENCES_AUTOSTART_HINT: Key = Key(82);
/// `preferences.autostart_off`
pub const PREFERENCES_AUTOSTART_OFF: Key = Key(83);
/// `preferences.autostart_on`
pub const PREFERENCES_AUTOSTART_ON: Key = Key(84);
/// `preferences.hint`
pub const PREFERENCES_HINT: Key = Key(85);
/// `preferences.hotkey`
pub const PREFERENCES_HOTKEY: Key = Key(86);
/// `preferences.hotkey_error_format`
pub const PREFERENCES_HOTKEY_ERROR_FORMAT: Key = Key(87);
/// `preferences.hotkey_error_taken`
pub const PREFERENCES_HOTKEY_ERROR_TAKEN: Key = Key(88);
/// `preferences.hotkey_hint`
pub const PREFERENCES_HOTKEY_HINT: Key = Key(89);
/// `preferences.hotkey_off`
pub const PREFERENCES_HOTKEY_OFF: Key = Key(90);
/// `preferences.hotkey_recording`
pub const PREFERENCES_HOTKEY_RECORDING: Key = Key(91);
/// `preferences.hotkey_reset`
pub const PREFERENCES_HOTKEY_RESET: Key = Key(92);
/// `preferences.language`
pub const PREFERENCES_LANGUAGE: Key = Key(93);
/// `preferences.window`
pub const PREFERENCES_WINDOW: Key = Key(94);
/// `scenario.qq.name`
pub const SCENARIO_QQ_NAME: Key = Key(95);
/// `scenario.qq.note`
pub const SCENARIO_QQ_NOTE: Key = Key(96);
/// `scenario.tim.name`
pub const SCENARIO_TIM_NAME: Key = Key(97);
/// `scenario.tim.note`
pub const SCENARIO_TIM_NOTE: Key = Key(98);
/// `scenario.wechat.name`
pub const SCENARIO_WECHAT_NAME: Key = Key(99);
/// `scenario.wechat.note`
pub const SCENARIO_WECHAT_NOTE: Key = Key(100);
/// `scenarios.add`
pub const SCENARIOS_ADD: Key = Key(101);
/// `scenarios.builtin`
pub const SCENARIOS_BUILTIN: Key = Key(102);
/// `scenarios.col_processes`
pub const SCENARIOS_COL_PROCESSES: Key = Key(103);
/// `scenarios.col_suffix`
pub const SCENARIOS_COL_SUFFIX: Key = Key(104);
/// `scenarios.col_trigger`
pub const SCENARIOS_COL_TRIGGER: Key = Key(105);
/// `scenarios.delete`
pub const SCENARIOS_DELETE: Key = Key(106);
/// `scenarios.hint`
pub const SCENARIOS_HINT: Key = Key(107);
/// `scenarios.name_placeholder`
pub const SCENARIOS_NAME_PLACEHOLDER: Key = Key(108);
/// `scenarios.new_name`
pub const SCENARIOS_NEW_NAME: Key = Key(109);
/// `scenarios.processes_placeholder`
pub const SCENARIOS_PROCESSES_PLACEHOLDER: Key = Key(110);
/// `scenarios.suffix_placeholder`
pub const SCENARIOS_SUFFIX_PLACEHOLDER: Key = Key(111);
/// `scenarios.title`
pub const SCENARIOS_TITLE: Key = Key(112);
/// `scenarios.trigger_default`
pub const SCENARIOS_TRIGGER_DEFAULT: Key = Key(113);
/// `scenarios.untitled`
pub const SCENARIOS_UNTITLED: Key = Key(114);
/// `selftest.hint`
pub const SELFTEST_HINT: Key = Key(115);
/// `selftest.placeholder`
pub const SELFTEST_PLACEHOLDER: Key = Key(116);
/// `selftest.title`
pub const SELFTEST_TITLE: Key = Key(117);
/// `settings.close_action`
pub const SETTINGS_CLOSE_ACTION: Key = Key(118);
/// `settings.delay`
pub const SETTINGS_DELAY: Key = Key(119);
/// `settings.dry_run`
pub const SETTINGS_DRY_RUN: Key = Key(120);
/// `settings.inject_mode`
pub const SETTINGS_INJECT_MODE: Key = Key(121);
/// `settings.require_content`
pub const SETTINGS_REQUIRE_CONTENT: Key = Key(122);
/// `settings.skip_composing`
pub const SETTINGS_SKIP_COMPOSING: Key = Key(123);
/// `settings.suffix`
pub const SETTINGS_SUFFIX: Key = Key(124);
/// `settings.suffix_placeholder`
pub const SETTINGS_SUFFIX_PLACEHOLDER: Key = Key(125);
/// `settings.title`
pub const SETTINGS_TITLE: Key = Key(126);
/// `settings.trigger`
pub const SETTINGS_TRIGGER: Key = Key(127);
/// `skip.composing`
pub const SKIP_COMPOSING: Key = Key(128);
/// `skip.disabled`
pub const SKIP_DISABLED: Key = Key(129);
/// `skip.empty_input`
pub const SKIP_EMPTY_INPUT: Key = Key(130);
/// `skip.injected`
pub const SKIP_INJECTED: Key = Key(131);
/// `skip.modifier`
pub const SKIP_MODIFIER: Key = Key(132);
/// `skip.no_scenario`
pub const SKIP_NO_SCENARIO: Key = Key(133);
/// `skip.not_trigger`
pub const SKIP_NOT_TRIGGER: Key = Key(134);
/// `skip.scenario_disabled`
pub const SKIP_SCENARIO_DISABLED: Key = Key(135);
/// `status.foreground`
pub const STATUS_FOREGROUND: Key = Key(136);
/// `status.foreground_empty`
pub const STATUS_FOREGROUND_EMPTY: Key = Key(137);
/// `status.foreground_hit`
pub const STATUS_FOREGROUND_HIT: Key = Key(138);
/// `status.foreground_label`
pub const STATUS_FOREGROUND_LABEL: Key = Key(139);
/// `status.hook_checking`
pub const STATUS_HOOK_CHECKING: Key = Key(140);
/// `status.hook_failed`
pub const STATUS_HOOK_FAILED: Key = Key(141);
/// `status.hook_label`
pub const STATUS_HOOK_LABEL: Key = Key(142);
/// `status.hook_missing`
pub const STATUS_HOOK_MISSING: Key = Key(143);
/// `status.hook_mounted`
pub const STATUS_HOOK_MOUNTED: Key = Key(144);
/// `status.hook_preview`
pub const STATUS_HOOK_PREVIEW: Key = Key(145);
/// `status.no_tauri`
pub const STATUS_NO_TAURI: Key = Key(146);
/// `status.reset`
pub const STATUS_RESET: Key = Key(147);
/// `status.theme`
pub const STATUS_THEME: Key = Key(148);
/// `status.theme_system`
pub const STATUS_THEME_SYSTEM: Key = Key(149);
/// `status.times`
pub const STATUS_TIMES: Key = Key(150);
/// `status.today`
pub const STATUS_TODAY: Key = Key(151);
/// `status.total`
pub const STATUS_TOTAL: Key = Key(152);
/// `summary.disabled`
pub const SUMMARY_DISABLED: Key = Key(153);
/// `summary.enabled`
pub const SUMMARY_ENABLED: Key = Key(154);
/// `summary.line`
pub const SUMMARY_LINE: Key = Key(155);
/// `summary.no_scenarios`
pub const SUMMARY_NO_SCENARIOS: Key = Key(156);
/// `theme.dark`
pub const THEME_DARK: Key = Key(157);
/// `theme.group_label`
pub const THEME_GROUP_LABEL: Key = Key(158);
/// `theme.light`
pub const THEME_LIGHT: Key = Key(159);
/// `theme.system`
pub const THEME_SYSTEM: Key = Key(160);
/// `tray.quit`
pub const TRAY_QUIT: Key = Key(161);
/// `tray.show`
pub const TRAY_SHOW: Key = Key(162);
/// `tray.tooltip`
pub const TRAY_TOOLTIP: Key = Key(163);
/// `trigger.ctrl_enter`
pub const TRIGGER_CTRL_ENTER: Key = Key(164);
/// `trigger.enter`
pub const TRIGGER_ENTER: Key = Key(165);
