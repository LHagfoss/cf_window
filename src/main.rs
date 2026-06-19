mod accessibility;

use std::sync::mpsc::channel;
use std::thread;
use core_graphics::event::{CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType};
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes, CFRunLoopRun};
use core_graphics::geometry::CGPoint;

fn main() {
    unsafe {
        if !accessibility::AXIsProcessTrusted() {
            println!("[-] Accessibility permissions missing! Please grant them in System Settings.");
            std::process::exit(1);
        }
    }

    let (tx, rx) = channel::<CGPoint>();

    thread::spawn(move || {
        println!("[+] Focus pipeline initialized.");

        let mut last_window: Option<accessibility::AXUIElementRef> = None;
        while let Ok(point) = rx.recv() {
            println!("[DEBUG] Worker thread received point: x={}, y={}", point.x, point.y);
            unsafe {
                if let Some(window) = accessibility::find_window_at(point.x as f32, point.y as f32) {
                    println!("[DEBUG] Found window at point!");
                    let is_same = match last_window {
                        Some(lw) => core_foundation::base::CFEqual(lw as *const _, window as *const _) != 0,
                        None => false,
                    };
                    println!("[DEBUG] is_same = {}", is_same);
                    if !is_same {
                        println!("[DEBUG] Focusing new window...");
                        accessibility::focus_window(window);
                        if let Some(lw) = last_window {
                            core_foundation::base::CFRelease(lw as *const _);
                        }
                        // Keep the window reference
                        last_window = Some(window);
                    } else {
                        // Same window, release the duplicate reference we got from find_window_at
                        core_foundation::base::CFRelease(window as *const _);
                    }
                } else {
                    println!("[DEBUG] No window found at point.");
                }
            }
        }
    });

    let tx_clone = tx.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::MouseMoved],
        move |_proxy, _type, event| {
            let location = event.location();
            println!("[DEBUG] MouseMoved callback: x={}, y={}", location.x, location.y);
            let _ = tx_clone.send(location);
            core_graphics::event::CallbackResult::Keep
        }
    ).expect("Failed to hook CGEventTap");

    let run_loop_source = tap.mach_port().create_runloop_source(0).expect("Failed to create run loop source");
    let current_loop = CFRunLoop::get_current();
    current_loop.add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });
    tap.enable();

    println!("[+] Rust Hover Focus Daemon running smoothly!");

    unsafe {
        CFRunLoopRun();
    }
}
