use cocoa::base::id;
use log::{info, debug};
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::Sel;
use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use crate::config::WhisprConfig;

type NSUInteger = libc::c_ulong;

const NSEVENT_MASK_FLAGS_CHANGED: NSUInteger = 1 << 12;

pub struct HotkeyManager {
    monitors: Vec<*mut std::ffi::c_void>,
    callback: Arc<dyn Fn(bool) + Send + Sync>,
    key_code: u16,
    key_mask: NSUInteger,
}

impl HotkeyManager {
    pub fn new<F>(callback: F, config: WhisprConfig) -> Self 
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        debug!("HotkeyManager: Initializing");
        let (key_code, key_mask) = Self::get_key_code_and_mask(&config.keyboard_shortcut);
        debug!("HotkeyManager: Using key_code: {}, key_mask: {}, and shortcut: {}", key_code, key_mask, config.keyboard_shortcut);
        HotkeyManager {
            monitors: Vec::new(),
            callback: Arc::new(callback),
            key_code,
            key_mask,
        }
    }

    fn get_key_code_and_mask(shortcut: &str) -> (u16, NSUInteger) {
        let key_map: HashMap<&str, (u16, NSUInteger)> = [
            // Key mappings for different shortcuts
            ("right_option_key", (61, 1 << 19)), // Right Option key
            ("right_command_key", (54, 1 << 20)), // Right Command key
            // Add more key mappings as needed
        ]
        .iter()
        .cloned()
        .collect();

        *key_map.get(shortcut).unwrap()
    }

    fn add_monitor(&mut self, monitor_selector: Sel) -> Result<()> {
        let callback = self.callback.clone();
        let key_code = self.key_code;
        let key_mask = self.key_mask;
        let monitor: id = unsafe {
            let handler = block::ConcreteBlock::new(move |event: id| {
                if !event.is_null() {
                    let event_key_code: u16 = msg_send![event, keyCode];
                    if event_key_code == key_code {
                        let flags: NSUInteger = msg_send![event, modifierFlags];
                        let is_pressed = flags & key_mask != 0;
                        debug!("HotkeyManager: Key - pressed: {}", is_pressed);
                        callback(is_pressed);
                    }
                }
            })
            .copy();
            
            msg_send![class!(NSEvent), performSelector:monitor_selector 
                withObject:NSEVENT_MASK_FLAGS_CHANGED 
                withObject:handler]
        };

        if monitor.is_null() {
            return Err(anyhow::anyhow!("Failed to create event monitor"));
        }

        self.monitors.push(monitor as *mut std::ffi::c_void);
        debug!("HotkeyManager: Event monitor created");
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        info!("HotkeyManager: Starting event monitors");
        self.add_monitor(sel!(addGlobalMonitorForEventsMatchingMask:handler:))?;
        self.add_monitor(sel!(addLocalMonitorForEventsMatchingMask:handler:))?;
        Ok(())
    }
}
