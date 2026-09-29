//! 全局热键：开/关总开关（默认 `Ctrl+Alt+'`）。
//!
//! 用 `RegisterHotKey` 而不是再挂一个键盘钩子：
//! - **不会吃掉你的按键**：不匹配的按键系统根本不通知我们，只有真正命中那一下才收到
//!   `WM_HOTKEY`（键盘钩子则是每次按键都要过一遍我们的回调，多一份被拖慢甚至卡输入的风险）；
//! - 注册失败（组合键被别的程序占了）能立刻知道，可以在界面上说出来；
//! - 需要管理员权限的目标窗口（比如以管理员身份跑的 QQ）也能收到——热键是系统级的。
//!
//! 线程归属：`RegisterHotKey` 注册在**调用它的那个线程**上，`WM_HOTKEY` 也只投递到那个线程的
//! 消息队列。所以这里有一个专属线程：既跑消息循环，也收「换一个热键」的指令。

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 默认热键：Ctrl+Alt+'（`'` 在大多数键盘布局上是 `VK_OEM_7`，中文输入法下也好按）。
pub const DEFAULT_HOTKEY: &str = "ctrl+alt+'";

/// 修饰键在配置里的写法和展示顺序。
const MODIFIER_NAMES: [&str; 4] = ["ctrl", "alt", "shift", "win"];

/// 辅助键 → `(配置里的小写名, 虚拟键码)`。
///
/// 虚拟键码在 Windows 上是稳定契约（`VK_OEM_7 = 0xDE` 之类），这里刻意写死数值，
/// 免得把整个 `windows` crate 拖进纯逻辑测试。
const KEYS: &[(&str, u16)] = &[
    ("a", 0x41),
    ("b", 0x42),
    ("c", 0x43),
    ("d", 0x44),
    ("e", 0x45),
    ("f", 0x46),
    ("g", 0x47),
    ("h", 0x48),
    ("i", 0x49),
    ("j", 0x4A),
    ("k", 0x4B),
    ("l", 0x4C),
    ("m", 0x4D),
    ("n", 0x4E),
    ("o", 0x4F),
    ("p", 0x50),
    ("q", 0x51),
    ("r", 0x52),
    ("s", 0x53),
    ("t", 0x54),
    ("u", 0x55),
    ("v", 0x56),
    ("w", 0x57),
    ("x", 0x58),
    ("y", 0x59),
    ("z", 0x5A),
    ("0", 0x30),
    ("1", 0x31),
    ("2", 0x32),
    ("3", 0x33),
    ("4", 0x34),
    ("5", 0x35),
    ("6", 0x36),
    ("7", 0x37),
    ("8", 0x38),
    ("9", 0x39),
    ("f1", 0x70),
    ("f2", 0x71),
    ("f3", 0x72),
    ("f4", 0x73),
    ("f5", 0x74),
    ("f6", 0x75),
    ("f7", 0x76),
    ("f8", 0x77),
    ("f9", 0x78),
    ("f10", 0x79),
    ("f11", 0x7A),
    ("f12", 0x7B),
    ("f13", 0x7C),
    ("f14", 0x7D),
    ("f15", 0x7E),
    ("f16", 0x7F),
    ("f17", 0x80),
    ("f18", 0x81),
    ("f19", 0x82),
    ("f20", 0x83),
    ("f21", 0x84),
    ("f22", 0x85),
    ("f23", 0x86),
    ("f24", 0x87),
    ("space", 0x20),
    ("tab", 0x09),
    ("enter", 0x0D),
    ("backspace", 0x08),
    ("insert", 0x2D),
    ("delete", 0x2E),
    ("home", 0x24),
    ("end", 0x23),
    ("pageup", 0x21),
    ("pagedown", 0x22),
    ("up", 0x26),
    ("down", 0x28),
    ("left", 0x25),
    ("right", 0x27),
    // 符号：名字就是键帽上那个字符
    ("`", 0xC0),
    ("-", 0xBD),
    ("=", 0xBB),
    ("[", 0xDB),
    ("]", 0xDD),
    ("\\", 0xDC),
    (";", 0xBA),
    ("'", 0xDE),
    (",", 0xBC),
    (".", 0xBE),
    ("/", 0xBF),
];

