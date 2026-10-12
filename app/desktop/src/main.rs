// Prevents a console window from opening alongside the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Remembers which files the user has picked, so recent files reopen after a restart.
        .plugin(tauri_plugin_persisted_scope::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.with_webview(allow_audio_input)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running gtrTab");
}

/// Lets the page open a microphone or line in, for the tuner. The webview on Linux refuses
/// any device unless the app says otherwise; it is allowed sound, and nothing else.
/// Windows and macOS ask the user themselves.
#[cfg(target_os = "linux")]
fn allow_audio_input(webview: tauri::webview::PlatformWebview) {
    use webkit2gtk::glib::prelude::Cast;
    use webkit2gtk::{
        PermissionRequestExt, SettingsExt, UserMediaPermissionRequest,
        UserMediaPermissionRequestExt, WebViewExt,
    };

    let view = webview.inner();
    if let Some(settings) = WebViewExt::settings(&view) {
        settings.set_enable_media_stream(true);
    }
    view.connect_permission_request(|_, request| {
        match request.downcast_ref::<UserMediaPermissionRequest>() {
            Some(media) if media.is_for_audio_device() && !media.is_for_video_device() => {
                request.allow();
                true
            }
            _ => false,
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn allow_audio_input(_webview: tauri::webview::PlatformWebview) {}
