#![allow(missing_docs, reason = "the item names are the documentation")]

pub const SCREEN: (u32, u32) = (1424, 280);

pub const TOP_BAR: (u32, u32) = (1424, 60);

pub const SLOTS_PER_PAGE: u32 = 6;

pub const SLOT_PITCH: u32 = 237;

pub const FIRST_SLOT_X: u32 = 28;

pub const WIDGET_TOP: u32 = 98;

pub const WIDGET: (u32, u32) = (180, 145);

pub const CONTROL: u32 = 145;

pub const MAX_START_PADDING: u32 = 5;

pub const BLOCK_IMAGE: u32 = 200;

pub const BLOCK_NAME: (u32, u32) = (400, 50);

pub const SCENE_CONTROL: (u32, u32) = (331, 50);

pub const BYPASS: (u32, u32) = (107, 50);

pub const ABOUT: (u32, u32) = (SCREEN.0, SCREEN.1 - TOP_BAR.1);

pub mod position {
    pub const CLOSE: (i32, i32) = (5, 5);
    pub const BLOCK_NAME: (i32, i32) = (110, 5);
    pub const BYPASS: (i32, i32) = (659, 5);
    pub const SCENE_CONTROL: (i32, i32) = (856, 5);
    pub const BUTTONS: [(i32, i32); 3] = [(1229, 5), (1299, 5), (1369, 5)];
}

#[must_use]
pub const fn slot_x(column: u32) -> u32 {
    FIRST_SLOT_X + SLOT_PITCH * column
}

#[must_use]
pub const fn span_width(slots: u32) -> u32 {
    SLOT_PITCH * slots.saturating_sub(1) + WIDGET.0
}

#[cfg(test)]
mod tests {
    use super::{slot_x, span_width};

    #[test]
    fn slot_edges_and_spans_follow_the_pitch() {
        assert_eq!(slot_x(5), 1213);
        assert_eq!(span_width(3), 654);
    }
}