/// 解析成 `(修饰键, 主键)`；格式不对就返回 `None`。
///
/// 规则：至少一个修饰键 + 恰好一个主键，主键不能又是修饰键
/// （否则 `Ctrl+Alt` 这种半截组合会被当成合法热键，注册上却永远触发不了）。
pub fn parse(text: &str) -> Option<(Vec<&'static str>, &'static str)> {
    let mut modifiers: Vec<&'static str> = Vec::new();
    let mut key: Option<&'static str> = None;

    for raw in text.split('+') {
        let part = raw.trim().to_ascii_lowercase();
        if part.is_empty() {
            return None;
        }
        if let Some(name) = MODIFIER_NAMES.iter().find(|name| **name == part) {
            if !modifiers.contains(name) {
                modifiers.push(name);
            }
            continue;
        }
        // 主键只认已知的名字（`f1`、`space`、`'` …）
        let (name, _) = KEYS.iter().find(|(name, _)| *name == part)?;
        if key.is_some() {
            return None; // 两个主键
        }
        key = Some(name);
    }

    let key = key?;
    if modifiers.is_empty() {
        return None; // 光一个键不叫热键
    }
    // 统一顺序：`shift+ctrl+a` 和 `ctrl+shift+a` 存成同一个样子
    modifiers.sort_by_key(|name| MODIFIER_NAMES.iter().position(|n| n == name).unwrap_or(0));
    Some((modifiers, key))
}

/// 把任意写法规范成 `ctrl+alt+'` 这种形式；不合法就是 `None`。
pub fn canonical(text: &str) -> Option<String> {
    let (mut parts, key) = parse(text)?;
    parts.push(key);
    Some(parts.join("+"))
}

/// 给界面看的写法：`Ctrl+Alt+'`。
pub fn display(text: &str) -> Option<String> {
    let (modifiers, key) = parse(text)?;
    let mut parts: Vec<String> = modifiers
        .iter()
        .map(|name| match *name {
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            _ => "Win".to_string(),
        })
        .collect();

    parts.push(match key {
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "enter" => "Enter".to_string(),
        "backspace" => "Backspace".to_string(),
        "insert" => "Insert".to_string(),
        "delete" => "Delete".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        other => other.to_ascii_uppercase(),
    });
    Some(parts.join("+"))
}

/// 这个键有没有对应的虚拟键码（`RegisterHotKey` 要的就是它）。
pub fn virtual_key(key: &str) -> Option<u16> {
    KEYS.iter().find(|(name, _)| *name == key).map(|(_, code)| *code)
}

/// 跑热键的线程句柄：能换热键，也能问当前注册的是哪个。
pub struct HotkeyThread {
    commands: Sender<(String, Sender<Result<(), String>>)>,
    registered: Arc<Mutex<Option<String>>>,
}

impl HotkeyThread {
    /// 起线程并注册第一个热键；`on_fire` 命中时执行（在热键线程上跑，别在里面做重活）。
    ///
    /// 返回 `(句柄, 首次注册的失败原因)`；传空字符串表示先不注册。
    pub fn spawn<F>(hotkey: String, on_fire: F) -> (Self, Option<String>)
    where
        F: Fn() + Send + 'static,
    {
        let (commands, incoming) = channel::<(String, Sender<Result<(), String>>)>();
        let (ready_tx, ready_rx) = channel::<Option<String>>();
        let registered = Arc::new(Mutex::new(None));

        let state = Arc::clone(&registered);
        std::thread::Builder::new()
            .name("meow-hotkey".into())
            .spawn(move || run_message_loop(incoming, ready_tx, hotkey, state, Box::new(on_fire)))
            .expect("启动热键线程失败");

        let first = ready_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|_| Some("热键线程没在超时前就绪".to_string()));

        (Self { commands, registered }, first)
    }

    /// 换一个热键（空字符串 = 取消）。同步等结果，界面好即时显示成功还是被占用。
    pub fn set(&self, hotkey: String) -> Result<(), String> {
        let (done_tx, done_rx) = channel::<Result<(), String>>();
        self.commands
            .send((hotkey, done_tx))
            .map_err(|_| "热键线程已经不在了".to_string())?;
        done_rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|_| Err("等待热键线程响应超时".to_string()))
    }

    /// 当前真正注册着的热键。
    pub fn current(&self) -> Option<String> {
        self.registered.lock().ok().and_then(|value| value.clone())
    }
}

