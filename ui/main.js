// meow-text 控制面板。没有打包器，直接用 withGlobalTauri 暴露的 window.__TAURI__。
//
// 布局照 WinUI 3 的 NavigationView：左侧导航栏切页，右侧一页内容。
// 主面板只留总开关 + 状态 + 最近事件，偏好（主题/语言/窗口）和高级（触发/注入/场景/自测）各自成页。
//
// 语言不用重新加载页面就能切：locale.js 里打包了**全部**语言的文案表，
// 这里只维护一个 currentLocale，切语言 = 换表重画。
// 静态文字靠 data-i18n / data-i18n-placeholder / data-i18n-title，动态文字用 t() 查表。

const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;

const LOCALES = window.MEOW_LOCALES ?? [];
const DEFAULT_LOCALE = window.MEOW_DEFAULT_LOCALE ?? LOCALES[0]?.tag ?? "en-US";

/** 导航收起状态记在本地，重启后保持。 */
const NAV_KEY = "meow.nav.collapsed";
/** 「最近事件」在主页显示几条。 */
const RECENT_LIMIT = 5;

/** 热键的默认值（和后端 hotkey::DEFAULT_HOTKEY 保持一致）。 */
const DEFAULT_HOTKEY = "ctrl+alt+'";

/**
 * 浏览器 KeyboardEvent.code → 配置里的键名（和后端 src/hotkey.rs 的 KEYS 表一一对应）。
 * 用 code 而不是 key：这样按的是哪个物理键就是哪个，不受输入法/大小写影响。
 */
const HOTKEY_KEYS = {
  Quote: "'",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Space: "space",
  Tab: "tab",
  Enter: "enter",
  Backspace: "backspace",
  Insert: "insert",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

const $ = (id) => document.getElementById(id);
const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

let state = null;
let config = null;
let saveTimer = null;
let currentLocale = DEFAULT_LOCALE;
let currentPage = "main";
/** 已经看过的最后一条日志，用来算导航栏上的小圆点 */
let seenLogKey = null;

/* ---------------------------------------------------------------- 文案 */

function stringsOf(tag) {
  return LOCALES.find((locale) => locale.tag === tag)?.strings ?? {};
}

/** 取文案：当前语言 -> 默认语言 -> 键名本身（漏配时一眼能看出来）。 */
function t(key, vars) {
  let text = stringsOf(currentLocale)[key] ?? stringsOf(DEFAULT_LOCALE)[key] ?? key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.split(`{${name}}`).join(String(value));
    }
  }
  return text;
}

/** 语言自称（简体中文 / English），永远用它自己的语言显示。 */
function localeName(tag) {
  return LOCALES.find((locale) => locale.tag === tag)?.name ?? tag;
}

/** 一次按键 → `ctrl+alt+'` 这种写法；不是合法热键就返回 null。 */
function hotkeyFromEvent(event) {
  let key = HOTKEY_KEYS[event.code];
  if (!key && /^Key[A-Z]$/.test(event.code)) {
    key = event.code.slice(3).toLowerCase();
  }
  if (!key && /^Digit[0-9]$/.test(event.code)) {
    key = event.code.slice(5);
  }
  if (!key && /^F([1-9]|1[0-9]|2[0-4])$/.test(event.code)) {
    key = event.code.toLowerCase();
  }
  if (!key) {
    return null; // 纯修饰键按下、或者我们不认的键
  }

  const modifiers = [];
  if (event.ctrlKey) modifiers.push("ctrl");
  if (event.altKey) modifiers.push("alt");
  if (event.shiftKey) modifiers.push("shift");
  if (event.metaKey) modifiers.push("win");
  if (!modifiers.length) {
    return null; // 至少得带一个修饰键
  }
  return [...modifiers, key].join("+");
}

/**
 * 有些文案里带少量标记（`<b>Enter</b>`、`<code>qq.exe</code>`，见 scenarios.hint），
 * 这些字符串来自编译进产物的语言包，不是用户输入，所以按 HTML 渲染；
 * 纯文本文案照旧走 textContent。
 */
function setText(el, key) {
  const text = t(key);
  if (text.includes("<")) {
    el.innerHTML = text;
  } else {
    el.textContent = text;
  }
}

/* ---------------------------------------------------------------- 导航 */

