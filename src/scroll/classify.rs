use core_graphics::event::{CGEvent, EventField};

/// Trackpads producen scroll continuo (pixel-precise + phase). Mouse wheels no.
/// Heuristica estandar usada por Scroll Reverser, Mos, LinearMouse.
pub fn is_trackpad(event: &CGEvent) -> bool {
    event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_IS_CONTINUOUS) != 0
}
