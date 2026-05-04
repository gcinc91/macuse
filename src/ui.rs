//! UI nativa Cocoa con layout manual de frames absolutos.

use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use core_graphics::event::CGEventTap;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

use crate::config;
use crate::login_item;
use crate::permissions;
use crate::scroll::{tap, ScrollState};

const TAG_TRACKPAD: i64 = 1;
const TAG_MOUSE: i64 = 2;
const TAG_LOGIN: i64 = 3;
const TAG_OPEN_ACCESS: i64 = 4;
const TAG_RECHECK: i64 = 5;

const STATE_OFF: i64 = 0;
const STATE_ON: i64 = 1;

// Layout
const WIN_W: f64 = 540.0;
const WIN_H: f64 = 420.0;
const MARGIN: f64 = 24.0;
const BANNER_H: f64 = 72.0;
const SECTION_H: f64 = 76.0;

struct Globals {
    state: Arc<ScrollState>,
    tap: Option<CGEventTap<'static>>,
    banner: id,
    header: id,
    trackpad_section: id,
    mouse_section: id,
}

thread_local! {
    static G: RefCell<Option<Globals>> = const { RefCell::new(None) };
}

unsafe fn ns(s: &str) -> id {
    NSString::alloc(nil).init_str(s)
}

unsafe fn rgb(r: f64, g: f64, b: f64) -> id {
    msg_send![class!(NSColor),
        colorWithSRGBRed: r green: g blue: b alpha: 1.0_f64]
}

extern "C" fn on_action(_this: &mut Object, _: Sel, sender: id) {
    unsafe {
        let tag: i64 = msg_send![sender, tag];
        match tag {
            TAG_TRACKPAD => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                G.with(|g| {
                    if let Some(g) = g.borrow().as_ref() {
                        g.state.trackpad_natural.store(on_b, Ordering::Relaxed);
                    }
                });
                let mut cfg = config::load();
                cfg.trackpad_natural = on_b;
                let _ = config::save(&cfg);
            }
            TAG_MOUSE => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                G.with(|g| {
                    if let Some(g) = g.borrow().as_ref() {
                        g.state.mouse_natural.store(on_b, Ordering::Relaxed);
                    }
                });
                let mut cfg = config::load();
                cfg.mouse_natural = on_b;
                let _ = config::save(&cfg);
            }
            TAG_LOGIN => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                let mut cfg = config::load();
                cfg.login_at_start = on_b;
                let _ = config::save(&cfg);
                if on_b {
                    let _ = login_item::install();
                } else {
                    let _ = login_item::uninstall();
                }
            }
            TAG_OPEN_ACCESS => permissions::open_accessibility_pane(),
            TAG_RECHECK => {
                if permissions::is_trusted() {
                    let needs_relayout = G.with(|g| {
                        let mut g = g.borrow_mut();
                        if let Some(g) = g.as_mut() {
                            if g.tap.is_none() {
                                if let Ok(t) = tap::start(g.state.clone()) {
                                    g.tap = Some(t);
                                }
                            }
                            if g.tap.is_some() && g.banner != nil {
                                let _: () = msg_send![g.banner, setHidden: YES];
                                return true;
                            }
                        }
                        false
                    });
                    if needs_relayout {
                        relayout_after_banner_hidden();
                    }
                }
            }
            _ => {}
        }
    }
}

extern "C" fn application_should_terminate_after_last_window_closed(
    _: &mut Object,
    _: Sel,
    _: id,
) -> bool {
    true
}

fn delegate_class() -> &'static Class {
    use std::sync::OnceLock;
    static CLASS: OnceLock<&'static Class> = OnceLock::new();
    CLASS.get_or_init(|| {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MacuseDelegate", superclass).unwrap();
        unsafe {
            decl.add_method(
                sel!(onAction:),
                on_action as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(applicationShouldTerminateAfterLastWindowClosed:),
                application_should_terminate_after_last_window_closed
                    as extern "C" fn(&mut Object, Sel, id) -> bool,
            );
        }
        decl.register()
    })
}

