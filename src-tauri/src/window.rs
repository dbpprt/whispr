use tauri::{Window, Manager, Position};
use tauri_plugin_positioner::{WindowExt, Position as PositionerPosition};

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
        if let Some(window) = app_handle.get_window("main") {
            println!("Window found successfully");
            self.window = Some(window);
        } else {
            eprintln!("Failed to get main window");
        }
    }

    pub fn show(&self) {
        println!("Attempting to show window...");
        if let Some(window) = &self.window {
            // Move window to top-right position
            if let Err(e) = window.move_window(PositionerPosition::TopRight) {
                eprintln!("Failed to position window: {}", e);
            }

            // Show window
            if let Err(e) = window.show() {
                eprintln!("Failed to show window: {}", e);
            } else {
                println!("Window shown successfully");

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
            }
        }
    }
}
