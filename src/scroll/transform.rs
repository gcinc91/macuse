use core_graphics::event::{CGEvent, CGEventField, EventField};

use super::ScrollState;

/// Decide si invertir y aplica la inversion in-place sobre el CGEvent.
///
/// Para trackpad (continuous): invierte SOLO los deltas pixel-preciso
/// (point y fixed). El line delta queda intacto - macOS lo computa
/// internamente y al ir despacio invertirlo causa oscilaciones porque
/// el buffer de smoothing del Session espera consistencia con el line
/// delta original.
///
/// Para mouse (line scroll): invierte SOLO el line delta. Los point/fixed
/// suelen ir a 0 en eventos de rueda discreta.
pub fn apply(state: &ScrollState, event: &CGEvent) {
    let is_trackpad = super::classify::is_trackpad(event);
    let (trackpad_natural, mouse_natural, system_natural) = state.snapshot();

    let user_wants_natural = if is_trackpad {
        trackpad_natural
    } else {
        mouse_natural
    };

    if user_wants_natural == system_natural {
        return;
    }

    if is_trackpad {
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2);
        invert_double(event, EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1);
        invert_double(event, EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_2);
    } else {
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1);
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2);
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
        invert_int(event, EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2);
        invert_double(event, EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_1);
        invert_double(event, EventField::SCROLL_WHEEL_EVENT_FIXED_POINT_DELTA_AXIS_2);
    }
}

fn invert_int(event: &CGEvent, field: CGEventField) {
    let v = event.get_integer_value_field(field);
    if v != 0 {
        event.set_integer_value_field(field, -v);
    }
}

fn invert_double(event: &CGEvent, field: CGEventField) {
    let v = event.get_double_value_field(field);
    if v != 0.0 {
        event.set_double_value_field(field, -v);
    }
}
