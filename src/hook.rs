//! Windows 全局键盘钩子（WH_KEYBOARD_LL）+ 注入工作线程。
//!
//! 数据流：
//! ```text
//! 用户按回车 ──> 钩子回调（必须极快）
//!                  ├─ 判定该不该喵化（meow::decide）
//!                  ├─ 吞掉这次回车，把任务丢进队列
//!                  └─ 立刻返回
//!               注入线程
//!                  ├─ 焦点没变？→ 逐字注入「喵~」（或走剪贴板）
//!                  └─ 重放回车，把消息发出去
//! ```

use crate::clock;
use crate::config;
use crate::i18n;
use crate::inject;
use crate::meow::{self, Config, Decision, InjectMode, KeyEvent};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::Ime::{GCS_COMPSTR, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_MENU, VK_RETURN, VK_V, VK_X,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HHOOK,
    KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, LLKHF_INJECTED, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use windows::core::PWSTR;

const MAX_LOGS: usize = 60;

/// 日志外送通道：由上层（Tauri 层）注入，引擎本身不依赖任何 UI 框架。
pub type LogSink = Box<dyn Fn(LogEntry) + Send + Sync + 'static>;

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();
static HOOK_HANDLE: OnceLock<usize> = OnceLock::new();
/// 已经吞掉一次回车按下，那么对应的抬起也要吞掉，别让应用收到半截按键。
static SWALLOW_ENTER_UP: AtomicBool = AtomicBool::new(false);
/// 前台窗口 -> 进程名 的小缓存，避免每次按键都开进程。
static PROC_CACHE: Mutex<Option<(isize, Option<String>)>> = Mutex::new(None);

#[derive(Serialize, Clone, Debug)]
pub struct LogEntry {
    pub at: String,
    pub process: String,
    pub detail: String,
    pub ok: bool,
}

impl LogEntry {
    pub fn new(process: impl Into<String>, detail: impl Into<String>, ok: bool) -> Self {
        Self {
            at: clock::now_hms(),
            process: process.into(),
            detail: detail.into(),
            ok,
        }
    }

    pub fn ok(process: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(process, detail, true)
    }

    pub fn warn(process: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(process, detail, false)
    }
}

/// 交给注入线程的一条任务。
pub struct Job {
    pub hwnd: isize,
    pub suffix: String,
    pub mode: InjectMode,
    pub delay_ms: u64,
    pub process: String,
    pub scenario: String,
    pub dry_run: bool,
}

struct WindowTrack {
    hint: i32,
    seen: Instant,
}

/// 引擎：配置、统计、日志、窗口内容启发式、注入队列都在这里。
pub struct Engine {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    pub own_process: String,
    pub selftest: AtomicBool,
    /// 用户正在偏好页录制热键：这期间热键回调不生效，免得把正在录的那一串按出来
    pub hotkey_capture: AtomicBool,
    /// 仅供测试：接受模拟按键（正常运行时必须为 false，否则会自己触发自己）
    pub accept_injected: AtomicBool,
    pub hook_active: AtomicBool,
    pub hook_error: Mutex<Option<String>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    windows: Mutex<HashMap<isize, WindowTrack>>,
    sink: OnceLock<LogSink>,
    jobs: Sender<Job>,
}

impl Engine {
    pub fn new(config: Config, config_path: PathBuf, own_process: String) -> Arc<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        let engine = Arc::new(Self {
            config: RwLock::new(config),
            config_path,
            own_process,
            selftest: AtomicBool::new(false),
            hotkey_capture: AtomicBool::new(false),
            accept_injected: AtomicBool::new(false),
            hook_active: AtomicBool::new(false),
            hook_error: Mutex::new(None),
            logs: Mutex::new(VecDeque::new()),
            windows: Mutex::new(HashMap::new()),
            sink: OnceLock::new(),
            jobs,
        });

        let worker = Arc::clone(&engine);
        std::thread::Builder::new()
            .name("meow-inject".into())
            .spawn(move || inject_worker(worker, queue))
            .expect("启动注入线程失败");

        engine
    }

    /// 接上外送通道（控制面板用它把日志推给前端）。
    pub fn attach_log_sink(&self, sink: LogSink) {
        let _ = self.sink.set(sink);
    }

    pub fn config_snapshot(&self) -> Config {
        match self.config.read() {
            Ok(cfg) => cfg.clone(),
            Err(_) => Config::default(),
        }
    }

    /// 界面保存配置：统计数字由引擎维护，不接受界面覆盖。
    pub fn set_config(&self, mut incoming: Config) -> Result<Config, String> {
        config::ensure_builtin_scenarios(&mut incoming);
        incoming.normalize();
        if let Ok(live) = self.config.read() {
            incoming.stats = live.stats.clone();
        }
        config::save(&self.config_path, &incoming)?;
        match self.config.write() {
            Ok(mut live) => *live = incoming.clone(),
            Err(_) => return Err("配置锁已损坏".into()),
        }
        Ok(incoming)
    }

    /// 翻转总开关（热键用），返回翻转后的状态并记一条日志。
    pub fn toggle_enabled(&self) -> Result<bool, String> {
        let mut snapshot = self.config_snapshot();
        snapshot.enabled = !snapshot.enabled;
        let saved = self.set_config(snapshot)?;
        let detail = if saved.enabled {
            i18n::LOG_HOTKEY_ENABLED.text()
        } else {
            i18n::LOG_HOTKEY_DISABLED.text()
        };
        self.push_log(LogEntry::ok(i18n::LOG_SYSTEM, detail));
        Ok(saved.enabled)
    }

    pub fn reset_stats(&self) -> Result<Config, String> {
        let snapshot = match self.config.write() {
            Ok(mut cfg) => {
                config::reset_stats(&mut cfg, &clock::today());
                cfg.clone()
            }
            Err(_) => return Err("配置锁已损坏".into()),
        };
        config::save(&self.config_path, &snapshot)?;
        Ok(snapshot)
    }

    fn register_meow(&self) {
        let snapshot = match self.config.write() {
            Ok(mut cfg) => {
                config::bump_stats(&mut cfg, &clock::today());
                cfg.clone()
            }
            Err(_) => return,
        };
        if let Err(err) = config::save(&self.config_path, &snapshot) {
            eprintln!("[meow] 保存统计失败：{err}");
        }
    }

    pub fn push_log(&self, entry: LogEntry) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.push_front(entry.clone());
            while logs.len() > MAX_LOGS {
                logs.pop_back();
            }
        }
        if let Some(sink) = self.sink.get() {
            sink(entry);
        }
    }

    pub fn logs(&self) -> Vec<LogEntry> {
        self.logs
            .lock()
            .map(|logs| logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.clear();
        }
    }

    /// 输入框内容启发式：> 0 就认为框里大概有内容。
    fn hint(&self, hwnd: isize) -> i32 {
        self.windows
            .lock()
            .ok()
            .and_then(|map| map.get(&hwnd).map(|track| track.hint))
            .unwrap_or(0)
    }

    fn bump_hint(&self, hwnd: isize, delta: i32) {
        if let Ok(mut map) = self.windows.lock() {
            if map.len() > 128 {
                map.retain(|_, track| track.seen.elapsed() < Duration::from_secs(600));
            }
            let track = map.entry(hwnd).or_insert(WindowTrack {
                hint: 0,
                seen: Instant::now(),
            });
            track.hint = (track.hint + delta).clamp(0, 4096);
            track.seen = Instant::now();
        }
    }

    fn reset_hint(&self, hwnd: isize) {
        if let Ok(mut map) = self.windows.lock() {
            map.remove(&hwnd);
        }
    }
}

