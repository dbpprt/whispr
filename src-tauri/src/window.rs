use tauri::{Window, Manager};
use tauri_plugin_positioner::{WindowExt, Position as PositionerPosition};

const WINDOW_TITLE: &str = "whispr:overlay";

pub struct OverlayWindow {
    window: Option<Window>,
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
        println!("Creating window...");

        // Get the predefined window
        if let Some(window) = app_handle.get_window(WINDOW_TITLE) {
            println!("Window found successfully");
            
            // Set window to be non-interactive
            if let Err(e) = window.set_decorations(false) {
                eprintln!("Failed to set window decorations: {}", e);
            }
            
            self.window = Some(window);
            
            if let Some(window) = &self.window {
                let _ = window.hide();
                let _ = window.hide_menu();
            }
        } else {
            eprintln!("Failed to get {}", WINDOW_TITLE);
        }
    }

    pub fn show(&self) {
        println!("Attempting to show window...");
        if let Some(window) = &self.window {
            // Move window to top-right position
            // if let Err(e) = window.move_window(PositionerPosition::TopRight) {
            //     eprintln!("Failed to position window: {}", e);
            // }

            if let Err(e) = window.set_skip_taskbar(true) {
                eprintln!("Failed to set skip taskbar: {}", e);
            }

            if let Err(e) = window.set_ignore_cursor_events(true) {
                eprintln!("Failed to set ignore cursor events: {}", e);
            }

            // Show window
            if let Err(e) = window.show() {
                eprintln!("Failed to show window: {}", e);
            } else {
                println!("Window shown successfully");
                
                if let Err(e) = window.hide_menu() {
                    eprintln!("Failed to hide window menu: {}", e);
                }

                // Ensure window is on top
                if let Err(e) = window.set_always_on_top(true) {
                    eprintln!("Failed to set window always on top: {}", e);
                }
            }
        } else {
            eprintln!("No window exists to show");
        }
    }

    pub fn hide(&self) {
        println!("Attempting to hide window...");
        if let Some(window) = &self.window {
            if let Err(e) = window.hide() {
                eprintln!("Failed to hide window: {}", e);
            } else {
                println!("Window hidden successfully");

                if let Err(e) = window.hide_menu() {
                    eprintln!("Failed to hide window menu: {}", e);
                }
            }
        }
    }
}
