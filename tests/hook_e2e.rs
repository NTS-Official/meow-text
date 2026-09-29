//! 端到端验证：全局键盘钩子 → 判定 → 注入 → 重放回车，全链路走一遍。
//!
//! 做法：开一个真实的 Win32 EDIT 窗口并抢到前台，装上真正的 WH_KEYBOARD_LL，
//! 用 SendInput 模拟「打字 + 回车」，然后读回编辑框内容，确认「喵~」是被 Rust 侧补进去的。
//!
//! 只有在能抢到前台焦点时才会真正跑；抢不到就打印 SKIP 并跳过（避免误报红）。
//! 另外钩子只看前台进程名，所以测试期间你在别的窗口敲回车不会被打扰。

#![cfg(windows)]

use meow_text_lib::{config, hook};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    MSG, PM_REMOVE, PeekMessageW, SW_SHOW, SetForegroundWindow, SetWindowTextW, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::w;

const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_WANTRETURN: u32 = 0x1000;

fn own_process_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()))
        .unwrap_or_default()
}

fn create_edit_window() -> HWND {
    let style = WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 | WS_VISIBLE.0 | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            style,
            120,
            120,
            360,
            140,
            None,
            None,
            None,
            None,
        )
        .expect("创建测试用 EDIT 窗口失败")
    }
}

/// 抢前台；抢不到就返回 false（测试会跳过）。
fn force_foreground(hwnd: HWND) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        if GetForegroundWindow() == hwnd {
            return true;
        }

        // 前台锁会挡 SetForegroundWindow，用 AttachThreadInput 绕过
        let foreground = GetForegroundWindow();
        let other_thread = GetWindowThreadProcessId(foreground, None);
        let this_thread = GetCurrentThreadId();
        if other_thread != 0 && other_thread != this_thread {
            let _ = AttachThreadInput(other_thread, this_thread, true);
            std::thread::sleep(Duration::from_millis(50));
            let _ = SetForegroundWindow(hwnd);
            let _ = AttachThreadInput(other_thread, this_thread, false);
        }
        GetForegroundWindow() == hwnd
    }
}

/// 模拟真实按键（dwExtraInfo = 0，不带 MAGIC，钩子会当成外部输入处理）。
fn send_text(text: &str) {
    let mut inputs: Vec<INPUT> = Vec::new();
    for unit in text.encode_utf16() {
        for up in [false, true] {
            let mut flags = KEYEVENTF_UNICODE;
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: unit,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });
        }
    }
    unsafe {
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        assert_eq!(sent as usize, inputs.len(), "模拟打字没发出去");
    }
    std::thread::sleep(Duration::from_millis(80));
}

fn send_enter() {
    let scan = unsafe { MapVirtualKeyW(VK_RETURN.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    let make = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_RETURN,
                wScan: scan,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [make(false), make(true)];
    unsafe {
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        assert_eq!(sent as usize, inputs.len(), "模拟回车没发出去");
    }
}

/// 抽掉窗口消息，让 EDIT 控件真正处理那些字符与回车。
fn pump_for(ms: u64) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    let mut msg = MSG::default();
    while Instant::now() < deadline {
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn clear_text(hwnd: HWND) {
    unsafe {
        let _ = SetWindowTextW(hwnd, w!(""));
    }
}

#[test]
fn hook_pipeline_fires_only_when_input_box_has_content() {
    let process = own_process_name();
    assert!(!process.is_empty(), "拿不到自己的进程名");

    let hwnd = create_edit_window();
    if !force_foreground(hwnd) {
        println!("[e2e] SKIP：抢不到前台焦点（当前前台是别的窗口），未运行端到端断言");
        // CI 日志里「跳过」和「跑过」都是 PASS，这里补一条 annotation，
        // 免得看到绿灯就以为注入链路真的在那个环境里验证过了。
        if std::env::var_os("CI").is_some() {
            println!("::warning title=e2e 被跳过::抢不到前台焦点，这次没有真正验证注入链路");
        }
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    }

    // 配置：只对「本测试进程」生效，QQ 那种场景在这里换成测试自己的进程名
    let mut cfg = config::default_config();
    cfg.enabled = true;
    cfg.require_content = true; // 顺便验证「输入框非空」启发式
    cfg.skip_when_composing = false;
    cfg.inject_delay_ms = 20;
    cfg.stats = Default::default();
    cfg.scenarios = vec![meow_text_lib::meow::Scenario {
        id: "e2e".into(),
        name: "e2e".into(),
        processes: vec![process.clone()],
        enabled: true,
        suffix: None,
        trigger: None,
        builtin: false,
        note: String::new(),
    }];

    let engine = hook::Engine::new(
        cfg,
        std::env::temp_dir().join("meow-text-e2e-config.json"),
        process.clone(),
    );
    // 模拟按键需要被钩子接受（真实使用中永远为 false）
    engine.accept_injected.store(true, std::sync::atomic::Ordering::SeqCst);

    hook::start(Arc::clone(&engine)).expect("启动钩子失败");
    assert!(
        hook::wait_hook_ready(&engine, Duration::from_millis(1500)),
        "键盘钩子没有就绪"
    );
    assert!(engine.hook_active.load(std::sync::atomic::Ordering::SeqCst));

    // ---- 阶段 1：空输入框按回车 → 应该原样放行，绝不能凭空补一个「喵~」 ----
    send_enter();
    pump_for(500);
    let empty_case = window_text(hwnd);
    println!("[e2e] 阶段1 空输入框 + 回车 → 编辑框={empty_case:?}");
    assert!(
        !empty_case.contains("喵~"),
        "空输入框不该被附加后缀，实际是 {empty_case:?}"
    );
    assert!(
        empty_case.contains('\n'),
        "回车应当原样交给应用（这里表现为换行），实际是 {empty_case:?}"
    );

    // ---- 阶段 2：框里有内容再按回车 → 吞掉回车、补「喵~」、重放回车 ----
    clear_text(hwnd);
    send_text("hi");
    pump_for(120);
    send_enter();
    pump_for(900);
    let fired_case = window_text(hwnd);
    println!("[e2e] 阶段2 有内容 + 回车 → 编辑框={fired_case:?}");

    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    assert!(
        fired_case.contains("hi喵~"),
        "注入链路没有生效：期望出现「hi喵~」，实际是 {fired_case:?}"
    );
    assert!(
        fired_case.contains("hi喵~\r\n"),
        "回车没有被正确重放，实际是 {fired_case:?}"
    );

    let stats = engine.config_snapshot().stats;
    println!("[e2e] 统计：今日 {} 次 / 累计 {} 次", stats.today, stats.total);
    assert_eq!(stats.today, 1, "统计应当只记 1 次（阶段 1 是跳过的）");
}