fn foreground_hwnd() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

fn process_name_of(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_ascii_lowercase())
    }
}

/// 前台窗口的进程名（小写），带一层缓存。
pub fn process_of_window(hwnd: HWND) -> Option<String> {
    let key = hwnd.0 as isize;
    if let Ok(cache) = PROC_CACHE.lock()
        && let Some((cached_key, name)) = cache.as_ref()
            && *cached_key == key {
                return name.clone();
            }

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let name = if pid == 0 { None } else { process_name_of(pid) };

    if let Ok(mut cache) = PROC_CACHE.lock() {
        *cache = Some((key, name.clone()));
    }
    name
}

/// 当前前台窗口的进程名，面板用来显示「前台：qq.exe」。
pub fn foreground_process() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    process_of_window(hwnd)
}

fn key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// 输入法是否正在组词（best effort：拿不到 IME 上下文就当作没在组词）。
fn is_composing(hwnd: HWND) -> bool {
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.0.is_null() {
            return false;
        }
        let len = ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0);
        let _ = ImmReleaseContext(hwnd, himc);
        len > 0
    }
}

/// 会往输入框里落字的按键（用于内容启发式）。
fn is_text_key(vk: u16) -> bool {
    matches!(
        vk,
        0x20                          // space
            | 0x30..=0x39             // 0-9
            | 0x41..=0x5A             // A-Z
            | 0x60..=0x6F             // 数字小键盘
            | 0xBA..=0xC0             // ; = , - . /
            | 0xDB..=0xDE             // [ \ ] '
            | 0xE2                     // OEM 102
            | 0xE5                     // VK_PROCESSKEY（输入法处理过的键）
            | 0xE7 // VK_PACKET
    )
}

