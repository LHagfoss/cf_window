use std::ffi::c_void;
use core_foundation::string::{CFStringRef, CFString};
use core_foundation::base::TCFType;

pub type AXUIElementRef = *mut c_void;
pub type AXError = i32;

pub const K_AX_ERROR_SUCCESS: AXError = 0;
pub const K_AX_MAIN_ATTRIBUTE: &str = "AXMain";
pub const K_AX_ROLE_ATTRIBUTE: &str = "AXRole";
pub const K_AX_PARENT_ATTRIBUTE: &str = "AXParent";
pub const K_AX_RAISE_ACTION: &str = "AXRaise";
pub const K_AX_WINDOW_ROLE: &str = "AXWindow";

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCopyElementAtPosition(
        sys_element: AXUIElementRef,
        x: f32,
        y: f32,
        element: *mut AXUIElementRef,
    ) -> AXError;

    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut *mut c_void,
    ) -> AXError;

    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut c_void,
    ) -> AXError;

    pub fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;

    pub fn AXIsProcessTrusted() -> bool;

    pub fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut libc::pid_t) -> AXError;
    pub fn AXUIElementCreateApplication(pid: libc::pid_t) -> AXUIElementRef;
}

pub fn find_window_at(x: f32, y: f32) -> Option<AXUIElementRef> {
    unsafe {
        let sys_wide = AXUIElementCreateSystemWide();
        let mut element = std::ptr::null_mut();

        if AXUIElementCopyElementAtPosition(sys_wide, x, y, &mut element) != K_AX_ERROR_SUCCESS {
            core_foundation::base::CFRelease(sys_wide as *const _);
            return None;
        }

        let mut current = element;
        let mut window = std::ptr::null_mut();

        let role_attr = CFString::from_static_string(K_AX_ROLE_ATTRIBUTE);
        let parent_attr = CFString::from_static_string(K_AX_PARENT_ATTRIBUTE);
        let target_role = CFString::from_static_string(K_AX_WINDOW_ROLE);

        while !current.is_null() {
            let mut role_ref = std::ptr::null_mut();

            if AXUIElementCopyAttributeValue(current, role_attr.as_concrete_TypeRef(), &mut role_ref) == K_AX_ERROR_SUCCESS {
                let role_str = CFString::wrap_under_create_rule(role_ref as CFStringRef);

                if role_str.to_string() == target_role.to_string() {
                    window = current;
                    break;
                }
            }

            let mut parent_ref = std::ptr::null_mut();

            if AXUIElementCopyAttributeValue(current, parent_attr.as_concrete_TypeRef(), &mut parent_ref) == K_AX_ERROR_SUCCESS {
                if current != element && current != window {
                    core_foundation::base::CFRelease(current as *const _);
                }

                current = parent_ref as AXUIElementRef;
            } else {
                if current != element && current != window {
                    core_foundation::base::CFRelease(current as *const _);
                }
                break;
            }
        }

        if window.is_null() {
            if !element.is_null() { core_foundation::base::CFRelease(element as *const _); }
            core_foundation::base::CFRelease(sys_wide as *const _);
            None
        } else {
            if element != window { core_foundation::base::CFRelease(element as *const _); }
            core_foundation::base::CFRelease(sys_wide as *const _);
            Some(window)
        }
    }
}

pub fn focus_window(window: AXUIElementRef) {
    if window.is_null() { return; }

    unsafe {
        let main_attr = CFString::from_static_string(K_AX_MAIN_ATTRIBUTE);
        let raise_act = CFString::from_static_string(K_AX_RAISE_ACTION);

        let mut is_main_ref = std::ptr::null_mut();
        
        if AXUIElementCopyAttributeValue(window, main_attr.as_concrete_TypeRef(), &mut is_main_ref) == K_AX_ERROR_SUCCESS {
            let is_main = core_foundation::boolean::CFBoolean::wrap_under_create_rule(is_main_ref as _);

            if is_main == core_foundation::boolean::CFBoolean::true_value() {
                // If it is already the main window, we still want to make sure the app is frontmost!
                // So do not return early, just proceed to activate the app.
                println!("[DEBUG] Window is already main window.");
            }
        }

        let true_val = core_foundation::boolean::CFBoolean::true_value();
        let true_val_ref = true_val.as_concrete_TypeRef();

        // 1. Get the app's PID and make the app frontmost
        let mut pid: libc::pid_t = 0;
        let pid_err = AXUIElementGetPid(window, &mut pid);
        println!("[DEBUG] AXUIElementGetPid returned: {}, pid = {}", pid_err, pid);
        if pid_err == K_AX_ERROR_SUCCESS {
            let app_ref = AXUIElementCreateApplication(pid);
            if !app_ref.is_null() {
                let frontmost_attr = CFString::from_static_string("AXFrontmost");
                let app_err = AXUIElementSetAttributeValue(app_ref, frontmost_attr.as_concrete_TypeRef(), true_val_ref as *mut _);
                println!("[DEBUG] AXUIElementSetAttributeValue(AXFrontmost) returned: {}", app_err);
                core_foundation::base::CFRelease(app_ref as *const _);
            } else {
                println!("[DEBUG] AXUIElementCreateApplication returned NULL!");
            }
        }

        // 2. Set AXMain on the window
        let main_err = AXUIElementSetAttributeValue(window, main_attr.as_concrete_TypeRef(), true_val_ref as *mut _);
        println!("[DEBUG] AXUIElementSetAttributeValue(AXMain) returned: {}", main_err);

        // 3. Perform AXRaise action on the window to bring it to the front
        let raise_err = AXUIElementPerformAction(window, raise_act.as_concrete_TypeRef());
        println!("[DEBUG] AXUIElementPerformAction(AXRaise) returned: {}", raise_err);
    }
}
