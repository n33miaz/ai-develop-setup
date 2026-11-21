//! Desktop window for the bundled interface and the local API.
//! No policy, storage or PTY lives here (see `defaults/dependency-matrix.toml`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    io::{self, Read},
    process::ExitCode,
};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use zeroize::{Zeroize, Zeroizing};

mod bridge;
#[cfg(windows)]
mod stdin;

const WINDOW_ORIGIN: &str = "http://tauri.localhost";
const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

struct Bootstrap {
    url: String,
    token: String,
}

impl Drop for Bootstrap {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

fn read_bootstrap(mut input: impl Read) -> io::Result<Bootstrap> {
    // Five port digits, a space, 43 token characters and a newline.
    let mut bytes = Zeroizing::new([0; 50]);
    let mut end = 0;
    for byte in bytes.iter_mut() {
        input.read_exact(std::slice::from_mut(byte))?;
        end += 1;
        if *byte == b'\n' {
            break;
        }
    }
    let (port, token) = std::str::from_utf8(&bytes[..end])
        .ok()
        .and_then(|line| line.strip_suffix('\n'))
        .and_then(|line| line.split_once(' '))
        .ok_or_else(invalid_bootstrap)?;
    let port: u16 = port.parse().map_err(|_| invalid_bootstrap())?;
    if port == 0
        || token.len() != 43
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(invalid_bootstrap());
    }
    Ok(Bootstrap {
        url: format!("ws://127.0.0.1:{port}"),
        token: token.to_owned(),
    })
}

fn invalid_bootstrap() -> io::Error {
    io::Error::other("invalid desktop bootstrap")
}

fn trusted_window(window: &WebviewWindow) -> bool {
    window.label() == "main"
        && window
            .url()
            .is_ok_and(|url| url.origin().ascii_serialization() == WINDOW_ORIGIN)
}

fn dimension(arg: &str) -> Option<f64> {
    arg.parse::<u16>()
        .ok()
        .filter(|value| *value != 0)
        .map(f64::from)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [width, height, flags @ ..] = args.as_slice() else {
        return Err("missing shell window dimensions".into());
    };
    let width = dimension(width).ok_or("invalid shell width")?;
    let height = dimension(height).ok_or("invalid shell height")?;
    let minimized = flags.iter().any(|arg| arg == "--minimized");
    let smoke = cfg!(feature = "desktop-smoke") && flags.iter().any(|arg| arg == "--smoke-test");
    #[cfg(windows)]
    let bootstrap = read_bootstrap(stdin::private_input()?)?;
    #[cfg(not(windows))]
    let bootstrap = read_bootstrap(io::stdin().lock())?;
    tauri::Builder::default()
        .manage(bridge::Bridge::new(bootstrap, smoke))
        .invoke_handler(tauri::generate_handler![
            bridge::desktop_connect,
            bridge::desktop_send,
            bridge::desktop_disconnect
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(move |app| {
            let mut window_config = app.config().app.windows[0].clone();
            window_config.width = width;
            window_config.height = height;
            window_config.url = WebviewUrl::App("index.html".into());
            let window = WebviewWindowBuilder::from_config(app, &window_config)?
                .on_navigation(|url| url.origin().ascii_serialization() == WINDOW_ORIGIN)
                .build()?;
            if minimized {
                window.minimize()?;
            }
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
                                let _ = window.unminimize();
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
        .run(tauri::generate_context!())?;
    Ok(())
}

fn main() -> ExitCode {
    if run().is_ok() {
        ExitCode::SUCCESS
    } else {
        eprintln!("ads-shell: desktop startup failed");
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_bounded_pipe_record() {
        let token = "a".repeat(43);
        let line = format!("65535 {token}\n");
        let bootstrap = read_bootstrap(line.as_bytes()).unwrap();
        assert_eq!(bootstrap.url, "ws://127.0.0.1:65535");
        assert!(bootstrap.token == token);
        for invalid in [
            format!("65536 {token}\n"),
            format!("0 {token}\n"),
            format!("80 {token}"),
            format!("80 !{token}\n"),
            format!("80 {token}a\n"),
        ] {
            assert!(read_bootstrap(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn menu_ids_are_distinct() {
        assert_ne!(MENU_SHOW, MENU_QUIT);
    }
}