fn next_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hhk = HOOK_HANDLE.get().map(|raw| HHOOK(*raw as *mut core::ffi::c_void));
    unsafe { CallNextHookEx(hhk, code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return next_hook(code, wparam, lparam);
    }

    // 回调里绝不允许 panic，所有取锁都用 if let / match 兜住。
    let Some(engine) = ENGINE.get() else {
        return next_hook(code, wparam, lparam);
    };

    let message = wparam.0 as u32;
    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let vk = kb.vkCode as u16;
    let injected = (kb.flags.0 & LLKHF_INJECTED.0) != 0;
    let is_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    let is_up = message == WM_KEYUP || message == WM_SYSKEYUP;

    // 我们自己注入的事件：原样放行
    if kb.dwExtraInfo == inject::MAGIC {
        return next_hook(code, wparam, lparam);
    }

    if vk == VK_RETURN.0 {
        if is_up {
            // 【重要】吞掉按下就必须吞掉抬起：绝不能把抬起事件放进去，
            // 否则目标应用会收到一个「只有 keyup」的回车。
            if SWALLOW_ENTER_UP.swap(false, Ordering::SeqCst) {
                return LRESULT(1);
            }
            return next_hook(code, wparam, lparam);
        }
        if !is_down {
            return next_hook(code, wparam, lparam);
        }

        let hwnd = unsafe { GetForegroundWindow() };
        let process = process_of_window(hwnd);
        let ctrl_down = key_down(VK_CONTROL);
        let event = KeyEvent {
            is_enter_down: true,
            ctrl_down,
            alt_down: key_down(VK_MENU),
            injected,
            is_own_process: process.as_deref() == Some(engine.own_process.as_str()),
            process: process.as_deref(),
            selftest_active: engine.selftest.load(Ordering::SeqCst),
            composing: is_composing(hwnd),
            input_maybe_empty: engine.hint(hwnd.0 as isize) <= 0,
        };

        let cfg = engine.config_snapshot();
        let accept_injected = engine.accept_injected.load(Ordering::SeqCst);

        return match meow::decide(&cfg, &event, accept_injected) {
            Decision::Meow(plan) => {
                SWALLOW_ENTER_UP.store(true, Ordering::SeqCst);
                let job = Job {
                    hwnd: hwnd.0 as isize,
                    suffix: plan.suffix,
                    mode: cfg.inject_mode,
                    delay_ms: cfg.inject_delay_ms,
                    process: process.unwrap_or_default(),
                    scenario: plan.scenario_name,
                    dry_run: cfg.dry_run,
                };
                if engine.jobs.send(job).is_err() {
                    // 队列挂了：至少把回车放回去，别把用户的消息卡住
                    SWALLOW_ENTER_UP.store(false, Ordering::SeqCst);
                    return next_hook(code, wparam, lparam);
                }
                LRESULT(1)
            }
            Decision::Skip { reason, scenario } => {
                if let Some(name) = scenario {
                    let detail = i18n::LOG_SKIP.fill( &[("reason", reason.label())]);
                    engine.push_log(LogEntry::warn(name, detail));
                }
                next_hook(code, wparam, lparam)
            }
        };
    }

    // 其它按键：维护「输入框里大概有内容」的启发式
    if is_down {
        let hwnd = foreground_hwnd();
        let ctrl = key_down(VK_CONTROL);
        if ctrl && vk == VK_V.0 {
            engine.bump_hint(hwnd, 1); // 粘贴：当作有内容
        } else if ctrl && vk == VK_X.0 {
            engine.reset_hint(hwnd); // 剪切：框空了
        } else if vk == VK_BACK.0 || vk == VK_DELETE.0 {
            engine.bump_hint(hwnd, -1);
        } else if is_text_key(vk) {
            engine.bump_hint(hwnd, 1);
        }
    }

    next_hook(code, wparam, lparam)
}

