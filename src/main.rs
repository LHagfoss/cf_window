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
        let mut last_window: Option<accessibility::AXElement> = None;
        while let Ok(point) = rx.recv() {
            if let Some(window) = accessibility::find_window_at(point.x as f32, point.y as f32) {
                let is_same = match &last_window {
                    Some(lw) => lw == &window,
                    None => false,
                };
                if !is_same {
                    accessibility::focus_window(&window);
                    last_window = Some(window);
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
