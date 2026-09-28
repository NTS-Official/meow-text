//! meow-text：在 QQ 等聊天软件里发消息时，自动在句尾补上「喵~」。
//!
//! 分工：
//! - [`meow`]   纯判定逻辑（可单测）
//! - [`hook`]   Windows 全局键盘钩子 + 注入线程
//! - [`inject`] SendInput / 剪贴板注入原语
//! - 本文件    Tauri 控制面板（功能开关 + 场景配置 + 状态展示）

#[cfg(not(windows))]
compile_error!("meow-text 目前只能在 Windows 上工作（依赖全局键盘钩子与 SendInput 注入）");

pub mod autostart;
pub mod clock;
pub mod config;
pub mod hook;
pub mod hotkey;
pub mod i18n;
pub mod inject;
pub mod meow;

use hook::{Engine, LogEntry};
use hotkey::HotkeyThread;
use meow::{CloseAction, Config, ThemeMode};
use serde::Serialize;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub engine: Arc<Engine>,
    /// 全局热键线程；`None` 表示还没起起来（单测/预览时不会走到这里）
    pub hotkeys: Mutex<Option<HotkeyThread>>,
}

/// 面板要显示的一种语言。
#[derive(Serialize, Clone)]
pub struct LocaleInfo {
    pub tag: String,
    /// 该语言的自称（简体中文 / English），任何语言下都显示这个
    pub name: String,
}

/// 交给控制面板的完整状态。
///
/// `Clone` 是给热键用的：热键在后台线程翻转开关后，要把整份状态推给面板。
#[derive(Serialize, Clone)]
pub struct UiState {
    pub config: Config,
    pub hook_active: bool,
    pub hook_error: Option<String>,
    pub foreground_process: Option<String>,
    pub own_process: String,
    pub config_path: String,
    /// 当前生效的语言标签
    pub locale: String,
    /// 编译进来的全部语言
    pub locales: Vec<LocaleInfo>,
    pub default_locale: String,
    /// 系统界面语言解析出来的标签（「跟随系统」时用它）
    pub system_locale: String,
    /// 注册表里登记的开机启动命令（没有就是 None），用来显示「实际生效的那个路径」
    pub autostart_command: Option<String>,
    /// 热键的显示形式（`Ctrl+Alt+'`），没配就是 None
    pub hotkey_display: Option<String>,
    /// 热键真正注册到系统了没（被别的程序占用时会失败）
    pub hotkey_active: bool,
    /// 注册失败的原因
    pub hotkey_error: Option<String>,
    pub logs: Vec<LogEntry>,
}

/// 组装给前端的完整状态。
///
/// 热键那两项要从热键线程问（它才知道系统里到底注册上了没），
/// 所以带上 `app` 拿状态；取不到（比如还没起起来）就按"没注册"处理。
fn build_state(app: &AppHandle, engine: &Engine) -> UiState {
    let configured = engine.config_snapshot().hotkey;
    let registered = app
        .try_state::<AppState>()
        .and_then(|state| state.hotkeys.lock().ok().and_then(|slot| slot.as_ref().and_then(|it| it.current())));

    let hotkey_display = hotkey::display(&configured);
    let hotkey_error = if configured.is_empty() || registered.as_deref() == Some(configured.as_str()) {
        None
    } else {
        Some(
            i18n::PREFERENCES_HOTKEY_ERROR_TAKEN
                .fill(&[("hotkey", &hotkey_display.clone().unwrap_or_else(|| configured.clone()))]),
        )
    };

    UiState {
        config: engine.config_snapshot(),
        hook_active: engine.hook_active.load(Ordering::SeqCst),
        hook_error: engine.hook_error.lock().ok().and_then(|err| err.clone()),
        foreground_process: hook::foreground_process(),
        own_process: engine.own_process.clone(),
        config_path: engine.config_path.display().to_string(),
        locale: i18n::current_tag().to_string(),
        locales: i18n::available()
            .iter()
            .map(|locale| LocaleInfo {
                tag: locale.tag.to_string(),
                name: locale.name.to_string(),
            })
            .collect(),
        default_locale: i18n::default_tag().to_string(),
        system_locale: i18n::system_tag().to_string(),
        autostart_command: autostart::registered(),
        hotkey_display,
        hotkey_active: registered.as_deref() == Some(configured.as_str()),
        hotkey_error,
        logs: engine.logs(),
    }
}

