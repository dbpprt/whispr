use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri::utils::WindowEffect;
use tauri::utils::config::WindowEffectsConfig;
use tauri_plugin_positioner::{Position as PositionerPosition, WindowExt};

const WINDOW_TITLE: &str = "whispr:overlay";

pub struct OverlayWindow {
    window: Option<WebviewWindow>,
}

impl Default for OverlayWindow {
    fn default() -> Self {
        Self { window: None }
    }
}

impl OverlayWindow {
    pub fn new() -> Self {
        Self { window: None }
    }

    pub fn create_window(&mut self, app_handle: &tauri::AppHandle) {
        let window = WebviewWindowBuilder::new(
            app_handle,
            WINDOW_TITLE,
            WebviewUrl::App("index.html".into())
        )
        .title("whispr")
        .inner_size(500.0, 120.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .effects(WindowEffectsConfig {
            effects: vec![
                // For macOS
                WindowEffect::HudWindow,
                // For Windows
                WindowEffect::Acrylic,
            ],
            state: None,
            radius: Some(16.0),
            color: None,
        })
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .resizable(false)
        //.theme(Some(tauri::Theme::Dark))
        //.shadow(true)
        //.title_bar_style(tauri::TitleBarStyle::Overlay)
        .build()
        .expect("Failed to create window");

        self.window = Some(window);

        if let Some(window) = &self.window {
            let _ = window.hide();
            let _ = window.hide_menu();
        }
    }

    pub fn show(&self) {
        if let Some(window) = &self.window {
            if let Err(e) = window.move_window(PositionerPosition::BottomRight)
                .and_then(|_| window.set_skip_taskbar(true))
                .and_then(|_| window.set_ignore_cursor_events(true))
                .and_then(|_| window.show())
            {
                eprintln!("Failed to show window: {}", e);
            } else {
                println!("Window shown successfully");

                if let Err(e) = window.hide_menu() {
                    eprintln!("Failed to hide window menu: {}", e);
                }

                if let Err(e) = window.set_always_on_top(true) {
                    eprintln!("Failed to set window always on top: {}", e);
                }
            }
        } else {
            eprintln!("No window exists to show");
        }
    }

    pub fn hide(&self) {
        if let Some(window) = &self.window {
            if let Err(e) = window.hide().and_then(|_| window.hide_menu()) {
                eprintln!("Failed to hide window: {}", e);
            } else {
                println!("Window hidden successfully");
            }
        }
    }
}
