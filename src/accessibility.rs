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

        if AXUIElementCopyElementAtPosition(sys_wide.raw(), x, y, &mut raw_element) != K_AX_ERROR_SUCCESS {
            return None;
        }
        let element = AXElement::new(raw_element);

        let role_attr = CFString::from_static_string(K_AX_ROLE_ATTRIBUTE);
        let parent_attr = CFString::from_static_string(K_AX_PARENT_ATTRIBUTE);
        let target_role = CFString::from_static_string(K_AX_WINDOW_ROLE);

        let mut current = element.clone();

        while !current.raw().is_null() {
            let mut role_ref = std::ptr::null_mut();

            if AXUIElementCopyAttributeValue(current.raw(), role_attr.as_concrete_TypeRef(), &mut role_ref) == K_AX_ERROR_SUCCESS {
                let role_str = CFString::wrap_under_create_rule(role_ref as CFStringRef);

                if role_str.to_string() == target_role.to_string() {
                    return Some(current);
                }
            }

            let mut parent_ref = std::ptr::null_mut();

            if AXUIElementCopyAttributeValue(current.raw(), parent_attr.as_concrete_TypeRef(), &mut parent_ref) == K_AX_ERROR_SUCCESS {
                current = AXElement::new(parent_ref);
            } else {
                break;
            }
        }

        None
    }
}

pub fn focus_window(window: &AXElement) {
    if window.raw().is_null() { return; }

    unsafe {
        let main_attr = CFString::from_static_string(K_AX_MAIN_ATTRIBUTE);
        let raise_act = CFString::from_static_string(K_AX_RAISE_ACTION);

        let mut is_main_ref = std::ptr::null_mut();
        
        if AXUIElementCopyAttributeValue(window.raw(), main_attr.as_concrete_TypeRef(), &mut is_main_ref) == K_AX_ERROR_SUCCESS {
            let is_main = core_foundation::boolean::CFBoolean::wrap_under_create_rule(is_main_ref as _);

            if is_main == core_foundation::boolean::CFBoolean::true_value() {
                // If it is already main, we still want to ensure application is frontmost,
                // so we don't return early.
            }
        }

        let true_val = core_foundation::boolean::CFBoolean::true_value();
        let true_val_ref = true_val.as_concrete_TypeRef();

        let mut pid: libc::pid_t = 0;
        if AXUIElementGetPid(window.raw(), &mut pid) == K_AX_ERROR_SUCCESS {
            let app: cocoa::base::id = msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
            if app != cocoa::base::nil {
                let _: cocoa::base::BOOL = msg_send![app, activateWithOptions: cocoa::appkit::NSApplicationActivateIgnoringOtherApps];
            }
        }

        AXUIElementSetAttributeValue(window.raw(), main_attr.as_concrete_TypeRef(), true_val_ref as *mut _);
        AXUIElementPerformAction(window.raw(), raise_act.as_concrete_TypeRef());
    }
}