/// 把主题同步给原生窗口（标题栏跟着变）；跟随系统时交还给系统。
fn apply_window_theme(app: &AppHandle, mode: ThemeMode) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let theme = match mode {
        ThemeMode::System => None,
        ThemeMode::Light => Some(tauri::Theme::Light),
        ThemeMode::Dark => Some(tauri::Theme::Dark),
    };
    if let Err(err) = window.set_theme(theme) {
        eprintln!("[meow] 设置窗口主题失败：{err}");
    }
}

fn show_main_window(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(window) => {
            if let Err(err) = window.show() {
                eprintln!("[meow] 显示面板失败：{err}");
            }
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        None => eprintln!("[meow] 找不到主窗口，无法显示面板"),
    }
}

/// 隐藏面板但保留进程：键盘钩子继续工作，托盘图标负责把它叫回来。
fn hide_main_window(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(window) => {
            if let Err(err) = window.hide() {
                eprintln!("[meow] 隐藏面板失败：{err}");
            } else {
                eprintln!("[meow] 面板已隐藏到后台");
            }
        }
        None => eprintln!("[meow] 找不到主窗口，无法隐藏"),
    }
}

/// 应用语言偏好：切 i18n 当前语言，并把窗口标题、托盘菜单一起换掉。
///
/// 返回实际生效的标签；菜单只在语言真的变了时重建。
fn apply_language(app: &AppHandle, preference: &str) -> &'static str {
    let previous = i18n::current_tag();
    let tag = i18n::apply_preference(preference);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(i18n::APP_WINDOW_TITLE.text());
    }
    if tag != previous {
        refresh_tray(app);
    }
    tag
}

/// 语言变了要重建托盘菜单（菜单项文案是建托盘时写死的）。
fn refresh_tray(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("meow-tray") else {
        return;
    };
    match build_tray_menu(app) {
        Ok(menu) => {
            if let Err(err) = tray.set_menu(Some(menu)) {
                eprintln!("[meow] 重建托盘菜单失败：{err}");
            }
            let _ = tray.set_tooltip(Some(i18n::TRAY_TOOLTIP.text()));
        }
        Err(err) => eprintln!("[meow] 重建托盘菜单失败：{err}"),
    }
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    MenuBuilder::new(app)
        .text("show", i18n::TRAY_SHOW.text())
        .separator()
        .text("quit", i18n::TRAY_QUIT.text())
        .build()
}

/// 托盘图标：左键点开面板，右键菜单能显示面板 / 退出。
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;

    let mut builder = TrayIconBuilder::with_id("meow-tray")
        .tooltip(i18n::TRAY_TOOLTIP.text())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

#[tauri::command]
fn get_state(app: AppHandle, state: State<'_, AppState>) -> UiState {
    build_state(&app, &state.engine)
}

#[tauri::command]
fn save_config(app: AppHandle, state: State<'_, AppState>, config: Config) -> Result<UiState, String> {
    let saved = state.engine.set_config(config)?;
    apply_window_theme(&app, saved.theme);
    // 语言可以随时切：存盘后立刻生效（窗口标题 + 托盘菜单一起换）
    apply_language(&app, &saved.language);
    Ok(build_state(&app, &state.engine))
}

#[tauri::command]
fn set_selftest(app: AppHandle, state: State<'_, AppState>, active: bool) -> UiState {
    state.engine.selftest.store(active, Ordering::SeqCst);
    build_state(&app, &state.engine)
}

#[tauri::command]
fn reset_stats(app: AppHandle, state: State<'_, AppState>) -> Result<UiState, String> {
    state.engine.reset_stats()?;
    Ok(build_state(&app, &state.engine))
}

#[tauri::command]
fn clear_logs(app: AppHandle, state: State<'_, AppState>) -> UiState {
    state.engine.clear_logs();
    build_state(&app, &state.engine)
}

/// 把当前配置的后缀注入到焦点窗口（自测按钮，不重放回车）。
#[tauri::command]
fn inject_once(app: AppHandle, state: State<'_, AppState>) -> Result<UiState, String> {
    hook::inject_probe(&state.engine)?;
    Ok(build_state(&app, &state.engine))
}

/// 开机自启开关：写注册表成功了才把配置改成对应状态（界面不会撒谎）。
#[tauri::command]
fn set_autostart(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<UiState, String> {
    let exe = own_exe_path();
    let result = if enabled {
        autostart::enable(&exe)
    } else {
        autostart::disable()
    };

    match result {
        Ok(()) => {
            let detail = if enabled {
                i18n::LOG_AUTOSTART_ON.fill(&[("path", &exe.display().to_string())])
            } else {
                i18n::LOG_AUTOSTART_OFF.text().to_string()
            };
            state.engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, detail));

            // 注册表写成功了才认这个配置，免得界面显示「已开启」而实际没有
            let mut cfg = state.engine.config_snapshot();
            cfg.autostart = enabled;
            state.engine.set_config(cfg)?;
            Ok(build_state(&app, &state.engine))
        }
        Err(error) => {
            state.engine.push_log(LogEntry::warn(
                i18n::LOG_SYSTEM,
                i18n::LOG_AUTOSTART_FAILED.fill(&[("error", &error)]),
            ));
            Err(error)
        }
    }
}