unsafe fn make_label(text: &str, frame: NSRect, font_size: f64, bold: bool, multiline: bool) -> id {
    let label: id = msg_send![class!(NSTextField), labelWithString: ns(text)];
    let _: () = msg_send![label, setFrame: frame];
    let font: id = if bold {
        msg_send![class!(NSFont), boldSystemFontOfSize: font_size]
    } else {
        msg_send![class!(NSFont), systemFontOfSize: font_size]
    };
    let _: () = msg_send![label, setFont: font];
    if multiline {
        let _: () = msg_send![label, setMaximumNumberOfLines: 0_i64];
        let _: () = msg_send![label, setLineBreakMode: 0_i64];
    }
    label
}

unsafe fn set_text_color(label: id, color: id) {
    let _: () = msg_send![label, setTextColor: color];
}

unsafe fn make_switch(target: id, tag: i64, on: bool, frame: NSRect) -> id {
    let sw: id = msg_send![class!(NSSwitch), new];
    let _: () = msg_send![sw, setState: if on { STATE_ON } else { STATE_OFF }];
    let _: () = msg_send![sw, setTarget: target];
    let _: () = msg_send![sw, setAction: sel!(onAction:)];
    let _: () = msg_send![sw, setTag: tag];
    let _: () = msg_send![sw, setFrame: frame];
    sw
}

/// Seccion: panel con borde redondo, titulo en negrita, subtitulo gris debajo,
/// switch a la derecha centrado verticalmente.
unsafe fn make_section(
    target: id,
    title: &str,
    sub: &str,
    tag: i64,
    on: bool,
    frame: NSRect,
) -> id {
    let panel: id = msg_send![class!(NSView), new];
    let _: () = msg_send![panel, setFrame: frame];
    let _: () = msg_send![panel, setWantsLayer: YES];
    let layer: id = msg_send![panel, layer];

    let bg: id = msg_send![class!(NSColor), windowBackgroundColor];
    let bg_cg: id = msg_send![bg, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_cg];
    let _: () = msg_send![layer, setCornerRadius: 8.0_f64];
    let _: () = msg_send![layer, setBorderWidth: 1.0_f64];
    let border: id = msg_send![class!(NSColor), separatorColor];
    let border_cg: id = msg_send![border, CGColor];
    let _: () = msg_send![layer, setBorderColor: border_cg];

    let pw = frame.size.width;
    let ph = frame.size.height;

    // Switch a la derecha
    let sw_w = 40.0;
    let sw_h = 22.0;
    let sw_frame = NSRect::new(
        NSPoint::new(pw - sw_w - 16.0, (ph - sw_h) / 2.0),
        NSSize::new(sw_w, sw_h),
    );
    let switch = make_switch(target, tag, on, sw_frame);

    // Titulo
    let title_frame = NSRect::new(
        NSPoint::new(16.0, ph / 2.0 + 2.0),
        NSSize::new(pw - sw_w - 48.0, 22.0),
    );
    let title_lbl = make_label(title, title_frame, 14.0, true, false);

    // Subtitulo
    let sub_frame = NSRect::new(
        NSPoint::new(16.0, ph / 2.0 - 22.0),
        NSSize::new(pw - sw_w - 48.0, 18.0),
    );
    let sub_lbl = make_label(sub, sub_frame, 12.0, false, false);
    let secondary: id = msg_send![class!(NSColor), secondaryLabelColor];
    set_text_color(sub_lbl, secondary);

    let _: () = msg_send![panel, addSubview: title_lbl];
    let _: () = msg_send![panel, addSubview: sub_lbl];
    let _: () = msg_send![panel, addSubview: switch];

    panel
}

