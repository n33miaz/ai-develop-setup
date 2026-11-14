//! Tauri shell skeleton: an empty window that loads `ui/dist` and a tray icon with a minimal menu.
//! No policy, storage or PTY lives here (see `defaults/dependency-matrix.toml`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Provisional English labels until the shell gets the i18n catalog.
            let show = MenuItem::with_id(app, MENU_SHOW, "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray =
                TrayIconBuilder::new()
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        MENU_SHOW => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        MENU_QUIT => app.exit(0),
                        _ => {}
                    });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the shell failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_are_distinct() {
        assert_ne!(MENU_SHOW, MENU_QUIT);
    }
}