fn hook_thread(engine: Arc<Engine>) {
    unsafe {
        let module = GetModuleHandleW(None).map(|m| HINSTANCE(m.0)).unwrap_or_default();
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(module), 0) {
            Ok(hook) => {
                let _ = HOOK_HANDLE.set(hook.0 as usize);
                engine.hook_active.store(true, Ordering::SeqCst);
                if let Ok(mut err) = engine.hook_error.lock() {
                    *err = None;
                }
                engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_HOOK_MOUNTED));

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }

                let _ = UnhookWindowsHookEx(hook);
                engine.hook_active.store(false, Ordering::SeqCst);
                engine.push_log(LogEntry::warn(i18n::LOG_SYSTEM, i18n::LOG_HOOK_UNMOUNTED));
            }
            Err(err) => {
                let text = i18n::LOG_HOOK_FAILED.fill( &[("error", &err.to_string())]);
                engine.hook_active.store(false, Ordering::SeqCst);
                if let Ok(mut slot) = engine.hook_error.lock() {
                    *slot = Some(text.clone());
                }
                engine.push_log(LogEntry::warn("系统", text));
            }
        }
    }
}

/// 启动全局钩子（只能调用一次）。
pub fn start(engine: Arc<Engine>) -> Result<(), String> {
    if ENGINE.set(Arc::clone(&engine)).is_err() {
        return Err("键盘钩子已经启动过了".into());
    }
    std::thread::Builder::new()
        .name("meow-hook".into())
        .spawn(move || hook_thread(engine))
        .map(|_| ())
        .map_err(|e| format!("启动钩子线程失败：{e}"))
}

