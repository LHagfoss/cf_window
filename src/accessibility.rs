#![allow(deprecated)]

use std::ffi::c_void;
use core_foundation::string::{CFStringRef, CFString};
use core_foundation::base::TCFType;
use objc::{msg_send, class, sel, sel_impl};

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
}

pub struct AXElement(AXUIElementRef);

impl AXElement {
    pub unsafe fn new(raw: AXUIElementRef) -> Self {
        AXElement(raw)
    }

    pub fn raw(&self) -> AXUIElementRef {
        self.0
    }
}

impl Drop for AXElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                core_foundation::base::CFRelease(self.0 as *const _);
            }
        }
    }
}

impl Clone for AXElement {
    fn clone(&self) -> Self {
        if !self.0.is_null() {
            unsafe {
                core_foundation::base::CFRetain(self.0 as *const _);
            }
        }
        AXElement(self.0)
    }
}

impl PartialEq for AXElement {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_null() || other.0.is_null() {
            return self.0 == other.0;
        }
        unsafe {
            core_foundation::base::CFEqual(self.0 as *const _, other.0 as *const _) != 0
        }
    }
}

pub fn find_window_at(x: f32, y: f32) -> Option<AXElement> {
    unsafe {
        let sys_wide = AXElement::new(AXUIElementCreateSystemWide());
        let mut raw_element = std::ptr::null_mut();

        let err = AXUIElementCopyElementAtPosition(sys_wide.raw(), x, y, &mut raw_element);
        if err != K_AX_ERROR_SUCCESS {
            println!("[DEBUG] AXUIElementCopyElementAtPosition failed with code: {}", err);
            return None;
        }
        let element = AXElement::new(raw_element);

        let role_attr = CFString::from_static_string(K_AX_ROLE_ATTRIBUTE);
        let parent_attr = CFString::from_static_string(K_AX_PARENT_ATTRIBUTE);
        let target_role = CFString::from_static_string(K_AX_WINDOW_ROLE);

        let mut current = element.clone();
        let mut depth = 0;

        while !current.raw().is_null() {
            let mut role_ref = std::ptr::null_mut();
            let mut role_name = String::from("unknown");

            if AXUIElementCopyAttributeValue(current.raw(), role_attr.as_concrete_TypeRef(), &mut role_ref) == K_AX_ERROR_SUCCESS {
                let role_str = CFString::wrap_under_create_rule(role_ref as CFStringRef);
                role_name = role_str.to_string();
            }

            // Query title for debug purposes
            let title_attr = CFString::from_static_string("AXTitle");
            let mut title_ref = std::ptr::null_mut();
            let mut title_name = String::from("");
            if AXUIElementCopyAttributeValue(current.raw(), title_attr.as_concrete_TypeRef(), &mut title_ref) == K_AX_ERROR_SUCCESS {
                let title_str = CFString::wrap_under_create_rule(title_ref as CFStringRef);
                title_name = title_str.to_string();
            }

            println!("[DEBUG] Traversal depth {}: role = {}, title = {:?}", depth, role_name, title_name);

            if role_name == target_role.to_string() {
                println!("[DEBUG] Found AXWindow element!");
                return Some(current);
            }

            let mut parent_ref = std::ptr::null_mut();
            let parent_err = AXUIElementCopyAttributeValue(current.raw(), parent_attr.as_concrete_TypeRef(), &mut parent_ref);
            if parent_err == K_AX_ERROR_SUCCESS {
                current = AXElement::new(parent_ref);
                depth += 1;
            } else {
                println!("[DEBUG] Parent lookup failed at depth {} with code: {}", depth, parent_err);
                break;
            }
        }

        println!("[DEBUG] Traversal finished, no AXWindow found.");
        None
    }
}

pub fn focus_window(window: &AXElement) {
    if window.raw().is_null() { return; }

    unsafe {
        let main_attr = CFString::from_static_string(K_AX_MAIN_ATTRIBUTE);
        let raise_act = CFString::from_static_string(K_AX_RAISE_ACTION);

        let mut is_main_ref = std::ptr::null_mut();
        let mut is_main_bool = false;
        
        if AXUIElementCopyAttributeValue(window.raw(), main_attr.as_concrete_TypeRef(), &mut is_main_ref) == K_AX_ERROR_SUCCESS {
            let is_main = core_foundation::boolean::CFBoolean::wrap_under_create_rule(is_main_ref as _);
            is_main_bool = is_main == core_foundation::boolean::CFBoolean::true_value();
        }
        println!("[DEBUG] Focus window target: is_main = {}", is_main_bool);

        let true_val = core_foundation::boolean::CFBoolean::true_value();
        let true_val_ref = true_val.as_concrete_TypeRef();

        let mut pid: libc::pid_t = 0;
        let pid_err = AXUIElementGetPid(window.raw(), &mut pid);
        println!("[DEBUG] AXUIElementGetPid returned: {}, pid = {}", pid_err, pid);
        
        if pid_err == K_AX_ERROR_SUCCESS {
            let app: cocoa::base::id = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
            if app != cocoa::base::nil {
                let success: cocoa::base::BOOL = msg_send![app, activateWithOptions: cocoa::appkit::NSApplicationActivateIgnoringOtherApps];
                println!("[DEBUG] NSRunningApplication activateWithOptions returned: {}", success);
            } else {
                println!("[DEBUG] NSRunningApplication for pid {} is nil", pid);
            }
        }

        let main_err = AXUIElementSetAttributeValue(window.raw(), main_attr.as_concrete_TypeRef(), true_val_ref as *mut _);
        println!("[DEBUG] AXUIElementSetAttributeValue(AXMain) returned: {}", main_err);
        
        let raise_err = AXUIElementPerformAction(window.raw(), raise_act.as_concrete_TypeRef());
        println!("[DEBUG] AXUIElementPerformAction(AXRaise) returned: {}", raise_err);
    }
}