/// 设置热键。`hotkey` 传空字符串就是关掉热键；注册不成功不改配置。
#[tauri::command]
fn set_hotkey(app: AppHandle, state: State<'_, AppState>, hotkey: String) -> Result<UiState, String> {
    let canonical = if hotkey.trim().is_empty() {
        String::new()
    } else {
        hotkey::canonical(&hotkey).ok_or_else(|| i18n::PREFERENCES_HOTKEY_ERROR_FORMAT.text().to_string())?
    };

    // 先让系统注册，成功了再改配置（和开机自启一个思路：界面不撒谎）
    {
        let slot = state.hotkeys.lock().map_err(|_| "热键状态锁失效".to_string())?;
        if let Some(thread) = slot.as_ref() {
            thread.set(canonical.clone())?;
        }
    }

    let detail = if canonical.is_empty() {
        i18n::LOG_HOTKEY_OFF.text().to_string()
    } else {
        let shown = hotkey::display(&canonical).unwrap_or_else(|| canonical.clone());
        i18n::LOG_HOTKEY_ON.fill(&[("hotkey", &shown)])
    };
    state.engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, detail));

    let mut cfg = state.engine.config_snapshot();
    cfg.hotkey = canonical;
    state.engine.set_config(cfg)?;
    Ok(build_state(&app, &state.engine))
}

/// 面板正在录制热键：这期间热键回调不生效，免得把正在录的那一串按键当成触发热键。
#[tauri::command]
fn set_hotkey_capture(state: State<'_, AppState>, active: bool) {
    state.engine.hotkey_capture.store(active, Ordering::SeqCst);
}

/// 开机自启时进程是被 Windows 拉起来的，加 `--hidden` 就只驻留托盘、不弹面板。
fn start_hidden() -> bool {
    std::env::args().any(|arg| arg == autostart::HIDDEN_FLAG)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// 面板上的「隐藏到后台」。
#[tauri::command]
fn hide_window(app: AppHandle, state: State<'_, AppState>) {
    state.engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_HIDDEN));
    hide_main_window(&app);
}

#[tauri::command]
fn show_window(app: AppHandle) {
    show_main_window(&app);
}

/// 关闭提示框的选择结果。`remember` 为真时把选择写进配置，下次不再问。
#[tauri::command]
fn resolve_close(
    app: AppHandle,
    state: State<'_, AppState>,
    action: CloseAction,
    remember: bool,
) -> Result<UiState, String> {
    if remember {
        let mut cfg = state.engine.config_snapshot();
        cfg.close_action = action;
        state.engine.set_config(cfg)?;
    }

    if action.is_hide() {
        state.engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_HIDDEN));
        hide_main_window(&app);
    } else {
        app.exit(0);
    }

    Ok(build_state(&app, &state.engine))
}

fn own_process_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_ascii_lowercase()))
        .unwrap_or_else(|| "meow-text.exe".to_string())
}

/// 当前这个 exe 的完整路径（开机启动项里要登记的就是它）。
fn own_exe_path() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("meow-text.exe"))
}

fn config_path_for(app: &AppHandle) -> std::path::PathBuf {
    match app.path().app_config_dir() {
        Ok(dir) => dir.join("config.json"),
        Err(_) => std::path::PathBuf::from("meow-text-config.json"),
    }
}

