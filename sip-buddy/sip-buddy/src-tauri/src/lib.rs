//! Sip Buddy: a tray app that sends a cheerful character across the screen
//! to remind you to drink water.

use std::{fs, path::PathBuf, sync::Mutex, thread, time::Duration};

use chrono::{Datelike, Local, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Wry,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const SNOOZE_MINUTES: i64 = 10;
const OVERLAY_HEIGHT: f64 = 430.0; // logical pixels

// ---------------------------------------------------------------- data ----

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    interval_minutes: u32,
    start_time: String, // "HH:MM"
    end_time: String,   // "HH:MM"
    weekdays_only: bool,
    daily_goal: u32,
    sound: bool,
    paused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval_minutes: 45,
            start_time: "09:00".into(),
            end_time: "18:00".into(),
            weekdays_only: false,
            daily_goal: 8,
            sound: true,
            paused: false,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Progress {
    date: String,
    glasses: u32,
}

struct State {
    settings: Settings,
    progress: Progress,
    next_due: i64,
    overlay_showing: bool,
    hit_rect: Option<[f64; 4]>,
    ignoring_cursor: bool,
}

struct Shared(Mutex<State>);
struct PauseItem(MenuItem<Wry>);

#[derive(Serialize)]
struct StateView {
    settings: Settings,
    glasses: u32,
    next_due: i64,
    autostart: bool,
}

#[derive(Clone, Serialize)]
struct ReminderInfo {
    glasses: u32,
    goal: u32,
    sound: bool,
    snooze_minutes: i64,
}

#[derive(Serialize)]
struct DoneInfo {
    glasses: u32,
    goal: u32,
}

// --------------------------------------------------------- persistence ----

fn now_ts() -> i64 {
    Local::now().timestamp()
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn data_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("sip-buddy"));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn load<T: for<'de> Deserialize<'de> + Default>(app: &AppHandle, name: &str) -> T {
    fs::read_to_string(data_dir(app).join(name))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save<T: Serialize>(app: &AppHandle, name: &str, value: &T) {
    if let Ok(s) = serde_json::to_string_pretty(value) {
        let _ = fs::write(data_dir(app).join(name), s);
    }
}

fn roll_day(app: &AppHandle, st: &mut State) {
    let d = today();
    if st.progress.date != d {
        st.progress = Progress { date: d, glasses: 0 };
        save(app, "progress.json", &st.progress);
    }
}

fn parse_time(s: &str, fallback: &str) -> NaiveTime {
    NaiveTime::parse_from_str(s, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::parse_from_str(fallback, "%H:%M").unwrap())
}

fn within_active_hours(s: &Settings) -> bool {
    let now = Local::now();
    if s.weekdays_only && matches!(now.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }
    let t = now.time();
    let start = parse_time(&s.start_time, "09:00");
    let end = parse_time(&s.end_time, "18:00");
    if start <= end {
        t >= start && t < end
    } else {
        // Overnight range, e.g. 22:00 – 06:00
        t >= start || t < end
    }
}

fn interval_secs(s: &Settings) -> i64 {
    i64::from(s.interval_minutes.clamp(5, 480)) * 60
}

// ---------------------------------------------------------------- tray ----

fn refresh_tray(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let status = if st.settings.paused {
        "paused".to_string()
    } else {
        let next = chrono::DateTime::from_timestamp(st.next_due, 0)
            .map(|d| d.with_timezone(&Local).format("%-I:%M %p").to_string())
            .unwrap_or_default();
        format!("next reminder {}", next.to_lowercase())
    };
    let tip = format!(
        "Sip Buddy · {} of {} glasses today · {}",
        st.progress.glasses, st.settings.daily_goal, status
    );
    let paused = st.settings.paused;
    drop(st);

    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&tip));
    }
    if let Some(item) = app.try_state::<PauseItem>() {
        let _ = item
            .0
            .set_text(if paused { "Resume reminders" } else { "Pause reminders" });
    }
}

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.emit("refresh", ());
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

// ------------------------------------------------------------- overlay ----