fn inject_worker(engine: Arc<Engine>, queue: Receiver<Job>) {
    while let Ok(job) = queue.recv() {
        let target = job.hwnd;

        if foreground_hwnd() != target {
            engine.push_log(LogEntry::warn(&job.scenario, i18n::LOG_FOCUS_LOST));
            continue;
        }

        std::thread::sleep(Duration::from_millis(job.delay_ms.min(2000)));

        if foreground_hwnd() != target {
            engine.push_log(LogEntry::warn(&job.scenario, i18n::LOG_FOCUS_LOST));
            engine.reset_hint(target);
            continue;
        }

        let mut notes: Vec<String> = Vec::new();
        let mut appended = false;
        let mut restore: Option<Option<String>> = None;

        if job.dry_run {
            notes.push(i18n::LOG_DRY_RUN.fill( &[("suffix", job.suffix.as_str())]));
        } else {
            match job.mode {
                InjectMode::Type => match inject::type_text(&job.suffix) {
                    Ok(()) => {
                        std::thread::sleep(Duration::from_millis(25));
                        appended = true;
                        notes.push(i18n::LOG_APPENDED.fill( &[("suffix", job.suffix.as_str())]));
                    }
                    Err(err) => notes.push(i18n::LOG_TYPE_FAILED.fill( &[("error", &err)])),
                },
                InjectMode::Clipboard => match inject::paste_text(&job.suffix) {
                    Ok(previous) => {
                        std::thread::sleep(Duration::from_millis(150));
                        appended = true;
                        restore = Some(previous);
                        notes.push(i18n::LOG_APPENDED_CLIPBOARD.fill(
                            &[("suffix", job.suffix.as_str())],
                        ));
                    }
                    Err(err) => notes.push(i18n::LOG_CLIPBOARD_FAILED.fill( &[("error", &err)])),
                },
            }
        }

        // 无论注入成功与否，都要把发送键还回去，否则用户的消息会卡在输入框里。
        if let Err(err) = inject::send_enter() {
            notes.push(i18n::LOG_ENTER_FAILED.fill( &[("error", &err.to_string())]));
        }

        if let Some(previous) = restore {
            std::thread::sleep(Duration::from_millis(250));
            inject::restore_clipboard(previous);
        }

        engine.reset_hint(target);
        if appended {
            engine.register_meow();
        }
        let detail = notes.join("；");
        engine.push_log(LogEntry::new(&job.scenario, detail, appended));
    }
}

/// 供 `--probe` 自检模式使用：等一小会儿，回报钩子状态。
pub fn probe_status(engine: &Engine) -> String {
    let cfg = engine.config_snapshot();
    let status = serde_json::json!({
        "hook_active": engine.hook_active.load(Ordering::SeqCst),
        "hook_error": engine.hook_error.lock().ok().and_then(|e| e.clone()),
        "own_process": engine.own_process,
        "enabled": cfg.enabled,
        "suffix": cfg.suffix,
        "scenarios": cfg.scenarios.iter().map(|s| serde_json::json!({
            "id": s.id,
            "enabled": s.enabled,
            "processes": s.processes,
        })).collect::<Vec<_>>(),
        "foreground_process": foreground_process(),
        "config": config::summarize(&cfg),
    });
    status.to_string()
}

/// 自检用：等钩子线程装好钩子。
pub fn wait_hook_ready(engine: &Engine, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if engine.hook_active.load(Ordering::SeqCst) || engine.hook_error.lock().map(|e| e.is_some()).unwrap_or(false)
        {
            return engine.hook_active.load(Ordering::SeqCst);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// 面板用：注入一次后缀到当前焦点窗口（不重放回车），验证注入链路。
pub fn inject_probe(engine: &Engine) -> Result<(), String> {
    let cfg = engine.config_snapshot();
    let target = foreground_hwnd();
    if cfg.dry_run {
        engine.push_log(LogEntry::ok(i18n::LOG_SELFTEST, i18n::LOG_SELFTEST_DRY_RUN));
        return Ok(());
    }
    let result = inject::type_text(&cfg.suffix);
    engine.reset_hint(target);
    match &result {
        Ok(()) => engine.push_log(LogEntry::ok(
            i18n::LOG_SELFTEST,
            i18n::LOG_SELFTEST_OK.fill( &[("suffix", cfg.suffix.as_str())]),
        )),
        Err(err) => engine.push_log(LogEntry::warn(
            i18n::LOG_SELFTEST,
            i18n::LOG_SELFTEST_FAILED.fill( &[("error", err)]),
        )),
    }
    result
}