pub fn run() {
    tauri::Builder::default()
        // 单实例：再点一次 exe 不新开一个（否则会有两个钩子、两个托盘图标），
        // 而是把已经藏在后台的那个面板叫回来。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
            if let Some(state) = app.try_state::<AppState>() {
                state
                    .engine
                    .push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_ALREADY_RUNNING));
            }
        }))
        .setup(|app| {
            let handle = app.handle().clone();
            let path = config_path_for(&handle);
            let cfg = config::load_or_default(&path);

            // 先定语言，后面所有文案（窗口标题、托盘菜单、日志）才是对的
            i18n::apply_preference(&cfg.language);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(i18n::APP_WINDOW_TITLE.text());
            }
            apply_window_theme(&handle, cfg.theme);

            let engine = Engine::new(cfg, path, own_process_name());
            engine.attach_log_sink(Box::new(move |entry: LogEntry| {
                let _ = handle.emit("meow://log", entry);
            }));

            // 配置里开着自启就把注册表对齐（换过目录/版本后顺手修掉旧路径），
            // 关着的话顺手清掉旧版本留下的启动项。
            match autostart::sync(engine.config_snapshot().autostart, &own_exe_path()) {
                Ok(true) => {
                    engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_AUTOSTART_SYNCED.text()))
                }
                Ok(false) => {}
                Err(error) => engine.push_log(LogEntry::warn(
                    i18n::LOG_SYSTEM,
                    i18n::LOG_AUTOSTART_FAILED.fill(&[("error", &error)]),
                )),
            }

            if let Err(err) = hook::start(Arc::clone(&engine)) {
                engine.push_log(LogEntry::warn(i18n::LOG_SYSTEM, err));
            }

            // 托盘图标：隐藏到后台之后靠它把面板叫回来
            if let Err(err) = build_tray(app.handle()) {
                engine.push_log(LogEntry::warn(
                    i18n::LOG_SYSTEM,
                    i18n::LOG_HOOK_FAILED.fill(&[("error", &err.to_string())]),
                ));
            }

            // 全局热键：开/关总开关。注册失败（被别的程序占了）就在界面和日志里说出来。
            let hotkey_value = engine.config_snapshot().hotkey;
            let hotkey_engine = Arc::clone(&engine);
            let hotkey_app = app.handle().clone();
            let (hotkeys, hotkey_error) = HotkeyThread::spawn(hotkey_value.clone(), move || {
                // 正在录制新热键时，别把这一串按键当成触发
                if hotkey_engine.hotkey_capture.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(err) = hotkey_engine.toggle_enabled() {
                    hotkey_engine.push_log(LogEntry::warn(i18n::LOG_SYSTEM, err));
                    return;
                }
                // 面板上的开关、状态条要立刻跟着变
                let state = build_state(&hotkey_app, &hotkey_engine);
                let _ = hotkey_app.emit("meow://state", state);
            });
            if let Some(error) = hotkey_error {
                engine.push_log(LogEntry::warn(
                    i18n::LOG_SYSTEM,
                    i18n::LOG_HOTKEY_FAILED.fill(&[("error", &error)]),
                ));
            } else if !hotkey_value.is_empty() {
                let shown = hotkey::display(&hotkey_value).unwrap_or(hotkey_value);
                engine.push_log(LogEntry::ok(
                    i18n::LOG_SYSTEM,
                    i18n::LOG_HOTKEY_READY.fill(&[("hotkey", &shown)]),
                ));
            }

            // 开机自启拉起来的那次只驻留托盘：面板留到用户点托盘图标时再出现
            if start_hidden() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                engine.push_log(LogEntry::ok(i18n::LOG_SYSTEM, i18n::LOG_AUTOSTART_STARTED.text()));
            }

            app.manage(AppState {
                engine,
                hotkeys: Mutex::new(Some(hotkeys)),
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };

            // 关窗一律先拦下来，按配置决定：问一句 / 隐藏到后台 / 直接退出
            api.prevent_close();
            let app = window.app_handle().clone();
            let action = app
                .try_state::<AppState>()
                .map(|state| state.engine.config_snapshot().close_action)
                .unwrap_or_default();

            match action {
                CloseAction::Hide => hide_main_window(&app),
                CloseAction::Quit => app.exit(0),
                CloseAction::Ask => {
                    let _ = app.emit("meow://close-requested", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_config,
            set_autostart,
            set_hotkey,
            set_hotkey_capture,
            set_selftest,
            reset_stats,
            clear_logs,
            inject_once,
            hide_window,
            show_window,
            resolve_close,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running meow-text");
}

/// `meow-text --probe [hold_ms]`：装好钩子、打印一行 JSON 状态后退出，方便脚本验证。
pub fn probe(hold_ms: u64) {
    let path = std::env::temp_dir().join("meow-text-probe.json");
    let engine = Engine::new(config::default_config(), path, own_process_name());
    if let Err(err) = hook::start(Arc::clone(&engine)) {
        eprintln!("[probe] {err}");
    }
    let ready = hook::wait_hook_ready(&engine, std::time::Duration::from_millis(1500));
    std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    println!("{}", hook::probe_status(&engine));
    if !ready {
        eprintln!("[probe] 钩子未在超时前就绪");
    }
}