fn show_overlay(app: &AppHandle) {
    let shared = app.state::<Shared>();
    let info = {
        let mut st = shared.0.lock().unwrap();
        if st.overlay_showing {
            return;
        }
        roll_day(app, &mut st);
        st.overlay_showing = true;
        st.hit_rect = None;
        ReminderInfo {
            glasses: st.progress.glasses,
            goal: st.settings.daily_goal,
            sound: st.settings.sound,
            snooze_minutes: SNOOZE_MINUTES,
        }
    };

    let Some(win) = app.get_webview_window("overlay") else { return };

    // Stretch across the bottom of the main screen, just above the taskbar / dock.
    if let Ok(Some(mon)) = win.primary_monitor() {
        let scale = mon.scale_factor();
        let area = mon.work_area();
        let h = ((OVERLAY_HEIGHT * scale) as u32).min(area.size.height);
        let _ = win.set_size(PhysicalSize::new(area.size.width, h));
        let _ = win.set_position(PhysicalPosition::new(
            area.position.x,
            area.position.y + area.size.height as i32 - h as i32,
        ));
    }

    let _ = win.set_always_on_top(true);
    let _ = win.set_visible_on_all_workspaces(true);
    let _ = win.show();
    // Only after show(): the native window must exist before changing input handling.
    let _ = win.set_ignore_cursor_events(true);
    {
        shared.0.lock().unwrap().ignoring_cursor = true;
    }

    // Give the page a moment to paint before she starts walking.
    let app2 = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let _ = app2.emit_to("overlay", "start-reminder", info);
    });
}

fn hide_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.set_ignore_cursor_events(true);
        let _ = win.hide();
    }
    let shared = app.state::<Shared>();
    let mut st = shared.0.lock().unwrap();
    st.overlay_showing = false;
    st.hit_rect = None;
    st.ignoring_cursor = true;
}

