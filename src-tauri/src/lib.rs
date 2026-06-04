pub mod actions;
pub mod app_state;
pub mod batch;
pub mod commands;
pub mod config;
pub mod discovery;
pub mod emit_gate;
pub mod git_reader;
pub mod model;
pub mod scheduler;
pub mod snapshot;

/// Tauri 앱 진입점. (커맨드/스케줄러 배선은 후속 태스크에서 채움)
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
