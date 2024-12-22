use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri::utils::WindowEffect;
use log::{error, info};
use tauri::utils::config::WindowEffectsConfig;

const WINDOW_TITLE: &str = "whispr:overlay";

#[derive(Default)]
pub struct OverlayWindow {
    window: Option<WebviewWindow>,
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
        .inner_size(350.0, 85.0)
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
        .build()
        .expect("Failed to create window");

        self.window = Some(window);

        if let Some(window) = &self.window {
            let _ = window.hide();
            let _ = window.hide_menu();
        }
    }

    pub fn move_bottom_right(&self, margin: i32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = &self.window {
            let screen = window.current_monitor()?.unwrap();
            let screen_position = screen.position();
            let screen_size = screen.size();
            let window_size = window.outer_size()?;

            let x = screen_position.x + (screen_size.width as i32 - window_size.width as i32 - margin);
            let y = screen_position.y + (screen_size.height as i32 - window_size.height as i32 - margin);

            window.set_position(tauri::PhysicalPosition::new(x, y))?;
        }
        Ok(())
    }

    pub fn show(&self) {
        if let Some(window) = &self.window {
            if let Err(e) = self.move_bottom_right(40) {
                error!("Failed to move window to bottom right: {}", e);
            } else if let Err(e) = window.set_skip_taskbar(true) {
                error!("Failed to set window to skip taskbar: {}", e);
            } else if let Err(e) = window.set_ignore_cursor_events(true) {
                error!("Failed to set window to ignore cursor events: {}", e);
            } else if let Err(e) = window.show() {
                error!("Failed to show window: {}", e);
            } else {
                info!("Window shown successfully");

                if let Err(e) = window.hide_menu() {
                    error!("Failed to hide window menu: {}", e);
                }

                if let Err(e) = window.set_always_on_top(true) {
                    error!("Failed to set window always on top: {}", e);
                }
            }
        } else {
            error!("No window exists to show");
        }
    }

    pub fn hide(&self) {
        if let Some(window) = &self.window {
            if let Err(e) = window.hide().and_then(|_| window.hide_menu()) {
                error!("Failed to hide window: {}", e);
            } else {
                info!("Window hidden successfully");
            }
        }
    }
}
