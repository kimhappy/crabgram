//! Maps a screen coordinate to the part of the preview it lands on.

use crabgram_styling::{Button, Slot, host, ui::Model};

use super::{
    look::{self, Area, MenuAction, MenuItem, ScreenPoint},
    renderer::origin,
    state::Target,
};

/// Whether `point` lands on the close button, which is also the about page's back button.
#[must_use]
pub fn on_close(point: ScreenPoint) -> bool {
    Area::at(host::position::CLOSE, Button::Close.frame()).contains(point)
}

/// The page whose pagination dot a touch at `point` lands on: a column of the styled `dots`, else
/// a stock dot.
fn dot(pages: u32, page: u32, dots: Option<Area>, point: ScreenPoint) -> Option<u32> {
    let ScreenPoint { x, y } = point;

    dots.map_or_else(
        || {
            look::dot_centers(pages, page)
                .position(|(middle, height)| {
                    (middle - x).abs() < look::DOT_REACH && (height - y).abs() < 10.0
                })
                .map(|dot| dot as u32)
        },
        |area| {
            area.contains(point)
                .then(|| ((x - area.x()) / area.width() * pages as f32) as u32)
        },
    )
}

/// What a touch at `point` on `page` lands on, or `None` for empty screen. The close, remove and
/// swap buttons do nothing in a standalone UI, so they land on nothing.
#[must_use]
pub fn hit(
    slots: &[Slot],
    pages: u32,
    page: u32,
    dots: Option<Area>,
    point: ScreenPoint,
) -> Option<Target> {
    if Area::at(host::position::BYPASS, host::BYPASS).contains(point) {
        return Some(Target::Bypass);
    }

    if Area::at(host::position::SCENE_CONTROL, host::SCENE_CONTROL).contains(point) {
        return Some(Target::Scenes);
    }

    if Area::at(host::position::BUTTONS[2], Button::More.frame()).contains(point) {
        return Some(Target::More);
    }

    if pages > 1
        && let Some(dot) = dot(pages, page, dots, point)
    {
        return Some(Target::Dot(dot));
    }

    slots
        .iter()
        .filter(|slot| slot.page() == page)
        .find_map(|slot| {
            let (x, y) = origin(slot);
            let (width, height) = (host::span_width(slot.slots()), host::WIDGET.1);

            Area::new(x, y, width as f32, height as f32)
                .contains(point)
                .then_some(Target::Widget(slot.position()))
        })
}

/// The "⋮" menu item a touch at `point` lands on.
#[must_use]
pub fn hit_menu(model: &Model, point: ScreenPoint) -> Option<MenuAction> {
    look::menu_items(model.settings().about().is_some())
        .find(|item| item.area().contains(point))
        .map(MenuItem::action)
}

#[cfg(test)]
mod tests {
    use super::{Area, ScreenPoint, dot};

    #[test]
    fn a_styled_row_of_dots_takes_one_column_per_page() {
        let dots = Some(Area::new(100.0, 250.0, 60.0, 20.0));
        let dot_at = |x, y| dot(3, 0, dots, ScreenPoint::new(x, y));

        assert_eq!(dot_at(105.0, 260.0), Some(0));
        assert_eq!(dot_at(135.0, 260.0), Some(1));
        assert_eq!(dot_at(159.0, 260.0), Some(2));
        assert_eq!(dot_at(161.0, 260.0), None);
        assert_eq!(dot_at(135.0, 240.0), None);
    }
}
