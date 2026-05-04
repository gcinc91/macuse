use std::ffi::c_void;
use std::sync::Arc;
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType,
};

use super::{transform, ScrollState};

/// Activado con MACUSE_DEBUG=1 al lanzar el proceso. Cuando esta off no
/// se loguean los detalles de cada evento, evitando filtrar patrones de
/// uso a un fichero de log persistente.
fn debug_logging_enabled() -> bool {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MACUSE_DEBUG")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
            .unwrap_or(false)
    })
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: *const c_void, enable: bool);
}

// Guardamos el puntero al CFMachPort del tap como usize (para que sea Send/Sync)
// y lo usamos para re-habilitar el tap si el sistema lo desactiva por timeout
// (callback demasiado lento). Sin esto el tap queda muerto y el scroll deja
// de invertirse silenciosamente.
//
// Invariante: el tap se crea una sola vez y vive lo que dura el proceso.
// `OnceLock` hace explicita esa invariante: si se intenta inicializar dos
// veces salta el debug_assert. Si en el futuro hay que reiniciar el tap,
// hay que rediseñar este almacenamiento (el puntero sobreviviria al drop
// del CFMachPort y un callback en vuelo haria UAF).
static TAP_PORT: OnceLock<usize> = OnceLock::new();

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
            // Un panic dentro de este callback cruzaria FFI hacia el run loop
            // de CoreGraphics. AssertUnwindSafe es necesario porque `event`
            // (CGEvent / CFType) no es UnwindSafe, y aceptamos la responsabilidad
            // de que un panic aqui solo deja el evento pasar tal cual.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let et = event_type as u32;
                if et == TAP_DISABLED_BY_TIMEOUT || et == TAP_DISABLED_BY_USER_INPUT {
                    if let Some(&port) = TAP_PORT.get() {
                        unsafe { CGEventTapEnable(port as *const c_void, true) };
                        crate::mlog!("tap reactivado tras TapDisabled");
                    }
                    return CallbackResult::Keep;
                }
                if et == SCROLL_WHEEL {
                    if debug_logging_enabled() {
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
                            return CallbackResult::Keep;
                        }
                    }
                    transform::apply(&state, event);
                }
                CallbackResult::Keep
            }))
            .unwrap_or_else(|_| {
                crate::mlog!("tap callback: panic capturado");
                CallbackResult::Keep
            })
        },
    )
    .map_err(|_| anyhow!("CGEventTap::new fallo - falta permiso de Accesibilidad?"))?;

    // Guardar mach_port para re-habilitacion desde el callback.
    let port_ref = tap.mach_port().as_concrete_TypeRef() as usize;
    if TAP_PORT.set(port_ref).is_err() {
        return Err(anyhow!("tap::start invocado dos veces; no soportado"));
    }

    let runloop_src = tap
        .mach_port()
        .create_runloop_source(0)
        .map_err(|_| anyhow!("create_runloop_source fallo"))?;

    unsafe {
        CFRunLoop::get_current().add_source(&runloop_src, kCFRunLoopCommonModes);
    }
    tap.enable();
    crate::mlog!("event tap habilitado en run loop");
    Ok(tap)
}
