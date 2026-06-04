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

use app_state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// Tauri 앱 진입점: AppState 관리, 커맨드 등록, 창 포커스 게이팅 + 폴링 루프 배선.
pub fn run() {
    let cfg = config::load_from(&config::config_path());
    let state = Arc::new(AppState::new(cfg));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::scan_repos,
            commands::refresh_status,
            commands::open_action,
        ])
        .setup(move |app| {
            use tauri::Manager;
            let handle = app.handle().clone();

            // 창 포커스/블러 → 폴링 활성 토글
            if let Some(win) = app.get_webview_window("main") {
                let st = state.clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::Focused(focused) = ev {
                        st.polling_active.store(*focused, Ordering::SeqCst);
                    }
                });
            }

            // 백그라운드 폴링 루프
            let st = state.clone();
            tauri::async_runtime::spawn(async move {
                // 시작 시 1회: 스캔 + 상태 읽기 (포커스 게이팅과 무관하게 초기 표시)
                let _ = commands::do_scan(&st).await;
                let _ = commands::do_refresh(&handle, &st).await;

                loop {
                    let interval = st.config.lock().await.poll_interval_secs.clamp(5, 300) as u64;
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                    if scheduler::should_run_poll(
                        st.polling_active.load(Ordering::SeqCst),
                        st.in_flight.load(Ordering::SeqCst),
                    ) {
                        let _ = commands::do_refresh(&handle, &st).await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