unsafe fn make_banner(target: id, frame: NSRect) -> id {
    let banner: id = msg_send![class!(NSView), new];
    let _: () = msg_send![banner, setFrame: frame];
    let _: () = msg_send![banner, setWantsLayer: YES];

    let layer: id = msg_send![banner, layer];
    // Amarillo aviso bien saturado para alto contraste con texto negro
    let bg = rgb(1.0, 0.78, 0.20);
    let bg_cg: id = msg_send![bg, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_cg];
    let _: () = msg_send![layer, setCornerRadius: 10.0_f64];
    let _: () = msg_send![layer, setBorderWidth: 1.0_f64];
    let border = rgb(0.85, 0.55, 0.0);
    let border_cg: id = msg_send![border, CGColor];
    let _: () = msg_send![layer, setBorderColor: border_cg];

    let dark_text = rgb(0.10, 0.08, 0.0);

    // Titulo del banner
    let title = make_label(
        "Falta permiso de Accesibilidad",
        NSRect::new(NSPoint::new(16.0, frame.size.height - 30.0), NSSize::new(frame.size.width - 32.0, 20.0)),
        13.0,
        true,
        false,
    );
    set_text_color(title, dark_text);

    // Subtitulo
    let sub = make_label(
        "Sin este permiso macuse no puede invertir el scroll.",
        NSRect::new(NSPoint::new(16.0, frame.size.height - 50.0), NSSize::new(frame.size.width - 32.0, 18.0)),
        12.0,
        false,
        false,
    );
    set_text_color(sub, dark_text);

    let _: () = msg_send![banner, addSubview: title];
    let _: () = msg_send![banner, addSubview: sub];

    // Botones a la derecha, abajo del banner
    let btn_h = 24.0;
    let btn_y = 8.0;

    let recheck_btn: id = msg_send![class!(NSButton),
        buttonWithTitle: ns("Reintentar")
        target: target
        action: sel!(onAction:)];
    let _: () = msg_send![recheck_btn, setTag: TAG_RECHECK];
    let recheck_w = 100.0;
    let _: () = msg_send![recheck_btn, setFrame:
        NSRect::new(NSPoint::new(frame.size.width - recheck_w - 12.0, btn_y),
                    NSSize::new(recheck_w, btn_h))];
    let _: () = msg_send![banner, addSubview: recheck_btn];

    let open_btn: id = msg_send![class!(NSButton),
        buttonWithTitle: ns("Abrir Ajustes")
        target: target
        action: sel!(onAction:)];
    let _: () = msg_send![open_btn, setTag: TAG_OPEN_ACCESS];
    let open_w = 120.0;
    let _: () = msg_send![open_btn, setFrame:
        NSRect::new(NSPoint::new(frame.size.width - recheck_w - 12.0 - open_w - 8.0, btn_y),
                    NSSize::new(open_w, btn_h))];
    let _: () = msg_send![banner, addSubview: open_btn];

    banner
}

/// Cuando el banner pasa de visible a oculto, las secciones suben para ocupar
/// el hueco. Recolocamos sus frames.
fn relayout_after_banner_hidden() {
    G.with(|g| {
        let g = g.borrow();
        let g = match g.as_ref() {
            Some(x) => x,
            None => return,
        };
        unsafe {
            let header_y = WIN_H - MARGIN - 28.0;
            let _: () = msg_send![g.header, setFrame:
                NSRect::new(NSPoint::new(MARGIN, header_y),
                            NSSize::new(WIN_W - MARGIN * 2.0, 24.0))];

            let trackpad_y = header_y - SECTION_H - 16.0;
            let _: () = msg_send![g.trackpad_section, setFrame:
                NSRect::new(NSPoint::new(MARGIN, trackpad_y),
                            NSSize::new(WIN_W - MARGIN * 2.0, SECTION_H))];

            let mouse_y = trackpad_y - SECTION_H - 12.0;
            let _: () = msg_send![g.mouse_section, setFrame:
                NSRect::new(NSPoint::new(MARGIN, mouse_y),
                            NSSize::new(WIN_W - MARGIN * 2.0, SECTION_H))];
        }
    });
}