/** 每一页的页头副标题（页头标题复用导航项的文案，不用再配一遍）。 */
const PAGE_HINTS = {
  main: "app.tagline",
  logs: "logs.hint",
  preferences: "preferences.hint",
  advanced: "advanced.hint",
};

function showPage(page) {
  if (!PAGE_HINTS[page]) page = "main";
  currentPage = page;

  for (const item of $$(".nav-item")) {
    item.classList.toggle("active", item.dataset.page === page);
  }
  for (const section of $$(".page")) {
    section.hidden = section.dataset.page !== page;
  }

  $("page-title").dataset.i18n = `nav.${page}`;
  $("page-title").textContent = t(`nav.${page}`);
  $("page-hint").dataset.i18n = PAGE_HINTS[page];
  setText($("page-hint"), PAGE_HINTS[page]);

  if (page === "logs") {
    markLogsSeen();
  }
  if (state) {
    renderLogs(state.logs);
  }
}

function setNavCollapsed(collapsed) {
  $("navpane").parentElement.classList.toggle("nav-collapsed", collapsed);
  const label = t(collapsed ? "nav.expand" : "nav.collapse");
  $("nav-toggle").title = label;
  $("nav-toggle").dataset.i18nTitle = collapsed ? "nav.expand" : "nav.collapse";
  try {
    localStorage.setItem(NAV_KEY, collapsed ? "1" : "0");
  } catch {
    /* 隐私模式下没有 localStorage，不影响使用 */
  }
}

/** 导航栏上的未读小圆点：新日志到了而用户不在日志页时亮起。 */
function logKey(logs) {
  const last = logs[logs.length - 1];
  return last ? `${logs.length}:${last.at}:${last.detail}` : "";
}

function renderLogsBadge(logs) {
  const badge = $("logs-badge");
  const unseen = currentPage !== "logs" && logs.length > 0 && logKey(logs) !== seenLogKey;
  badge.hidden = !unseen;
  badge.textContent = unseen ? String(logs.length > 99 ? "99+" : logs.length) : "";
}

function markLogsSeen() {
  seenLogKey = state ? logKey(state.logs) : null;
  $("logs-badge").hidden = true;
}

/* ---------------------------------------------------------------- 语言 */

/** 语言下拉框：跟随系统 + 每种可用语言。 */
function renderLanguageSelect() {
  const select = $("language");
  const preference = config?.language ?? "system";
  const systemTag = state?.system_locale ?? DEFAULT_LOCALE;

  select.textContent = "";
  const systemOption = document.createElement("option");
  systemOption.value = "system";
  systemOption.textContent = t("language.system", { name: localeName(systemTag) });
  select.appendChild(systemOption);

  for (const locale of LOCALES) {
    const option = document.createElement("option");
    option.value = locale.tag;
    option.textContent = locale.name;
    select.appendChild(option);
  }

  select.value = LOCALES.some((locale) => locale.tag === preference) ? preference : "system";
  select.title = t("language.label");
}

/** 切换当前语言并重画整个面板（不重新加载页面）。 */
function applyLocale(tag) {
  if (tag && LOCALES.some((locale) => locale.tag === tag)) {
    currentLocale = tag;
  }

  document.documentElement.lang = currentLocale;
  document.title = t("app.window_title");

  for (const el of $$("[data-i18n]")) {
    setText(el, el.dataset.i18n);
  }
  for (const el of $$("[data-i18n-placeholder]")) {
    el.placeholder = t(el.dataset.i18nPlaceholder);
  }
  for (const el of $$("[data-i18n-title]")) {
    el.title = t(el.dataset.i18nTitle);
  }

  renderLanguageSelect();
  showPage(currentPage); // 页头标题/副标题是按 key 现算的
  if (config) {
    renderForm();
  }
  if (state) {
    renderStatus();
  }
}

/* ---------------------------------------------------------------- 主题 */

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

/** 配置里的三种模式解析成真正要用的 light / dark。 */
function resolvedTheme() {
  const mode = config?.theme ?? "system";
  if (mode === "system") {
    return darkQuery.matches ? "dark" : "light";
  }
  return mode;
}

function applyTheme() {
  const mode = config?.theme ?? "system";
  const resolved = resolvedTheme();

  document.documentElement.dataset.theme = resolved;

  for (const button of $("theme").querySelectorAll("button")) {
    button.classList.toggle("active", button.dataset.themeValue === mode);
  }

  const name = resolved === "dark" ? t("theme.dark") : t("theme.light");
  const summary = mode === "system" ? t("status.theme_system", { name }) : t("status.theme", { name });
  $("theme-summary").textContent = summary;
}

