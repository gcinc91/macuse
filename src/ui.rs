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
    cfg: config::Config,
    banner: id,
    header: id,
    trackpad_section: id,
    mouse_section: id,
    login_btn: id,
}

thread_local! {
    static G: RefCell<Option<Globals>> = const { RefCell::new(None) };
}

// Helpers FFI sobre AppKit. Marcar `safe` aqui es una promesa: los selectores
// y clases que usamos existen, somos invocados desde el main thread con un
// autoreleasepool activo (NSApp), y los argumentos tienen los tipos correctos.
// El `unsafe` queda contenido en el cuerpo de cada helper.

fn ns(s: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(s) }
}

fn rgb(r: f64, g: f64, b: f64) -> id {
    unsafe {
        msg_send![class!(NSColor),
            colorWithSRGBRed: r green: g blue: b alpha: 1.0_f64]
    }
}

/// Persiste el config actual y loguea si falla. Lo silencioso era frustante de
/// diagnosticar.
fn save_cfg(cfg: &config::Config) {
    if let Err(e) = config::save(cfg) {
        crate::mlog!("config::save fallo: {e:#}");
    }
}

extern "C" fn on_action(_this: &mut Object, _: Sel, sender: id) {
    // Cocoa llama esta funcion via objc_msgSend; un panic cruzando esa frontera
    // es UB. Atrapamos cualquier panic y solo logueamos.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let tag: i64 = msg_send![sender, tag];
        match tag {
            TAG_TRACKPAD => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                G.with(|g| {
                    let mut g = g.borrow_mut();
                    if let Some(g) = g.as_mut() {
                        g.state.trackpad_natural.store(on_b, Ordering::Relaxed);
                        g.cfg.trackpad_natural = on_b;
                        save_cfg(&g.cfg);
                    }
                });
            }
            TAG_MOUSE => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                G.with(|g| {
                    let mut g = g.borrow_mut();
                    if let Some(g) = g.as_mut() {
                        g.state.mouse_natural.store(on_b, Ordering::Relaxed);
                        g.cfg.mouse_natural = on_b;
                        save_cfg(&g.cfg);
                    }
                });
            }
            TAG_LOGIN => {
                let on: i64 = msg_send![sender, state];
                let on_b = on == STATE_ON;
                let install_result = if on_b {
                    login_item::install()
                } else {
                    login_item::uninstall()
                };
                match install_result {
                    Ok(()) => {
                        G.with(|g| {
                            let mut g = g.borrow_mut();
                            if let Some(g) = g.as_mut() {
                                g.cfg.login_at_start = on_b;
                                save_cfg(&g.cfg);
                            }
                        });
                    }
                    Err(e) => {
                        crate::mlog!("login_item {} fallo: {e:#}",
                            if on_b { "install" } else { "uninstall" });
                        // Revertir el switch para reflejar el fallo.
                        G.with(|g| {
                            let g = g.borrow();
                            if let Some(g) = g.as_ref() {
                                let prev = g.cfg.login_at_start;
                                let prev_state = if prev { STATE_ON } else { STATE_OFF };
                                let _: () = msg_send![g.login_btn, setState: prev_state];
                            }
                        });
                    }
                }
            }
            TAG_OPEN_ACCESS => permissions::open_accessibility_pane(),
            TAG_RECHECK => {
                if permissions::is_trusted() {
                    let needs_relayout = G.with(|g| {
                        let mut g = g.borrow_mut();
                        if let Some(g) = g.as_mut() {
                            if g.tap.is_none() {
                                match tap::start(g.state.clone()) {
                                    Ok(t) => g.tap = Some(t),
                                    Err(e) => crate::mlog!("tap::start fallo en recheck: {e:#}"),
                                }
                            }
                            if g.tap.is_some() && g.banner != nil {
                                set_hidden(g.banner, true);
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
    }));
    if result.is_err() {
        crate::mlog!("on_action: panic capturado en callback FFI");
    }
}

extern "C" fn application_should_terminate_after_last_window_closed(
    _: &mut Object,
    _: Sel,
    _: id,
) -> bool {
    true
}

/// Cuando la app vuelve a foreground, releemos el ajuste global de natural
/// scrolling: el usuario podria haberlo cambiado en Ajustes del Sistema
/// mientras nuestra ventana estaba en background.
extern "C" fn application_did_become_active(_: &mut Object, _: Sel, _: id) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        G.with(|g| {
            if let Some(g) = g.borrow().as_ref() {
                g.state.refresh_system();
            }
        });
    }));
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
            decl.add_method(
                sel!(applicationDidBecomeActive:),
                application_did_become_active as extern "C" fn(&mut Object, Sel, id),
            );
        }
        decl.register()
    })
}

