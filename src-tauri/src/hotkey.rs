use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use anyhow::Result;
use std::sync::Arc;

type NSUInteger = libc::c_ulong;

pub struct HotkeyManager {
    #[allow(dead_code)]
    monitor: Option<*mut std::ffi::c_void>, // Keep monitor alive as raw pointer
    callback: Arc<dyn Fn(bool) + Send + Sync>,
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
        HotkeyManager {
            monitor: None,
            callback: Arc::new(callback),
        }
    }
    
    pub fn start(&mut self) -> Result<()> {
        println!("HotkeyManager: Starting event monitor");
        let callback = self.callback.clone();

        unsafe {
            let monitor: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: NSEVENT_MASK_FLAGS_CHANGED
                handler: block::ConcreteBlock::new(move |event: id| {
                    if !event.is_null() {
                        let key_code: u16 = msg_send![event, keyCode];
                        
                        // Only handle right option key
                        if key_code == RIGHT_OPTION_KEY_CODE {
                            let flags: NSUInteger = msg_send![event, modifierFlags];
                            let is_pressed = (flags & OPTION_KEY_FLAG) != 0  // Option key flag
                                && (flags & RIGHT_OPTION_MASK) != 0;  // Right option key mask
                            
                            println!("HotkeyManager: Right Option key - pressed: {}", is_pressed);
                            callback(is_pressed);
                        }
                    }
                })
                .copy()
            ];

            if monitor.is_null() {
                println!("HotkeyManager: Failed to create event monitor");
                return Err(anyhow::anyhow!("Failed to create event monitor"));
            }

            println!("HotkeyManager: Event monitor created successfully");
            self.monitor = Some(monitor as *mut std::ffi::c_void);
        }

        Ok(())
    }
}