// 跟随系统时，系统换主题要立刻跟上
darkQuery.addEventListener("change", () => {
  if ((config?.theme ?? "system") === "system") {
    applyTheme();
  }
});

/** 关闭行为对应的提示（用字面量 key，方便本地化检查脚本扫到）。 */
function closeHint(mode) {
  if (mode === "hide") return t("footer.close_hide");
  if (mode === "quit") return t("footer.close_quit");
  return t("footer.close_ask");
}

/* ------------------------------------------------------------ 关闭提示框 */

function showCloseModal() {
  $("close-remember").checked = false;
  $("close-modal").hidden = false;
  $("close-hide").focus();
}

function hideCloseModal() {
  $("close-modal").hidden = true;
}

async function resolveClose(action) {
  const remember = $("close-remember").checked;
  hideCloseModal();
  if (!invoke) return;
  try {
    state = await invoke("resolve_close", { action, remember });
    config = state.config;
    renderStatus();
    renderForm();
  } catch (error) {
    console.error(error);
  }
}

/* ---------------------------------------------------------------- 数据往返 */

async function refresh() {
  if (!invoke) {
    $("hook-pill").textContent = t("status.hook_preview");
    $("save-state").textContent = t("status.no_tauri");
    return;
  }
  state = await invoke("get_state");
  config = state.config;
  // 配置里的语言可能和当前显示的不一致（比如别处改过），跟上
  if (state.locale && state.locale !== currentLocale) {
    applyLocale(state.locale);
  }
  renderStatus();
  renderForm();
}

async function save() {
  if (!invoke || !config) return;
  $("save-state").textContent = t("footer.saving");
  try {
    state = await invoke("save_config", { config });
    config = state.config;
    // 存盘时 Rust 侧也切了语言（窗口标题 / 托盘菜单），这里同步前端
    applyLocale(state.locale);
    renderStatus();
    $("save-state").textContent = t("footer.saved", { time: new Date().toLocaleTimeString() });
  } catch (error) {
    $("save-state").textContent = t("footer.save_failed", { error });
    console.error(error);
  }
}

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, 250);
}

/* ------------------------------------------------------------------ 渲染 */

function renderStatus() {
  if (!state) return;

  $("enabled").checked = config.enabled;
  $("enabled-text").textContent = config.enabled ? t("app.switch_on") : t("app.switch_off");

  $("stat-today").textContent = config.stats.today;
  $("stat-total").textContent = config.stats.total;

  const hook = $("hook-pill");
  if (state.hook_error) {
    hook.textContent = t("status.hook_failed", { error: state.hook_error });
    hook.className = "pill bad";
  } else if (state.hook_active) {
    hook.textContent = t("status.hook_mounted");
    hook.className = "pill good";
  } else {
    hook.textContent = t("status.hook_missing");
    hook.className = "pill warn";
  }

  const foreground = state.foreground_process;
  const known = foreground && config.scenarios.some((s) => s.processes.includes(foreground));
  $("fg-pill").textContent = foreground
    ? t(known ? "status.foreground_hit" : "status.foreground", { process: foreground })
    : t("status.foreground_empty");
  $("fg-pill").className = known ? "pill good" : "pill";

  // 导航栏底部一小行路径；详细那行在日志页
  $("config-path").textContent = state.config_path;
  $("config-path-detail").textContent = t("logs.config_path", {
    path: state.config_path,
    process: state.own_process,
  });
  $("app-info").textContent = [
    state.own_process,
    t("logs.locale", { locale: currentLocale }),
  ].join(" · ");

  applyTheme();
  renderLogs(state.logs);
}

function renderForm() {
  $("suffix").value = config.suffix;
  $("trigger").value = config.trigger;
  $("inject-mode").value = config.inject_mode;
  $("delay").value = config.inject_delay_ms;
  $("close-action").value = config.close_action;
  $("require-content").checked = config.require_content;
  $("skip-composing").checked = config.skip_when_composing;
  $("dry-run").checked = config.dry_run;
  $("close-hint").textContent = closeHint(config.close_action);
  // 语言那一行的说明：跟随系统时顺便告诉用户实际选中了哪一门
  $("language-summary").textContent =
    config.language === "system"
      ? t("language.system", { name: localeName(state?.system_locale ?? DEFAULT_LOCALE) })
      : localeName(config.language);

  // 开机自启显示的是注册表里**实际登记的那条命令**（可能和我们现在的路径不一样）
  renderAutostart();
  renderHotkey();

  renderScenarios();
}