pub fn build(state: Arc<ScrollState>, started_tap: Option<CGEventTap<'static>>) -> id {
    unsafe {
        G.with(|g| {
            *g.borrow_mut() = Some(Globals {
                state: state.clone(),
                tap: started_tap,
                banner: nil,
                header: nil,
                trackpad_section: nil,
                mouse_section: nil,
            });
        });

        let cls = delegate_class();
        let delegate: id = msg_send![cls, new];

        let app = NSApp();
        app.setActivationPolicy_(
            NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        );
        let _: () = msg_send![app, setDelegate: delegate];

        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(WIN_W, WIN_H));
        let style = NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask;
        let window: id = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
            frame,
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            NO,
        );
        window.setTitle_(ns("macuse"));
        window.center();
        let _: () = msg_send![window, setReleasedWhenClosed: NO];

        let content: id = msg_send![window, contentView];

        let trusted = permissions::is_trusted();

        // BANNER
        let banner_y = WIN_H - MARGIN - BANNER_H;
        let banner_frame = NSRect::new(
            NSPoint::new(MARGIN, banner_y),
            NSSize::new(WIN_W - MARGIN * 2.0, BANNER_H),
        );
        let banner = make_banner(delegate, banner_frame);
        let _: () = msg_send![banner, setHidden: if trusted { YES } else { NO }];
        let _: () = msg_send![content, addSubview: banner];

        // HEADER (ajusta posicion segun banner visible o no)
        let header_y = if trusted {
            WIN_H - MARGIN - 28.0
        } else {
            banner_y - 28.0 - 12.0
        };
        let header = make_label(
            "Natural scrolling independiente",
            NSRect::new(
                NSPoint::new(MARGIN, header_y),
                NSSize::new(WIN_W - MARGIN * 2.0, 24.0),
            ),
            16.0,
            true,
            false,
        );
        let _: () = msg_send![content, addSubview: header];

        let cfg = config::load();

        // TRACKPAD
        let trackpad_y = header_y - SECTION_H - 16.0;
        let trackpad_section = make_section(
            delegate,
            "Trackpad",
            "Natural scrolling para el trackpad",
            TAG_TRACKPAD,
            cfg.trackpad_natural,
            NSRect::new(
                NSPoint::new(MARGIN, trackpad_y),
                NSSize::new(WIN_W - MARGIN * 2.0, SECTION_H),
            ),
        );
        let _: () = msg_send![content, addSubview: trackpad_section];

        // MOUSE
        let mouse_y = trackpad_y - SECTION_H - 12.0;
        let mouse_section = make_section(
            delegate,
            "Raton",
            "Natural scrolling para el raton",
            TAG_MOUSE,
            cfg.mouse_natural,
            NSRect::new(
                NSPoint::new(MARGIN, mouse_y),
                NSSize::new(WIN_W - MARGIN * 2.0, SECTION_H),
            ),
        );
        let _: () = msg_send![content, addSubview: mouse_section];

        // LOGIN
        let login_y = mouse_y - 36.0;
        let login_btn: id = msg_send![class!(NSButton),
            checkboxWithTitle: ns("Iniciar al arrancar el Mac")
            target: delegate
            action: sel!(onAction:)];
        let _: () = msg_send![login_btn, setTag: TAG_LOGIN];
        let _: () = msg_send![login_btn, setState:
            if cfg.login_at_start { STATE_ON } else { STATE_OFF }];
        let _: () = msg_send![login_btn, setFrame:
            NSRect::new(NSPoint::new(MARGIN, login_y),
                        NSSize::new(WIN_W - MARGIN * 2.0, 22.0))];
        let _: () = msg_send![content, addSubview: login_btn];

        // HINT
        let hint_h = login_y - MARGIN - 8.0;
        let hint = make_label(
            "macuse intercepta los eventos de scroll y los invierte segun el periferico (trackpad o raton). No modifica el ajuste global del Sistema.",
            NSRect::new(NSPoint::new(MARGIN, MARGIN), NSSize::new(WIN_W - MARGIN * 2.0, hint_h)),
            12.0,
            false,
            true,
        );
        let secondary: id = msg_send![class!(NSColor), secondaryLabelColor];
        set_text_color(hint, secondary);
        let _: () = msg_send![content, addSubview: hint];

        // Guardar refs para relayout
        G.with(|g| {
            if let Some(g) = g.borrow_mut().as_mut() {
                g.banner = banner;
                g.header = header;
                g.trackpad_section = trackpad_section;
                g.mouse_section = mouse_section;
            }
        });

        let _: () = msg_send![window, makeKeyAndOrderFront: nil];
        app.activateIgnoringOtherApps_(YES);

        delegate
    }
}
