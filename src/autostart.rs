//! 开机自启：往 `HKCU\...\CurrentVersion\Run` 写/删一个值。
//!
//! 为什么用注册表而不是任务计划程序：
//! - 只用 HKCU，**不需要管理员权限**，也不会在登录时弹 UAC（任务计划可以提权，但为一个
//!   句尾补「喵~」的小工具装计划任务太重了，还容易被安全软件盯上）；
//! - 只影响当前用户，`reg query` / 任务管理器「启动」页都能看到，用户想手动关掉也容易。
//!
//! 注册的命令带 `--hidden`：开机起来时只驻留托盘，不弹面板。
//!
//! `RegCreateKeyExW` 在 windows crate 里挂在 `Win32_Security` feature 下面（它要
//! `SECURITY_ATTRIBUTES` 类型），所以 Cargo.toml 里那个 feature 不能删。

/// 注册表里那个值的名字（任务管理器「启动」页显示的就是它）。
pub const VALUE_NAME: &str = "meow-text";

/// 开机自启时附加的命令行参数：起来后藏到托盘，不弹面板。
pub const HIDDEN_FLAG: &str = "--hidden";

#[cfg(windows)]
mod imp {
    use super::{HIDDEN_FLAG, VALUE_NAME};
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW,
        RegDeleteValueW, RegSetValueExW,
    };
    use windows::core::{PCWSTR, w};

    /// 当前用户的「启动」项，在 `HKCU` 下，写它不需要管理员。
    const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn err(code: WIN32_ERROR, what: &str) -> String {
        format!("{what} 失败（Win32 错误码 {}）", code.0)
    }

    /// 打开（必要时创建）Run 键。
    fn open_run_key() -> Result<HKEY, String> {
        let mut key = HKEY::default();
        // 传入 None 表示「不请求写权限」，注册表 API 在键已存在时会直接打开它
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
        };
        if status.is_ok() {
            Ok(key)
        } else {
            Err(err(status, "打开启动项注册表键"))
        }
    }

    /// 注册表里那一行命令行：带引号的 exe 路径 + `--hidden`。
    pub fn command(path: &std::path::Path) -> String {
        format!("\"{}\" {HIDDEN_FLAG}", path.display())
    }

    /// 写入启动项；注册表里原来的命令行会被整段替换。
    pub fn enable(path: &std::path::Path) -> Result<(), String> {
        let key = open_run_key()?;
        let name = wide(VALUE_NAME);
        let data = wide(&command(path));

        let status = unsafe {
            RegSetValueExW(
                key,
                PCWSTR(name.as_ptr()),
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    data.as_ptr().cast::<u8>(),
                    std::mem::size_of_val(data.as_slice()),
                )),
            )
        };
        unsafe {
            let _ = RegCloseKey(key);
        }

        if status.is_ok() {
            Ok(())
        } else {
            Err(err(status, "写入开机启动项"))
        }
    }

    /// 删掉启动项；本来就没有也算成功。
    pub fn disable() -> Result<(), String> {
        let key = open_run_key()?;
        let name = wide(VALUE_NAME);
        let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
        unsafe {
            let _ = RegCloseKey(key);
        }

        // ERROR_FILE_NOT_FOUND：本来就没这个值，正是我们想要的结果
        if status.is_ok() || status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(err(status, "删除开机启动项"))
        }
    }

    /// 启动项现在登记的命令行（没有就是 `None`）。
    pub fn registered() -> Option<String> {
        use windows::Win32::System::Registry::{RRF_RT_REG_SZ, RRF_ZEROONFAILURE, RegGetValueW};

        let name = wide(VALUE_NAME);
        // 先问长度：数据是 UTF-16，size 出来是字节数（含结尾的 0）
        let mut size: u32 = 0;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ | RRF_ZEROONFAILURE,
                None,
                None,
                Some(&mut size),
            )
        };
        if !status.is_ok() || size == 0 {
            return None;
        }

        // 缓冲区按 UTF-16 码元算（size 是字节数，含结尾的 0）
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ | RRF_ZEROONFAILURE,
                None,
                Some(buffer.as_mut_ptr().cast::<core::ffi::c_void>()),
                Some(&mut size),
            )
        };
        if !status.is_ok() {
            return None;
        }

        let text = String::from_utf16_lossy(&buffer);
        let text = text.trim_end_matches('\0').to_string();
        (!text.is_empty()).then_some(text)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn command(path: &std::path::Path) -> String {
        format!("\"{}\" --hidden", path.display())
    }

    pub fn enable(_path: &std::path::Path) -> Result<(), String> {
        Err("只有在 Windows 上才能设置开机自启".to_string())
    }

    pub fn disable() -> Result<(), String> {
        Err("只有在 Windows 上才能设置开机自启".to_string())
    }

    pub fn registered() -> Option<String> {
        None
    }
}

pub use imp::{command, disable, enable, registered};

/// 注册表里的命令是不是就是「当前这个 exe + --hidden」。
///
/// 换过版本、挪过目录之后注册表里可能残留旧路径，用这个判断要不要重写一遍。
pub fn is_current(path: &std::path::Path) -> bool {
    registered().is_some_and(|text| text.trim().eq_ignore_ascii_case(command(path).trim()))
}

/// 按配置把开机自启调到该有的状态，返回这次有没有真的动注册表。
///
/// 只在真的不一致时才写，免得每次启动都去改一遍注册表。
pub fn sync(enabled: bool, path: &std::path::Path) -> Result<bool, String> {
    match (enabled, is_current(path)) {
        (true, true) | (false, false) => Ok(false),
        (true, false) => enable(path).map(|()| true),
        // 关掉时不管登记的是哪个路径，直接删——可能是旧版本留下的
        (false, true) => disable().map(|()| true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn command_quotes_the_path_and_asks_for_hidden_start() {
        let text = command(Path::new(r"C:\Program Files\meow-text.exe"));
        assert_eq!(text, "\"C:\\Program Files\\meow-text.exe\" --hidden");
    }

    #[test]
    fn command_keeps_the_flag_outside_the_quotes() {
        let text = command(Path::new(r"D:\tools\meow-text.exe"));
        assert!(text.starts_with('"') && text.ends_with(HIDDEN_FLAG), "{text}");
    }

    /// 这条会真的读写 HKCU，所以默认不跑：`cargo test -- --ignored` 才会执行。
    #[test]
    #[ignore = "会真的改当前用户的注册表启动项"]
    fn registry_write_and_remove_round_trip() {
        let fake = Path::new(r"C:\definitely\not\here\meow-text.exe");
        let before = registered();

        enable(fake).expect("写入失败");
        assert_eq!(registered().as_deref(), Some(command(fake).as_str()));
        assert!(!is_current(Path::new(r"C:\other\meow-text.exe")));

        disable().expect("删除失败");
        assert_ne!(registered(), Some(command(fake)));

        // 还原成测试之前的样子
        if let Some(previous) = before {
            let path = previous.trim().trim_matches('"').to_string();
            let _ = enable(Path::new(&path));
        }
    }
}
