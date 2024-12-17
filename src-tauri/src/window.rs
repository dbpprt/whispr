use tauri::{Window, Manager, PhysicalPosition, Position};

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
            // Get the app handle from the window
            let app_handle = window.app_handle();
            
            // Try to get the focused window's monitor
            if let Some(focused_window) = app_handle.windows().values().find(|w| w.is_focused().unwrap_or(false)) {
                println!("Found focused window");
                if let Ok(Some(focused_monitor)) = focused_window.current_monitor() {
                    let monitor_size = focused_monitor.size();
                    let monitor_pos = focused_monitor.position();
                    let window_width = 300.0;
                    let window_height = 40.0;
                    
                    // Calculate center position relative to the focused monitor
                    let x = monitor_pos.x as f64 + (monitor_size.width as f64 - window_width) / 2.0;
                    let y = monitor_pos.y as f64 + (monitor_size.height as f64 - window_height) / 2.0;
                    
                    println!("Using focused monitor: {:?}", focused_monitor.name());
                    println!("Monitor position: {:?}, size: {:?}", monitor_pos, monitor_size);
                    println!("Setting window position to x:{}, y:{}", x, y);
                    
                    // Position window
                    if let Err(e) = window.set_position(Position::Physical(PhysicalPosition { 
                        x: x.round() as i32, 
                        y: y.round() as i32 
                    })) {
                        eprintln!("Failed to position window: {}", e);
                    }
                } else {
                    eprintln!("Could not get focused window's monitor");
                }
            } else {
                eprintln!("No focused window found");
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
