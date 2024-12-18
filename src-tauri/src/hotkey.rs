use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::Sel;
use anyhow::Result;
use std::sync::Arc;

type NSUInteger = libc::c_ulong;

const NSEVENT_MASK_FLAGS_CHANGED: NSUInteger = 1 << 12;
const RIGHT_OPTION_KEY_CODE: u16 = 61;
const OPTION_KEY_FLAG: NSUInteger = 1 << 19;
const RIGHT_OPTION_MASK: NSUInteger = 0x040;

pub struct HotkeyManager {
    monitors: Vec<*mut std::ffi::c_void>,
    callback: Arc<dyn Fn(bool) + Send + Sync>,
}

impl HotkeyManager {
    pub fn new<F>(callback: F) -> Self 
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        println!("HotkeyManager: Initializing");
        HotkeyManager {
            monitors: Vec::new(),
            callback: Arc::new(callback),
        }
    }

    fn add_monitor(&mut self, monitor_selector: Sel) -> Result<()> {
        let callback = self.callback.clone();
        let monitor: id = unsafe {
            let handler = block::ConcreteBlock::new(move |event: id| {
                if !event.is_null() {
                    let key_code: u16 = msg_send![event, keyCode];
                    if key_code == RIGHT_OPTION_KEY_CODE {
                        let flags: NSUInteger = msg_send![event, modifierFlags];
                        let is_pressed = (flags & OPTION_KEY_FLAG != 0) && (flags & RIGHT_OPTION_MASK != 0);
                        println!("HotkeyManager: Right Option key - pressed: {}", is_pressed);
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
        println!("HotkeyManager: Event monitor created");
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        println!("HotkeyManager: Starting event monitors");
        self.add_monitor(sel!(addGlobalMonitorForEventsMatchingMask:handler:))?;
        self.add_monitor(sel!(addLocalMonitorForEventsMatchingMask:handler:))?;
        Ok(())
    }
}
