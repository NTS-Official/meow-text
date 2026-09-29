// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(index) = args.iter().position(|arg| arg == "--probe") {
        let hold_ms = args
            .get(index + 1)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(300);
        attach_parent_console();
        meow_text_lib::probe(hold_ms);
        return;
    }

    // `--hidden` 由注册表里的开机启动项使用（见 src/autostart.rs）：
    // 登录后静默起来，只留托盘图标，不弹面板。
    meow_text_lib::run();
}

/// release 构建是 windows 子系统（平时没有控制台），`--probe` 时借一下父进程的控制台好打印状态。
///
/// 注意：如果标准输出已经被重定向（管道/文件），那时句柄是有效的，绝不能碰它 ——
/// AttachConsole 会把标准句柄改成控制台的，反而让输出丢失。
#[cfg(windows)]
fn attach_parent_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE};
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE).is_ok() {
            return;
        }
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}