#[cfg(windows)]
fn run_message_loop(
    incoming: Receiver<(String, Sender<Result<(), String>>)>,
    ready: Sender<Option<String>>,
    first: String,
    registered: Arc<Mutex<Option<String>>>,
    on_fire: Box<dyn Fn() + Send>,
) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
    };
    use windows::Win32::UI::WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY};

    /// 本线程内的热键 id，随便取一个不冲突的就行。
    const HOTKEY_ID: i32 = 1;

    /// 把配置里的写法真正注册到系统。
    fn apply(text: &str) -> Result<(), String> {
        let (modifiers, key) = parse(text).ok_or_else(|| format!("热键写法不认识：{text}"))?;
        let code = virtual_key(key).ok_or_else(|| format!("这个键不能当热键：{key}"))?;

        let mut flags = MOD_NOREPEAT; // 按住不放也只触发一次
        for modifier in modifiers {
            flags |= match modifier {
                "ctrl" => MOD_CONTROL,
                "alt" => MOD_ALT,
                "shift" => MOD_SHIFT,
                _ => MOD_WIN,
            };
        }
        unsafe { RegisterHotKey(None, HOTKEY_ID, flags, code as u32) }
            .map_err(|err| format!("{} 注册失败：{err}", display(text).unwrap_or_else(|| text.to_string())))
    }

    let mut current = String::new();
    let mut first_error = None;
    if !first.is_empty() {
        match apply(&first) {
            Ok(()) => {
                current = first.clone();
                if let Ok(mut slot) = registered.lock() {
                    *slot = Some(first);
                }
            }
            Err(err) => first_error = Some(err),
        }
    }
    let _ = ready.send(first_error);

    let mut message = MSG::default();
    loop {
        // 先处理「换热键」的指令（不阻塞）
        while let Ok((hotkey, done)) = incoming.try_recv() {
            if !current.is_empty() {
                unsafe {
                    let _ = UnregisterHotKey(None, HOTKEY_ID);
                }
                current.clear();
                if let Ok(mut slot) = registered.lock() {
                    *slot = None;
                }
            }
            let outcome = if hotkey.is_empty() {
                Ok(())
            } else {
                apply(&hotkey).map(|()| {
                    current = hotkey.clone();
                    if let Ok(mut slot) = registered.lock() {
                        *slot = Some(hotkey.clone());
                    }
                })
            };
            let _ = done.send(outcome);
        }

        // 看有没有 WM_HOTKEY；没有就回去看指令。
        // 这里用 PeekMessage 轮询而不是阻塞式 GetMessage：
        // 阻塞在消息队列上就没法及时响应换热键了，25ms 的空转代价可以忽略。
        let has_message = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) };
        if has_message.as_bool() {
            if message.message == WM_HOTKEY {
                on_fire();
            }
            continue;
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(not(windows))]
fn run_message_loop(
    _incoming: Receiver<(String, Sender<Result<(), String>>)>,
    ready: Sender<Option<String>>,
    _first: String,
    _registered: Arc<Mutex<Option<String>>>,
    _on_fire: Box<dyn Fn() + Send>,
) {
    let _ = ready.send(Some("只有在 Windows 上才能注册全局热键".to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkey_is_ctrl_alt_quote() {
        let (modifiers, key) = parse(DEFAULT_HOTKEY).expect("默认热键必须能解析");
        assert_eq!(modifiers, vec!["ctrl", "alt"]);
        assert_eq!(key, "'");
        assert_eq!(virtual_key(key), Some(0xDE), "`'` 应当是 VK_OEM_7");
        assert_eq!(canonical(DEFAULT_HOTKEY).as_deref(), Some(DEFAULT_HOTKEY));
    }

    #[test]
    fn parsing_is_case_and_order_insensitive() {
        assert_eq!(canonical("Ctrl+Alt+'").as_deref(), Some("ctrl+alt+'"));
        assert_eq!(canonical("alt+ctrl+'").as_deref(), Some("ctrl+alt+'"));
        assert_eq!(canonical("  CTRL + ALT + '  ").as_deref(), Some("ctrl+alt+'"));
    }

    #[test]
    fn single_key_without_modifier_is_rejected() {
        assert_eq!(canonical("a"), None);
        assert_eq!(canonical("f5"), None);
        assert_eq!(canonical("'"), None);
    }

    #[test]
    fn incomplete_or_unknown_combinations_are_rejected() {
        assert_eq!(canonical("ctrl+alt"), None, "只有修饰键不算热键");
        assert_eq!(canonical("ctrl"), None);
        assert_eq!(canonical(""), None);
        assert_eq!(canonical("apostrophe"), None);
        assert_eq!(canonical("ctrl+alt+nosuchkey"), None);
        assert_eq!(canonical("ctrl+alt+a+b"), None, "两个主键");
    }

    #[test]
    fn modifiers_are_deduplicated_and_ordered() {
        assert_eq!(canonical("alt+shift+ctrl+x").as_deref(), Some("ctrl+alt+shift+x"));
        assert_eq!(canonical("ctrl+ctrl+a").as_deref(), Some("ctrl+a"));
    }

    #[test]
    fn display_uses_readable_names() {
        assert_eq!(display("ctrl+alt+'").as_deref(), Some("Ctrl+Alt+'"));
        assert_eq!(display("shift+f5").as_deref(), Some("Shift+F5"));
        assert_eq!(display("win+space").as_deref(), Some("Win+Space"));
        assert_eq!(display("ctrl+left").as_deref(), Some("Ctrl+←"));
        assert_eq!(display("garbage"), None);
    }

    #[test]
    fn every_named_key_has_a_virtual_code() {
        for (name, code) in KEYS {
            assert!(virtual_key(name).is_some(), "{name} 没有虚拟键码");
            assert!(*code > 0, "{name} 的虚拟键码是 0");
        }
    }
}