fn make_label(text: &str, frame: NSRect, font_size: f64, bold: bool, multiline: bool) -> id {
    unsafe {
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
}

fn set_text_color(label: id, color: id) {
    unsafe {
        let _: () = msg_send![label, setTextColor: color];
    }
}

fn add_subview(parent: id, child: id) {
    unsafe {
        let _: () = msg_send![parent, addSubview: child];
    }
}

fn set_hidden(view: id, hidden: bool) {
    unsafe {
        let _: () = msg_send![view, setHidden: if hidden { YES } else { NO }];
    }
}

fn set_frame(view: id, frame: NSRect) {
    unsafe {
        let _: () = msg_send![view, setFrame: frame];
    }
}

fn set_tag(obj: id, tag: i64) {
    unsafe {
        let _: () = msg_send![obj, setTag: tag];
    }
}

fn make_switch(target: id, tag: i64, on: bool, frame: NSRect) -> id {
    unsafe {
        let sw: id = msg_send![class!(NSSwitch), new];
        let _: () = msg_send![sw, setState: if on { STATE_ON } else { STATE_OFF }];
        let _: () = msg_send![sw, setTarget: target];
        let _: () = msg_send![sw, setAction: sel!(onAction:)];
        let _: () = msg_send![sw, setTag: tag];
        let _: () = msg_send![sw, setFrame: frame];
        sw
    }
}

/// Seccion: panel con borde redondo, titulo en negrita, subtitulo gris debajo,
/// switch a la derecha centrado verticalmente.
fn make_section(
    target: id,
    title: &str,
    sub: &str,
    tag: i64,
    on: bool,
    frame: NSRect,
) -> id {
    let pw = frame.size.width;
    let ph = frame.size.height;

    // Calculos de layout en codigo safe.
    let sw_w = 40.0;
    let sw_h = 22.0;
    let sw_frame = NSRect::new(
        NSPoint::new(pw - sw_w - 16.0, (ph - sw_h) / 2.0),
        NSSize::new(sw_w, sw_h),
    );
    let title_frame = NSRect::new(
        NSPoint::new(16.0, ph / 2.0 + 2.0),
        NSSize::new(pw - sw_w - 48.0, 22.0),
    );
    let sub_frame = NSRect::new(
        NSPoint::new(16.0, ph / 2.0 - 22.0),
        NSSize::new(pw - sw_w - 48.0, 18.0),
    );

    let panel: id = unsafe {
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
        panel
    };

    let switch = make_switch(target, tag, on, sw_frame);
    let title_lbl = make_label(title, title_frame, 14.0, true, false);
    let sub_lbl = make_label(sub, sub_frame, 12.0, false, false);
    let secondary: id = unsafe { msg_send![class!(NSColor), secondaryLabelColor] };
    set_text_color(sub_lbl, secondary);

    add_subview(panel, title_lbl);
    add_subview(panel, sub_lbl);
    add_subview(panel, switch);

    panel
}

fn make_banner(target: id, frame: NSRect) -> id {
    // Layout en codigo safe.
    let title_frame = NSRect::new(
        NSPoint::new(16.0, frame.size.height - 30.0),
        NSSize::new(frame.size.width - 32.0, 20.0),
    );
    let sub_frame = NSRect::new(
        NSPoint::new(16.0, frame.size.height - 50.0),
        NSSize::new(frame.size.width - 32.0, 18.0),
    );
    let btn_h = 24.0;
    let btn_y = 8.0;
    let recheck_w = 100.0;
    let open_w = 120.0;
    let recheck_frame = NSRect::new(
        NSPoint::new(frame.size.width - recheck_w - 12.0, btn_y),
        NSSize::new(recheck_w, btn_h),
    );
    let open_frame = NSRect::new(
        NSPoint::new(frame.size.width - recheck_w - 12.0 - open_w - 8.0, btn_y),
        NSSize::new(open_w, btn_h),
    );

    // Setup del fondo y bordes del banner (FFI).
    let banner: id = unsafe {
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
        banner
    };

    let dark_text = rgb(0.10, 0.08, 0.0);

    let title = make_label("Falta permiso de Accesibilidad", title_frame, 13.0, true, false);
    set_text_color(title, dark_text);

    let sub = make_label(
        "Sin este permiso macuse no puede invertir el scroll.",
        sub_frame,
        12.0,
        false,
        false,
    );
    set_text_color(sub, dark_text);

    add_subview(banner, title);
    add_subview(banner, sub);

    let recheck_btn: id = unsafe {
        msg_send![class!(NSButton),
            buttonWithTitle: ns("Reintentar")
            target: target
            action: sel!(onAction:)]
    };
    set_tag(recheck_btn, TAG_RECHECK);
    set_frame(recheck_btn, recheck_frame);
    add_subview(banner, recheck_btn);

    let open_btn: id = unsafe {
        msg_send![class!(NSButton),
            buttonWithTitle: ns("Abrir Ajustes")
            target: target
            action: sel!(onAction:)]
    };
    set_tag(open_btn, TAG_OPEN_ACCESS);
    set_frame(open_btn, open_frame);
    add_subview(banner, open_btn);

    banner
}

/// Cuando el banner pasa de visible a oculto, las secciones suben para ocupar
/// el hueco. Recolocamos sus frames.
fn relayout_after_banner_hidden() {
    let header_y = WIN_H - MARGIN - 28.0;
    let trackpad_y = header_y - SECTION_H - 16.0;
    let mouse_y = trackpad_y - SECTION_H - 12.0;
    let inner_w = WIN_W - MARGIN * 2.0;

    let header_frame = NSRect::new(NSPoint::new(MARGIN, header_y), NSSize::new(inner_w, 24.0));
    let trackpad_frame =
        NSRect::new(NSPoint::new(MARGIN, trackpad_y), NSSize::new(inner_w, SECTION_H));
    let mouse_frame =
        NSRect::new(NSPoint::new(MARGIN, mouse_y), NSSize::new(inner_w, SECTION_H));

    G.with(|g| {
        if let Some(g) = g.borrow().as_ref() {
            set_frame(g.header, header_frame);
            set_frame(g.trackpad_section, trackpad_frame);
            set_frame(g.mouse_section, mouse_frame);
        }
    });
}

pub fn build(
    state: Arc<ScrollState>,
    started_tap: Option<CGEventTap<'static>>,
    cfg: config::Config,
) -> id {
    G.with(|g| {
        *g.borrow_mut() = Some(Globals {
            state: state.clone(),
            tap: started_tap,
            cfg: cfg.clone(),
            banner: nil,
            header: nil,
            trackpad_section: nil,
            mouse_section: nil,
            login_btn: nil,
        });
    });

    // Layout calculations en codigo safe.
    let inner_w = WIN_W - MARGIN * 2.0;
    let win_frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(WIN_W, WIN_H));
    let banner_y = WIN_H - MARGIN - BANNER_H;
    let banner_frame = NSRect::new(NSPoint::new(MARGIN, banner_y), NSSize::new(inner_w, BANNER_H));

    let trusted = permissions::is_trusted();

    let header_y = if trusted {
        WIN_H - MARGIN - 28.0
    } else {
        banner_y - 28.0 - 12.0
    };
    let header_frame = NSRect::new(NSPoint::new(MARGIN, header_y), NSSize::new(inner_w, 24.0));

    let trackpad_y = header_y - SECTION_H - 16.0;
    let trackpad_frame =
        NSRect::new(NSPoint::new(MARGIN, trackpad_y), NSSize::new(inner_w, SECTION_H));

    let mouse_y = trackpad_y - SECTION_H - 12.0;
    let mouse_frame =
        NSRect::new(NSPoint::new(MARGIN, mouse_y), NSSize::new(inner_w, SECTION_H));

    let login_y = mouse_y - 36.0;
    let login_frame = NSRect::new(NSPoint::new(MARGIN, login_y), NSSize::new(inner_w, 22.0));

    let hint_h = login_y - MARGIN - 8.0;
    let hint_frame = NSRect::new(NSPoint::new(MARGIN, MARGIN), NSSize::new(inner_w, hint_h));

    let style = NSWindowStyleMask::NSTitledWindowMask
        | NSWindowStyleMask::NSClosableWindowMask
        | NSWindowStyleMask::NSMiniaturizableWindowMask;

    // Setup de delegate, app y ventana (FFI, scoped).
    let cls = delegate_class();
    let (delegate, app, window, content) = unsafe {
        let delegate: id = msg_send![cls, new];
        let app = NSApp();
        app.setActivationPolicy_(
            NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
        );
        let _: () = msg_send![app, setDelegate: delegate];

        let window: id = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
            win_frame,
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            NO,
        );
        window.setTitle_(ns("macuse"));
        window.center();
        let _: () = msg_send![window, setReleasedWhenClosed: NO];
        let content: id = msg_send![window, contentView];
        (delegate, app, window, content)
    };

    // BANNER
    let banner = make_banner(delegate, banner_frame);
    set_hidden(banner, trusted);
    add_subview(content, banner);

    // HEADER
    let header = make_label("Natural scrolling independiente", header_frame, 16.0, true, false);
    add_subview(content, header);

    // TRACKPAD
    let trackpad_section = make_section(
        delegate,
        "Trackpad",
        "Natural scrolling para el trackpad",
        TAG_TRACKPAD,
        cfg.trackpad_natural,
        trackpad_frame,
    );
    add_subview(content, trackpad_section);

    // MOUSE
    let mouse_section = make_section(
        delegate,
        "Raton",
        "Natural scrolling para el raton",
        TAG_MOUSE,
        cfg.mouse_natural,
        mouse_frame,
    );
    add_subview(content, mouse_section);

    // LOGIN
    let login_btn: id = unsafe {
        let btn: id = msg_send![class!(NSButton),
            checkboxWithTitle: ns("Iniciar al arrancar el Mac")
            target: delegate
            action: sel!(onAction:)];
        let login_state = if cfg.login_at_start { STATE_ON } else { STATE_OFF };
        let _: () = msg_send![btn, setState: login_state];
        btn
    };
    set_tag(login_btn, TAG_LOGIN);
    set_frame(login_btn, login_frame);
    add_subview(content, login_btn);

    // HINT
    let hint = make_label(
        "macuse intercepta los eventos de scroll y los invierte segun el periferico (trackpad o raton). No modifica el ajuste global del Sistema.",
        hint_frame,
        12.0,
        false,
        true,
    );
    let secondary: id = unsafe { msg_send![class!(NSColor), secondaryLabelColor] };
    set_text_color(hint, secondary);
    add_subview(content, hint);

    // Guardar refs para relayout
    G.with(|g| {
        if let Some(g) = g.borrow_mut().as_mut() {
            g.banner = banner;
            g.header = header;
            g.trackpad_section = trackpad_section;
            g.mouse_section = mouse_section;
            g.login_btn = login_btn;
        }
    });

    unsafe {
        let _: () = msg_send![window, makeKeyAndOrderFront: nil];
        app.activateIgnoringOtherApps_(YES);
    }

    delegate
}
