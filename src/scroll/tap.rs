use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
};

use super::{transform, ScrollState};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: *const c_void, enable: bool);
}

// Guardamos el puntero al CFMachPort del tap como usize (para que sea Send/Sync)
// y lo usamos para re-habilitar el tap si el sistema lo desactiva por timeout
// (callback demasiado lento). Sin esto el tap queda muerto y el scroll deja
// de invertirse silenciosamente.
static TAP_PORT: AtomicUsize = AtomicUsize::new(0);

const SCROLL_WHEEL: u32 = CGEventType::ScrollWheel as u32;
const TAP_DISABLED_BY_TIMEOUT: u32 = CGEventType::TapDisabledByTimeout as u32;
const TAP_DISABLED_BY_USER_INPUT: u32 = CGEventType::TapDisabledByUserInput as u32;

pub fn start(state: Arc<ScrollState>) -> Result<CGEventTap<'static>> {
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::ScrollWheel],
        move |_proxy, event_type, event| {
            let et = event_type as u32;
            if et == TAP_DISABLED_BY_TIMEOUT || et == TAP_DISABLED_BY_USER_INPUT {
                let port = TAP_PORT.load(Ordering::Relaxed);
                if port != 0 {
                    unsafe { CGEventTapEnable(port as *const c_void, true) };
                    crate::mlog!("tap reactivado tras TapDisabled");
                }
                return None;
            }
            if et == SCROLL_WHEEL {
                static COUNTER: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);
                let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 200 {
                    use core_graphics::event::EventField;
                    let cont = event.get_integer_value_field(
                        EventField::SCROLL_WHEEL_EVENT_IS_CONTINUOUS);
                    let line_y = event.get_integer_value_field(
                        EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
                    let pt_y = event.get_integer_value_field(
                        EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
                    let fx_y = event.get_double_value_field(
                        EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1);
                    crate::mlog!(
                        "evt#{} cont={} pre line={} pt={} fx={:.2}",
                        n, cont, line_y, pt_y, fx_y
                    );
                    transform::apply(&state, event);
                    let line_y2 = event.get_integer_value_field(
                        EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
                    let pt_y2 = event.get_integer_value_field(
                        EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
                    let fx_y2 = event.get_double_value_field(
                        EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1);
                    crate::mlog!(
                        "evt#{} post line={} pt={} fx={:.2}",
                        n, line_y2, pt_y2, fx_y2
                    );
                } else {
                    transform::apply(&state, event);
                }
            }
            None
        },
    )
    .map_err(|_| anyhow!("CGEventTap::new fallo - falta permiso de Accesibilidad?"))?;

    // Guardar mach_port para re-habilitacion desde el callback.
    let port_ref = tap.mach_port.as_concrete_TypeRef() as usize;
    TAP_PORT.store(port_ref, Ordering::Relaxed);

    let runloop_src = tap
        .mach_port
        .create_runloop_source(0)
        .map_err(|_| anyhow!("create_runloop_source fallo"))?;

    unsafe {
        CFRunLoop::get_current().add_source(&runloop_src, kCFRunLoopCommonModes);
    }
    tap.enable();
    crate::mlog!("event tap habilitado en run loop");
    Ok(tap)
}
