use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use anyhow::Result;
use std::sync::Arc;

type NSUInteger = libc::c_ulong;

struct EventHandler {
    callback: Arc<dyn Fn(bool) + Send + Sync>,
}

impl EventHandler {
    fn new(callback: Arc<dyn Fn(bool) + Send + Sync>) -> Self {
        EventHandler { callback }
    }

    fn handle_event(&self, event: id) {
        if !event.is_null() {
            let key_code: u16 = unsafe { msg_send![event, keyCode] };
            
            // Only handle right option key
            if key_code == RIGHT_OPTION_KEY_CODE {
                let flags: NSUInteger = unsafe { msg_send![event, modifierFlags] };
                let is_pressed = (flags & OPTION_KEY_FLAG) != 0  // Option key flag
                    && (flags & RIGHT_OPTION_MASK) != 0;  // Right option key mask
                
                println!("HotkeyManager: Right Option key - pressed: {}", is_pressed);
                (self.callback)(is_pressed);
            }
        }
    }
}

pub struct HotkeyManager {
    #[allow(dead_code)]
    global_monitor: Option<*mut std::ffi::c_void>, // Keep global monitor alive as raw pointer
    #[allow(dead_code)]
    local_monitor: Option<*mut std::ffi::c_void>, // Keep local monitor alive as raw pointer
    event_handler: Arc<EventHandler>,
}

const NSEVENT_MASK_FLAGS_CHANGED: NSUInteger = 1 << 12;
const RIGHT_OPTION_KEY_CODE: u16 = 61;
const OPTION_KEY_FLAG: NSUInteger = 1 << 19;
const RIGHT_OPTION_MASK: NSUInteger = 0x040;

impl HotkeyManager {
    pub fn new<F>(callback: F) -> Self 
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        println!("HotkeyManager: Initializing");
        let event_handler = Arc::new(EventHandler::new(Arc::new(callback)));
        HotkeyManager {
            global_monitor: None,
            local_monitor: None,
            event_handler,
        }
    }
    
    pub fn start(&mut self) -> Result<()> {
        println!("HotkeyManager: Starting event monitors");
        let event_handler = self.event_handler.clone();

        unsafe {
            let global_monitor: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: NSEVENT_MASK_FLAGS_CHANGED
                handler: block::ConcreteBlock::new(move |event: id| {
                    event_handler.handle_event(event);
                })
                .copy()
            ];

            if global_monitor.is_null() {
                println!("HotkeyManager: Failed to create global event monitor");
                return Err(anyhow::anyhow!("Failed to create global event monitor"));
            }

            println!("HotkeyManager: Global event monitor created successfully");
            self.global_monitor = Some(global_monitor as *mut std::ffi::c_void);
        }

        let event_handler = self.event_handler.clone();

        unsafe {
            let local_monitor: id = msg_send![
                class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: NSEVENT_MASK_FLAGS_CHANGED
                handler: block::ConcreteBlock::new(move |event: id| {
                    event_handler.handle_event(event);
                })
                .copy()
            ];

            if local_monitor.is_null() {
                println!("HotkeyManager: Failed to create local event monitor");
                return Err(anyhow::anyhow!("Failed to create local event monitor"));
            }

            println!("HotkeyManager: Local event monitor created successfully");
            self.local_monitor = Some(local_monitor as *mut std::ffi::c_void);
        }

        Ok(())
    }
}
