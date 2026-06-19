#![allow(deprecated)]

mod accessibility;

use std::sync::mpsc::channel;
use std::thread;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType};
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_foundation::base::TCFType;
use core_graphics::geometry::CGPoint;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapEnable(tap: *const std::ffi::c_void, enable: bool);
}

use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory};
use tray_icon::{TrayIconBuilder, Icon};
use tray_icon::menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem, MenuEvent};

static ENABLED: AtomicBool = AtomicBool::new(true);
static TAP_PORT: AtomicUsize = AtomicUsize::new(0);

fn create_dummy_icon() -> Icon {
    let width = 18;
    let height = 18;
    let mut rgba = vec![0; (width * height * 4) as usize];
    
    let cx = 9.0f32;
    let cy = 9.0f32;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * width + x) * 4) as usize;
            
            // Draw a template focus ring (pure black color with alpha mask)
            if (dist >= 5.5 && dist <= 7.5) || dist <= 2.2 {
                rgba[idx] = 0;       // R
                rgba[idx + 1] = 0;   // G
                rgba[idx + 2] = 0;   // B
                rgba[idx + 3] = 255; // A (Opaque)
            } else {
                rgba[idx + 3] = 0;   // A (Transparent)
            }
        }
    }
    
    Icon::from_rgba(rgba, width, height).unwrap()
}

fn main() {
    unsafe {
        if !accessibility::AXIsProcessTrusted() {
            println!("[-] Accessibility permissions missing! Please grant them in System Settings.");
            std::process::exit(1);
        }
    }

    // 1. Initialize NSApplication and hide the Dock icon
    let app = unsafe {
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
        app
    };

    let (tx, rx) = channel::<CGPoint>();

    // 2. Spawn worker thread to handle hover window focusing
    thread::spawn(move || {
        let mut last_window: Option<accessibility::AXElement> = None;
        while let Ok(point) = rx.recv() {
            if !ENABLED.load(Ordering::SeqCst) {
                last_window = None;
                continue;
            }

            // Negative coordinates represent a cache-invalidation signal (from clicks/typing)
            if point.x < 0.0 || point.y < 0.0 {
                println!("[DEBUG] User input event detected. Invalidating focus cache.");
                last_window = None;
                continue;
            }

            println!("[DEBUG] Mouse position: x={}, y={}", point.x, point.y);

            if let Some(window) = accessibility::find_window_at(point.x as f32, point.y as f32) {
                let is_same = match &last_window {
                    Some(lw) => lw == &window,
                    None => false,
                };
                println!("[DEBUG] Window found. is_same = {}", is_same);
                
                if !is_same {
                    println!("[DEBUG] Focusing window...");
                    accessibility::focus_window(&window);
                    last_window = Some(window);
                }
            } else {
                println!("[DEBUG] No window found at coordinate.");
            }
        }
    });

    // 3. Set up the menu bar tray item
    let menu = Menu::new();
    let toggle_item = CheckMenuItem::new("Hover Focus Enabled", true, true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    
    let _ = menu.append(&toggle_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit_item);
    
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Hover Focus Daemon")
        .with_icon(create_dummy_icon())
        .with_icon_as_template(true)
        .build()
        .unwrap();

    let toggle_id = toggle_item.id().clone();
    let quit_id = quit_item.id().clone();

    // 4. Spawn thread to handle menu events
    thread::spawn(move || {
        let menu_channel = MenuEvent::receiver();
        while let Ok(event) = menu_channel.recv() {
            if event.id == toggle_id {
                let prev = ENABLED.fetch_xor(true, Ordering::SeqCst);
                let new_state = !prev;
                if new_state {
                    println!("[+] Hover focus enabled.");
                } else {
                    println!("[-] Hover focus disabled.");
                }
            } else if event.id == quit_id {
                std::process::exit(0);
            }
        }
    });

    // 5. Build and enable CGEventTap to capture global mouse moves, clicks, and keystrokes
    let tx_clone = tx.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::MouseMoved,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::KeyDown,
        ],
        move |_proxy, event_type, event| {
            let et = event_type as u32;
            if et == CGEventType::TapDisabledByTimeout as u32 || et == CGEventType::TapDisabledByUserInput as u32 {
                let port_ref = TAP_PORT.load(Ordering::SeqCst);
                if port_ref != 0 {
                    unsafe {
                        CGEventTapEnable(port_ref as *const _, true);
                    }
                    println!("[+] Event tap disabled by OS, re-enabled.");
                }
                return core_graphics::event::CallbackResult::Keep;
            }

            if et == CGEventType::LeftMouseDown as u32 
                || et == CGEventType::RightMouseDown as u32 
                || et == CGEventType::KeyDown as u32 
            {
                // Invalidate focus cache upon manual user interaction (click/keystroke)
                let _ = tx_clone.send(CGPoint::new(-1.0, -1.0));
            } else if et == CGEventType::MouseMoved as u32 {
                let location = event.location();
                let _ = tx_clone.send(location);
            }
            core_graphics::event::CallbackResult::Keep
        }
    ).expect("Failed to hook CGEventTap");

    TAP_PORT.store(tap.mach_port().as_concrete_TypeRef() as usize, Ordering::SeqCst);

    let run_loop_source = tap.mach_port().create_runloop_source(0).expect("Failed to create run loop source");
    let current_loop = CFRunLoop::get_current();
    current_loop.add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    println!("[+] Rust Hover Focus Daemon running in system menu bar!");

    // 6. Run the AppKit application event loop
    unsafe {
        app.run();
    }
}