/** 热键那一行：显示当前热键（没有就是提示文案），注册失败时把原因写在下面。 */
function renderHotkey() {
  const input = $("hotkey");
  const hint = $("hotkey-hint");
  const value = config.hotkey ?? "";

  input.value = state?.hotkey_display ?? value;
  input.title = t("preferences.hotkey");
  hint.textContent = state?.hotkey_error ?? t("preferences.hotkey_hint");
  hint.classList.toggle("warn", Boolean(state?.hotkey_error));
  $("hotkey-default").disabled = value === DEFAULT_HOTKEY;
}

/** 把一个热键真正装上（后端会写系统热键 + 存配置）。 */
async function applyHotkey(hotkey) {
  if (!invoke) {
    config.hotkey = hotkey;
    $("hotkey").value = hotkey;
    return;
  }
  try {
    state = await invoke("set_hotkey", { hotkey });
    config = state.config;
    renderHotkey();
  } catch (error) {
    // 注册被拒（多半是被别的程序占了）：开关不动，把原因显示在下面
    $("hotkey-hint").textContent = String(error);
    $("hotkey-hint").classList.add("warn");
    console.error(error);
  }
}

/** 开机自启那一行：开关状态 + 注册表里实际登记的命令。 */
function renderAutostart() {
  const toggle = $("autostart");
  const registered = state?.autostart_command ?? null;
  const hint = $("autostart-hint");

  toggle.checked = Boolean(config.autostart);
  toggle.title = t("preferences.autostart");

  if (registered) {
    hint.textContent = t("preferences.autostart_on", { command: registered });
    // 配置说开着、注册表里却不是这个 exe：多半是换了目录或版本
    hint.classList.toggle("stale", !config.autostart);
  } else {
    hint.textContent = t("preferences.autostart_off");
    hint.classList.toggle("stale", Boolean(config.autostart));
  }
}

function renderScenarios() {
  const host = $("scenarios");
  host.textContent = "";
  config.scenarios.forEach((scenario, index) => host.appendChild(scenarioRow(scenario, index)));
}

function scenarioRow(scenario, index) {
  const row = document.createElement("div");
  row.className = "scenario";

  const toggle = document.createElement("input");
  toggle.type = "checkbox";
  toggle.checked = scenario.enabled;
  toggle.title = t("scenarios.title");
  toggle.addEventListener("change", () => {
    scenario.enabled = toggle.checked;
    scheduleSave();
  });

  const name = document.createElement("input");
  name.type = "text";
  name.className = "scenario-name";
  name.value = scenario.name;
  name.placeholder = t("scenarios.name_placeholder");
  // 内置场景的名字跟着语言包走，不让改
  name.readOnly = scenario.builtin;
  name.addEventListener("change", () => {
    scenario.name = name.value.trim() || t("scenarios.untitled");
    scheduleSave();
  });

  const processes = document.createElement("input");
  processes.type = "text";
  processes.className = "scenario-processes";
  processes.value = scenario.processes.join(", ");
  processes.placeholder = t("scenarios.processes_placeholder");
  processes.addEventListener("change", () => {
    scenario.processes = processes.value
      .split(",")
      .map((item) => item.trim().toLowerCase())
      .filter(Boolean);
    scheduleSave();
  });

  const suffix = document.createElement("input");
  suffix.type = "text";
  suffix.className = "scenario-suffix";
  suffix.value = scenario.suffix ?? "";
  suffix.placeholder = t("scenarios.suffix_placeholder", { suffix: config.suffix });
  suffix.addEventListener("change", () => {
    const value = suffix.value.trim();
    scenario.suffix = value ? value : null;
    scheduleSave();
  });

  const trigger = document.createElement("select");
  trigger.className = "scenario-trigger";
  for (const [value, label] of [
    ["", t("scenarios.trigger_default")],
    ["enter", t("trigger.enter")],
    ["ctrl_enter", t("trigger.ctrl_enter")],
  ]) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    trigger.appendChild(option);
  }
  trigger.value = scenario.trigger ?? "";
  trigger.addEventListener("change", () => {
    scenario.trigger = trigger.value ? trigger.value : null;
    scheduleSave();
  });

  row.append(toggle, name, processes, suffix, trigger);

  if (scenario.builtin) {
    const tag = document.createElement("span");
    tag.className = "tag";
    tag.textContent = t("scenarios.builtin");
    tag.title = scenario.note || "";
    row.appendChild(tag);
  } else {
    const remove = document.createElement("button");
    remove.className = "ghost danger-text";
    remove.textContent = t("scenarios.delete");
    remove.addEventListener("click", () => {
      config.scenarios.splice(index, 1);
      renderScenarios();
      scheduleSave();
    });
    row.appendChild(remove);
  }

  return row;
}

