//! 键盘事件注入：逐字 Unicode 输入、剪贴板粘贴、发送键重放。
//!
//! 所有注入事件都带 [`MAGIC`] 标记，钩子看到该标记一律放行，避免自己触发自己。

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_RETURN, VK_V,
};

/// "MEOW"：标在注入事件上的记号，钩子据此识别自己的按键。
pub const MAGIC: usize = 0x4D45_4F57;

fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let size = std::mem::size_of::<INPUT>() as i32;
    let sent = unsafe { SendInput(inputs, size) };
    if sent as usize != inputs.len() {
        return Err(format!(
            "SendInput 只发出 {sent}/{} 个事件（{}）",
            inputs.len(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MAGIC,
            },
        },
    }
}

fn vk_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: MAGIC,
            },
        },
    }
}

/// 逐字模拟 Unicode 输入：绕过输入法，直接落字到焦点控件。
pub fn type_text(text: &str) -> Result<(), String> {
    let mut inputs = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        inputs.push(unicode_input(unit, false));
        inputs.push(unicode_input(unit, true));
    }
    send(&inputs)
}

/// 重放发送键。
///
/// 只发 Enter 本身：Ctrl+Enter 触发时用户手上的 Ctrl 还按着，
/// 我们再注入一次 Ctrl 抬起反而会让应用误判修饰键状态。
pub fn send_enter() -> Result<(), String> {
    send(&[vk_input(VK_RETURN, false), vk_input(VK_RETURN, true)])
}

fn set_clipboard_text(text: &str) -> Result<(), String> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;

    for attempt in 0..6 {
        let opened = unsafe { OpenClipboard(None) };
        if opened.is_err() {
            std::thread::sleep(std::time::Duration::from_millis(20 * (attempt + 1)));
            continue;
        }

        let result = unsafe {
            EmptyClipboard()
                .map_err(|e| format!("EmptyClipboard 失败：{e}"))
                .and_then(|()| {
                    GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| format!("GlobalAlloc 失败：{e}"))
                })
                .and_then(|hglobal| {
                    let ptr = GlobalLock(hglobal);
                    if ptr.is_null() {
                        return Err("GlobalLock 失败".to_string());
                    }
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
                    let _ = GlobalUnlock(hglobal);
                    SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hglobal.0)))
                        .map(|_| ())
                        .map_err(|e| format!("SetClipboardData 失败：{e}"))
                })
        };

        let _ = unsafe { CloseClipboard() };
        return result;
    }

    Err("剪贴板被其他程序占用，稍后再试".to_string())
}

fn clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let mut out = None;
        if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok()
            && let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32) {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let base = ptr as *const u16;
                    let mut len = 0usize;
                    while len < (1 << 20) && *base.add(len) != 0 {
                        len += 1;
                    }
                    out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(base, len)));
                    let _ = GlobalUnlock(hglobal);
                }
            }
        let _ = CloseClipboard();
        out
    }
}

/// 剪贴板兜底注入：写入后缀 -> Ctrl+V，返回原来的剪贴板文本以便稍后恢复。
pub fn paste_text(text: &str) -> Result<Option<String>, String> {
    let previous = clipboard_text();
    set_clipboard_text(text)?;
    send(&[
        vk_input(VK_CONTROL, false),
        vk_input(VK_V, false),
        vk_input(VK_V, true),
        vk_input(VK_CONTROL, true),
    ])?;
    Ok(previous)
}

pub fn restore_clipboard(previous: Option<String>) {
    if let Some(text) = previous {
        let _ = set_clipboard_text(&text);
    }
}
