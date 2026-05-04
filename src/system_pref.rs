use std::ffi::c_void;

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFPreferencesCopyValue(
        key: CFStringRef,
        application: CFStringRef,
        user_name: CFStringRef,
        host_name: CFStringRef,
    ) -> *const c_void;
    static kCFPreferencesAnyApplication: CFStringRef;
    static kCFPreferencesCurrentUser: CFStringRef;
    static kCFPreferencesAnyHost: CFStringRef;
}

const KEY: &str = "com.apple.swipescrolldirection";

/// Lee `com.apple.swipescrolldirection` del NSGlobalDomain via Core Foundation.
/// `true` = natural scrolling activado (default macOS).
pub fn is_natural_scrolling_enabled() -> bool {
    unsafe {
        let key = CFString::new(KEY);
        let value = CFPreferencesCopyValue(
            key.as_concrete_TypeRef(),
            kCFPreferencesAnyApplication,
            kCFPreferencesCurrentUser,
            kCFPreferencesAnyHost,
        );
        if value.is_null() {
            return true; // default macOS = natural ON
        }
        let cf = CFType::wrap_under_create_rule(value as CFTypeRef);
        if let Some(b) = cf.downcast::<CFBoolean>() {
            return b.into();
        }
        if let Some(n) = cf.downcast::<CFNumber>() {
            if let Some(v) = n.to_i64() {
                return v != 0;
            }
        }
        true
    }
}