/** 主面板只显示最近几条，日志页显示全部（同一个渲染器）。 */
function logList(host, logs, limit) {
  host.textContent = "";
  if (!logs.length) {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = t("logs.empty");
    host.appendChild(empty);
    return;
  }
  const shown = limit ? logs.slice(-limit).reverse() : [...logs].reverse();
  for (const entry of shown) {
    const item = document.createElement("li");
    item.className = entry.ok ? "log ok" : "log warn";
    const time = document.createElement("span");
    time.className = "time";
    time.textContent = entry.at;
    const who = document.createElement("span");
    who.className = "who";
    who.textContent = entry.process;
    const detail = document.createElement("span");
    detail.className = "detail";
    detail.textContent = entry.detail;
    item.append(time, who, detail);
    host.appendChild(item);
  }
}

function renderLogs(logs) {
  logList($("logs-recent"), logs, RECENT_LIMIT);
  logList($("logs"), logs, 0);
  renderLogsBadge(logs);
}

/* ------------------------------------------------------------------ 事件 */

function bindUi() {
  for (const item of $$(".nav-item")) {
    item.addEventListener("click", () => showPage(item.dataset.page));
  }
  $("nav-toggle").addEventListener("click", () => {
    setNavCollapsed(!$("navpane").parentElement.classList.contains("nav-collapsed"));
  });
  $("open-logs").addEventListener("click", () => showPage("logs"));

  $("enabled").addEventListener("change", (event) => {
    config.enabled = event.target.checked;
    $("enabled-text").textContent = config.enabled ? t("app.switch_on") : t("app.switch_off");
    scheduleSave();
  });

  $("language").addEventListener("change", (event) => {
    const preference = event.target.value;
    config.language = preference;
    // 立刻按新语言重画，不等存盘往返；"system" 用后端解析出的系统语言
    applyLocale(preference === "system" ? state?.system_locale ?? DEFAULT_LOCALE : preference);
    scheduleSave();
  });

  for (const button of $("theme").querySelectorAll("button")) {
    button.addEventListener("click", () => {
      config.theme = button.dataset.themeValue;
      applyTheme();
      // 存盘时 Rust 侧会把主题同步给原生标题栏
      scheduleSave();
    });
  }

  $("suffix").addEventListener("input", (event) => {
    config.suffix = event.target.value;
    scheduleSave();
  });
  $("trigger").addEventListener("change", (event) => {
    config.trigger = event.target.value;
    scheduleSave();
  });
  $("inject-mode").addEventListener("change", (event) => {
    config.inject_mode = event.target.value;
    scheduleSave();
  });
  $("delay").addEventListener("change", (event) => {
    config.inject_delay_ms = Math.max(0, Math.min(2000, Number(event.target.value) || 0));
    event.target.value = config.inject_delay_ms;
    scheduleSave();
  });
  $("close-action").addEventListener("change", (event) => {
    config.close_action = event.target.value;
    $("close-hint").textContent = closeHint(config.close_action);
    scheduleSave();
  });

  // 开机自启要动注册表，所以立刻往返一次；失败了就把开关拨回去
  $("autostart").addEventListener("change", async (event) => {
    const wanted = event.target.checked;
    if (!invoke) {
      config.autostart = wanted;
      scheduleSave();
      return;
    }
    try {
      state = await invoke("set_autostart", { enabled: wanted });
      config = state.config;
      renderStatus();
      renderForm();
    } catch (error) {
      event.target.checked = Boolean(config.autostart);
      $("save-state").textContent = t("footer.save_failed", { error });
      console.error(error);
    }
  });

  for (const [id, key] of [
    ["require-content", "require_content"],
    ["skip-composing", "skip_when_composing"],
    ["dry-run", "dry_run"],
  ]) {
    $(id).addEventListener("change", (event) => {
      config[key] = event.target.checked;
      scheduleSave();
    });
  }

  $("add-scenario").addEventListener("click", () => {
    config.scenarios.push({
      id: `custom-${Date.now()}`,
      name: t("scenarios.new_name"),
      processes: [],
      enabled: true,
      suffix: null,
      trigger: null,
      builtin: false,
      note: "",
    });
    renderScenarios();
    scheduleSave();
  });

  // 自测框聚焦时才允许钩子对本窗口生效，避免影响面板里其它输入。
  const selftest = $("selftest");
  selftest.addEventListener("focus", () => invoke?.("set_selftest", { active: true }));
  selftest.addEventListener("blur", () => invoke?.("set_selftest", { active: false }));

  $("inject-once").addEventListener("click", async () => {
    if (!invoke) return;
    state = await invoke("inject_once");
    config = state.config;
    renderStatus();
  });

  $("reset-stats").addEventListener("click", async () => {
    if (!invoke) return;
    state = await invoke("reset_stats");
    config = state.config;
    renderStatus();
  });

  $("clear-log").addEventListener("click", async () => {
    if (!invoke) return;
    state = await invoke("clear_logs");
    config = state.config;
    renderStatus();
  });

  // 热键录制：聚焦后按下的组合键会被记下来（Esc 取消，纯修饰键不算）
  const hotkeyInput = $("hotkey");
  let recording = false;

  const setRecording = (active) => {
    recording = active;
    hotkeyInput.classList.toggle("recording", active);
    hotkeyInput.placeholder = active ? t("preferences.hotkey_recording") : t("preferences.hotkey_off");
    invoke?.("set_hotkey_capture", { active });
  };

  hotkeyInput.addEventListener("focus", () => setRecording(true));
  hotkeyInput.addEventListener("blur", () => setRecording(false));
  hotkeyInput.addEventListener("keydown", async (event) => {
    event.preventDefault(); // 这个框只用来"按一下"，不接受输入
    if (event.key === "Escape") {
      hotkeyInput.blur();
      return;
    }
    const hotkey = hotkeyFromEvent(event);
    if (!hotkey) {
      return; // 只按了修饰键（或按了不支持的键）：继续等
    }
    // 先给出反馈，再等后端注册的结果
    hotkeyInput.value = hotkey
      .split("+")
      .map((part) => (part === "ctrl" ? "Ctrl" : part === "alt" ? "Alt" : part === "shift" ? "Shift" : part === "win" ? "Win" : part.toUpperCase()))
      .join("+");
    await applyHotkey(hotkey);
    hotkeyInput.blur();
  });

  $("hotkey-default").addEventListener("click", () => applyHotkey(DEFAULT_HOTKEY));

  $("quit").addEventListener("click", () => invoke?.("quit_app"));
  $("hide").addEventListener("click", async () => {
    if (!invoke) return;
    await invoke("hide_window");
  });

  $("close-hide").addEventListener("click", () => resolveClose("hide"));
  $("close-quit").addEventListener("click", () => resolveClose("quit"));
  // 点空白处或按 Esc = 我还没想好，继续开着
  $("close-modal").addEventListener("click", (event) => {
    if (event.target === $("close-modal")) hideCloseModal();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !$("close-modal").hidden) hideCloseModal();
  });
}

async function subscribe() {
  if (!listen) return;

  await listen("meow://log", async () => {
    state = await invoke("get_state");
    config = state.config;
    renderStatus();
  });

  // 热键开/关总开关时后端会直接推一份新状态过来，面板不用等下一次轮询
  await listen("meow://state", (event) => {
    state = event.payload;
    config = state.config;
    renderStatus();
    renderForm();
  });

  // 点 × 时 Rust 侧先把关闭拦下来，问我们要怎么办
  await listen("meow://close-requested", () => showCloseModal());
}

function boot() {
  let collapsed = false;
  try {
    collapsed = localStorage.getItem(NAV_KEY) === "1";
  } catch {
    /* 忽略 */
  }
  setNavCollapsed(collapsed);

  bindUi();
  applyLocale(DEFAULT_LOCALE);
  showPage("main");
  subscribe();
  refresh();
  setInterval(refresh, 1000);
}

boot();
