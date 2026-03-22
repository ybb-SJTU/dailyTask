use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, Position, PhysicalPosition, Size, PhysicalSize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub time: String,
    pub completed: bool,
    pub date: String,
    pub notified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppData { pub tasks: Vec<Task>, }
impl Default for AppData { fn default() -> Self { Self { tasks: Vec::new() } } }
pub struct AppState { pub data: Mutex<AppData>, }

fn get_data_path(app: &AppHandle) -> PathBuf {
    let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
    if !app_dir.exists() { fs::create_dir_all(&app_dir).expect("Failed to create app data dir"); }
    app_dir.join("tasks.json")
}

fn load_data(app: &AppHandle) -> AppData {
    let path = get_data_path(app);
    if path.exists() { let content = fs::read_to_string(&path).expect("Failed to read tasks file"); serde_json::from_str(&content).unwrap_or_default() }
    else { AppData::default() }
}

fn save_data(app: &AppHandle, data: &AppData) {
    let path = get_data_path(app);
    let content = serde_json::to_string_pretty(data).expect("Failed to serialize tasks");
    fs::write(&path, content).expect("Failed to write tasks file");
}

#[tauri::command]
fn get_tasks(state: State<AppState>, date: String) -> Vec<Task> {
    let data = state.data.lock().unwrap();
    let today = Local::now().format("%Y-%m-%d").to_string();
    if date == today { data.tasks.iter().filter(|t| t.date == date || t.date == today).cloned().collect() }
    else { data.tasks.iter().filter(|t| t.date == date).cloned().collect() }
}

#[tauri::command]
fn add_task(app: AppHandle, state: State<AppState>, title: String, time: String) -> Task {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let task = Task { id: Uuid::new_v4().to_string(), title, time, completed: false, date: today, notified: false };
    let mut data = state.data.lock().unwrap();
    data.tasks.push(task.clone());
    save_data(&app, &data);
    task
}

#[tauri::command]
fn toggle_task(app: AppHandle, state: State<AppState>, id: String) -> bool {
    let mut data = state.data.lock().unwrap();
    if let Some(task) = data.tasks.iter_mut().find(|t| t.id == id) { task.completed = !task.completed; let result = task.completed; save_data(&app, &data); return result; }
    false
}

#[tauri::command]
fn delete_task(app: AppHandle, state: State<AppState>, id: String) {
    let mut data = state.data.lock().unwrap();
    data.tasks.retain(|t| t.id != id);
    save_data(&app, &data);
}

#[tauri::command]
fn get_history_dates(state: State<AppState>) -> Vec<String> {
    let data = state.data.lock().unwrap();
    let mut dates: Vec<String> = data.tasks.iter().map(|t| t.date.clone()).collect();
    dates.sort(); dates.dedup(); dates.reverse(); dates
}

#[tauri::command]
fn get_date_stats(state: State<AppState>, date: String) -> (i32, i32) {
    let data = state.data.lock().unwrap();
    let tasks: Vec<&Task> = data.tasks.iter().filter(|t| t.date == date).collect();
    let completed = tasks.iter().filter(|t| t.completed).count() as i32;
    let total = tasks.len() as i32;
    (total - completed, completed)
}

#[tauri::command]
fn check_overdue_tasks(app: AppHandle, state: State<AppState>) -> Vec<Task> {
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_time = now.format("%H:%M").to_string();
    let mut data = state.data.lock().unwrap();
    let mut overdue_tasks = Vec::new();
    for task in data.tasks.iter_mut() { if task.date == today && !task.completed && !task.notified { if task.time <= current_time { task.notified = true; overdue_tasks.push(task.clone()); } } }
    if !overdue_tasks.is_empty() { save_data(&app, &data); }
    overdue_tasks
}

#[tauri::command]
fn set_always_on_top(window: tauri::Window, on_top: bool) { window.set_always_on_top(on_top).unwrap(); }
#[tauri::command]
fn get_today() -> String { Local::now().format("%Y-%m-%d").to_string() }
#[tauri::command]
fn get_screen_size(app: AppHandle) -> (u32, u32) {
    if let Some(window) = app.get_webview_window("main") { if let Ok(monitor) = window.current_monitor() { if let Some(m) = monitor { return (m.size().width, m.size().height); } } }
    (1920, 1080)
}

#[tauri::command]
fn hide_to_edge(app: AppHandle, edge: String) {
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(size) = window.outer_size() {
            let mut sw = 1920u32;
            if let Ok(monitor) = window.current_monitor() { if let Some(m) = monitor { sw = m.size().width; } }
            let hide_width = 5u32;
            let new_w = if edge == "top" || edge == "bottom" { size.width } else { hide_width };
            let new_h = if edge == "left" || edge == "right" { size.height } else { hide_width };
            let _ = window.set_size(Size::Physical(PhysicalSize { width: new_w, height: new_h }));
            let new_x = match edge.as_str() { "left" => 0i32, "right" => (sw as i32) - (hide_width as i32), _ => 0i32 };
            let _ = window.set_position(Position::Physical(PhysicalPosition { x: new_x, y: 0i32 }));
        }
    }
}

#[tauri::command]
fn set_window_size(window: tauri::Window, width: u32, height: u32) { let _ = window.set_size(Size::Physical(PhysicalSize { width, height })); }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = load_data(&app.handle());
            app.manage(AppState { data: Mutex::new(data) });
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::TrayIconBuilder;
            let show_item = MenuItemBuilder::with_id("show", "显示").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app).item(&show_item).separator().item(&quit_item).build()?;
            let _tray = TrayIconBuilder::new().menu(&menu).tooltip("每日任务")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => { if let Some(w) = app.get_webview_window("main") { let _ = w.show(); let _ = w.set_focus(); } }
                    "quit" => { app.exit(0); }
                    _ => {}
                }).build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_tasks, add_task, toggle_task, delete_task, get_history_dates, get_date_stats,
            check_overdue_tasks, set_always_on_top, get_today, get_screen_size, hide_to_edge, set_window_size
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