/// Clicks pass through the transparent overlay everywhere except on
/// the character and her speech bubble.
fn cursor_watcher(app: AppHandle) {
    loop {
        let (showing, rect, ignoring) = {
            let shared = app.state::<Shared>();
            let st = shared.0.lock().unwrap();
            (st.overlay_showing, st.hit_rect, st.ignoring_cursor)
        };
        if !showing {
            thread::sleep(Duration::from_millis(300));
            continue;
        }
        let want_ignore = match (rect, app.get_webview_window("overlay")) {
            (Some([x, y, w, h]), Some(win)) => {
                match (app.cursor_position(), win.outer_position(), win.scale_factor()) {
                    (Ok(c), Ok(p), Ok(scale)) => {
                        let lx = (c.x - p.x as f64) / scale;
                        let ly = (c.y - p.y as f64) / scale;
                        !(lx >= x && lx <= x + w && ly >= y && ly <= y + h)
                    }
                    // If we can't read the cursor, keep the buttons clickable.
                    _ => false,
                }
            }
            _ => true,
        };
        if want_ignore != ignoring {
            if let Some(win) = app.get_webview_window("overlay") {
                let _ = win.set_ignore_cursor_events(want_ignore);
            }
            app.state::<Shared>().0.lock().unwrap().ignoring_cursor = want_ignore;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn scheduler(app: AppHandle) {
    let mut last_tray_refresh = 0i64;
    loop {
        thread::sleep(Duration::from_secs(15));
        let due = {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            roll_day(&app, &mut st);
            let now = now_ts();
            if st.settings.paused || st.overlay_showing {
                false
            } else if !within_active_hours(&st.settings) {
                // Outside active hours: the first reminder comes one interval
                // after the active period starts.
                st.next_due = now + interval_secs(&st.settings);
                false
            } else {
                // Laptop was asleep for ages? Don't fire a stale reminder instantly
                // on wake; give a short grace period instead.
                if now - st.next_due > 30 * 60 {
                    st.next_due = now + 60;
                    false
                } else {
                    now >= st.next_due
                }
            }
        };
        if due {
            show_overlay(&app);
        }
        if now_ts() - last_tray_refresh > 60 {
            refresh_tray(&app);
            last_tray_refresh = now_ts();
        }
    }
}

// ------------------------------------------------------------ commands ----

#[tauri::command]
fn get_state(app: AppHandle, shared: tauri::State<Shared>) -> StateView {
    let mut st = shared.0.lock().unwrap();
    roll_day(&app, &mut st);
    StateView {
        settings: st.settings.clone(),
        glasses: st.progress.glasses,
        next_due: st.next_due,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, shared: tauri::State<Shared>, settings: Settings) {
    {
        let mut st = shared.0.lock().unwrap();
        st.settings = settings;
        st.settings.interval_minutes = st.settings.interval_minutes.clamp(5, 480);
        st.settings.daily_goal = st.settings.daily_goal.clamp(1, 30);
        st.next_due = now_ts() + interval_secs(&st.settings);
        save(&app, "settings.json", &st.settings);
    }
    refresh_tray(&app);
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    let res = if enabled { al.enable() } else { al.disable() };
    res.map_err(|e| e.to_string())
}

#[tauri::command]
fn reminder_done(app: AppHandle, shared: tauri::State<Shared>) -> DoneInfo {
    let info = {
        let mut st = shared.0.lock().unwrap();
        roll_day(&app, &mut st);
        st.progress.glasses += 1;
        st.next_due = now_ts() + interval_secs(&st.settings);
        save(&app, "progress.json", &st.progress);
        DoneInfo { glasses: st.progress.glasses, goal: st.settings.daily_goal }
    };
    refresh_tray(&app);
    let _ = app.emit_to("settings", "refresh", ());
    info
}

#[tauri::command]
fn reminder_snooze(app: AppHandle, shared: tauri::State<Shared>) {
    shared.0.lock().unwrap().next_due = now_ts() + SNOOZE_MINUTES * 60;
    refresh_tray(&app);
}

#[tauri::command]
fn set_hit_rect(shared: tauri::State<Shared>, rect: Option<[f64; 4]>) {
    shared.0.lock().unwrap().hit_rect = rect;
}

#[tauri::command]
fn overlay_finished(app: AppHandle) {
    hide_overlay(&app);
}

#[tauri::command]
fn overlay_ready() {}

#[tauri::command]
fn show_now(app: AppHandle) {
    show_overlay(&app);
}

#[tauri::command]
fn add_glass(app: AppHandle, shared: tauri::State<Shared>, delta: i32) -> u32 {
    let g = {
        let mut st = shared.0.lock().unwrap();
        roll_day(&app, &mut st);
        st.progress.glasses = (st.progress.glasses as i32 + delta).max(0) as u32;
        save(&app, "progress.json", &st.progress);
        st.progress.glasses
    };
    refresh_tray(&app);
    g
}

// ----------------------------------------------------------------- run ----

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Launching the app again just opens settings.
            open_settings(app);
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_settings,
            set_autostart,
            reminder_done,
            reminder_snooze,
            set_hit_rect,
            overlay_finished,
            overlay_ready,
            show_now,
            add_glass
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let settings: Settings = load(&handle, "settings.json");
            let first_run = !data_dir(&handle).join("settings.json").exists();
            let progress: Progress = load(&handle, "progress.json");
            let next_due = now_ts() + interval_secs(&settings);
            app.manage(Shared(Mutex::new(State {
                settings: settings.clone(),
                progress,
                next_due,
                overlay_showing: false,
                hit_rect: None,
                ignoring_cursor: true,
            })));

            // Tray menu
            let now_i = MenuItem::with_id(app, "now", "Remind me now", true, None::<&str>)?;
            let drank_i =
                MenuItem::with_id(app, "drank", "I just drank a glass (+1)", true, None::<&str>)?;
            let pause_i = MenuItem::with_id(app, "pause", "Pause reminders", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Sip Buddy", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(
                app,
                &[&now_i, &drank_i, &sep, &pause_i, &settings_i, &sep2, &quit_i],
            )?;
            app.manage(PauseItem(pause_i));

            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("Sip Buddy")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "now" => show_overlay(app),
                    "drank" => {
                        {
                            let shared = app.state::<Shared>();
                            let mut st = shared.0.lock().unwrap();
                            roll_day(app, &mut st);
                            st.progress.glasses += 1;
                            st.next_due = now_ts() + interval_secs(&st.settings);
                            save(app, "progress.json", &st.progress);
                        }
                        refresh_tray(app);
                        let _ = app.emit_to("settings", "refresh", ());
                    }
                    "pause" => {
                        {
                            let shared = app.state::<Shared>();
                            let mut st = shared.0.lock().unwrap();
                            st.settings.paused = !st.settings.paused;
                            st.next_due = now_ts() + interval_secs(&st.settings);
                            save(app, "settings.json", &st.settings);
                        }
                        refresh_tray(app);
                        let _ = app.emit_to("settings", "refresh", ());
                    }
                    "settings" => open_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Keep windows alive when closed: just hide them.
            for label in ["settings", "overlay"] {
                if let Some(w) = app.get_webview_window(label) {
                    let w2 = w.clone();
                    w.on_window_event(move |e| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                            api.prevent_close();
                            let _ = w2.hide();
                        }
                    });
                }
            }

            if first_run {
                // Sensible defaults: save them, start at login, and say hello.
                save(&handle, "settings.json", &settings);
                let _ = handle.autolaunch().enable();
                open_settings(&handle);
            }

            // `--remind-now` shows a reminder straight away (handy for testing).
            if std::env::args().any(|a| a == "--remind-now") {
                let h = handle.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(1200));
                    show_overlay(&h);
                });
            }

            refresh_tray(&handle);
            let h1 = handle.clone();
            thread::spawn(move || scheduler(h1));
            let h2 = handle.clone();
            thread::spawn(move || cursor_watcher(h2));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Sip Buddy")
        .run(|_app, event| {
            // Stay alive in the tray when all windows are hidden.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
