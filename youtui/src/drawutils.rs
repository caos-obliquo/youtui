use ratatui::prelude::Rect;
use ratatui::style::Color;

// Standard app colour scheme
pub const SELECTED_BORDER_COLOUR: Color = Color::Cyan;
pub const DESELECTED_BORDER_COLOUR: Color = Color::Reset;
// TODO: Implement in all locations.
pub const TEXT_COLOUR: Color = Color::Reset;
pub const BUTTON_BG_COLOUR: Color = Color::Gray;
pub const BUTTON_FG_COLOUR: Color = Color::Black;
pub const PROGRESS_BG_COLOUR: Color = Color::DarkGray;
pub const PROGRESS_FG_COLOUR: Color = Color::LightGreen;
pub const TABLE_HEADINGS_COLOUR: Color = Color::LightGreen;
pub const ROW_HIGHLIGHT_COLOUR: Color = Color::Blue;
pub const VISUAL_MODE_COLOUR: Color = Color::Cyan;
pub const PLAYING_COLOUR: Color = Color::Green;

/// Helper function to create a popup at bottom corner of chunk.
pub fn left_bottom_corner_rect(height: u16, width: u16, r: Rect) -> Rect {
    let r_x2 = r.x + r.width;
    let r_y2 = r.y + r.height;
    let x = r_x2.saturating_sub(width).max(r.x);
    let y = r_y2.saturating_sub(height).max(r.y);
    Rect {
        x,
        y,
        width: width.min(r_x2 - x),
        height: height.min(r_y2 - y),
    }
}
/// Helper function to create a popup below a chunk.
//  We pass in the max bounds that can be rendered by the application,
//  to avoid returning a Rect that is not drawable.
// TODO: Add a test to ensure this is returning correct area
pub fn below_left_rect(height: u16, width: u16, r: Rect, max_bounds: Rect) -> Rect {
    let x = r.x.max(max_bounds.x);
    let y = (r.y + r.height - 1).max(max_bounds.y);
    Rect {
        x,
        y,
        width: width.min(max_bounds.right().saturating_sub(x)),
        height: height.min(max_bounds.bottom().saturating_sub(y)),
    }
}
/// Helper function to create a popup in the center of a chunk.
pub fn centered_rect(height: u16, width: u16, r: Rect) -> Rect {
    Rect {
        x: (r.x + r.width / 2).saturating_sub(width / 2).max(r.x),
        y: (r.y + r.height / 2).saturating_sub(height / 2).max(r.y),
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
/// Helper function to get the bottom line of a chunk, ignoring side borders.
pub fn bottom_of_rect(r: Rect) -> Rect {
    Rect {
        x: r.x.saturating_add(1),
        y: r.y.saturating_add(r.height).saturating_sub(1),
        width: r.width.saturating_sub(2),
        height: 1,
    }
}
/// Helper function to get `offset` of a list widget like `List` or `Table`
/// after changing the size of the list.
pub fn middle_of_rect(r: Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y + (r.height - 1) / 2,
        width: r.width,
        height: 1,
    }
}
pub fn get_offset_after_list_resize(
    prev_offset: usize,
    prev_cur: usize,
    prev_max_cur: usize,
    new_cur: usize,
    new_max_cur: usize,
) -> usize {
    // Calculate previous offset relative to the previous cur (as a signed int),
    // defaulting to zero if any issues with cast identified.
    let prev_offset_rel_cur = isize::try_from(prev_offset)
        .map(|prev_offset| prev_offset.saturating_sub_unsigned(prev_cur))
        .unwrap_or(0);
    // Calculate previous offset relative to the previous max cur (as a signed int),
    // defaulting to zero if any issues with cast required.
    let prev_offset_rel_max = isize::try_from(prev_offset)
        .map(|prev_offset| prev_offset.saturating_sub_unsigned(prev_max_cur))
        .unwrap_or(0);
    // Adjust offset accordingly to ensure the offset relative to cur is the same as
    // it was previously.
    let Ok(new_cur_isize) = isize::try_from(new_cur) else {
        return 0;
    };
    let Ok(new_max_cur_isize) = isize::try_from(new_max_cur) else {
        return 0;
    };
    let new_offset_using_rel_cur = new_cur_isize + prev_offset_rel_cur;
    let new_offset_using_rel_max = new_max_cur_isize + prev_offset_rel_max;
    let new_offset: usize = ((new_offset_using_rel_max + new_offset_using_rel_cur) / 2)
        .try_into()
        .unwrap_or(0);
    new_offset
}

#[cfg(test)]
mod tests {
    use super::{below_left_rect, bottom_of_rect, centered_rect, left_bottom_corner_rect};
    use crate::drawutils::{get_offset_after_list_resize, middle_of_rect};
    use ratatui::layout::Rect;

    #[test]
    fn test_get_offset_after_list_resize_prev_upper_list() {
        let new_offset = get_offset_after_list_resize(30, 40, 50, 10, 10);
        assert_eq!(new_offset, 0);
    }
    #[test]
    fn test_get_offset_after_list_resize_prev_lower_list() {
        let new_offset = get_offset_after_list_resize(20, 40, 40, 10, 10);
        assert_eq!(new_offset, 0);
    }
    #[test]
    fn test_get_offset_after_list_resize_prev_no_change() {
        let prev_offset = 30;
        let new_offset = get_offset_after_list_resize(prev_offset, 40, 50, 40, 50);
        assert_eq!(prev_offset, new_offset);
    }
    fn bounds_check_rect(r: Rect, max_bounds: Rect) {
        assert!(r.left() >= max_bounds.left());
        assert!(r.right() <= max_bounds.right());
        assert!(r.bottom() <= max_bounds.bottom());
        assert!(r.top() >= max_bounds.top());
    }
    #[test]
    #[should_panic]
    fn test_bounds_check_rect() {
        bounds_check_rect(
            Rect::new(0, 0, 50, 50),
            Rect::new(0, 50, 50, 50),
        );
        bounds_check_rect(
            Rect::new(30, 30, 50, 50),
            Rect::new(30, 30, 51, 51),
        );
        bounds_check_rect(
            Rect::new(30, 30, 50, 50),
            Rect::new(30, 30, 51, 50),
        );
        bounds_check_rect(
            Rect::new(30, 30, 50, 50),
            Rect::new(30, 30, 50, 51),
        );
        bounds_check_rect(
            Rect::new(30, 30, 50, 50),
            Rect::new(31, 31, 50, 50),
        );
    }
    // These don't actually do anything as they don't try to draw...
    #[test]
    fn bounds_check_left_bottom_corner_rect() {
        left_bottom_corner_rect(
            u16::MAX,
            u16::MAX,
            Rect {
                x: 0,
                y: 0,
                height: 50,
                width: 50,
            },
        );
        left_bottom_corner_rect(
            u16::MAX,
            u16::MAX,
            Rect {
                x: 0,
                y: 50,
                height: 50,
                width: 50,
            },
        );
        left_bottom_corner_rect(
            u16::MAX,
            u16::MAX,
            Rect {
                x: 50,
                y: 0,
                height: 50,
                width: 50,
            },
        );
        left_bottom_corner_rect(
            u16::MAX,
            u16::MAX,
            Rect {
                x: 50,
                y: 50,
                height: 50,
                width: 50,
            },
        );
    }

    #[test]
    fn bounds_check_centered_rect() {
        let t_r1 = Rect::new(0, 0, 50, 50);
        let t_r2 = Rect::new(0, 50, 50, 50);
        let t_r3 = Rect::new(50, 0, 50, 50);
        let t_r4 = Rect::new(50, 50, 50, 50);
        let r1 = centered_rect(u16::MAX, u16::MAX, t_r1);
        let r2 = centered_rect(u16::MAX, u16::MAX, t_r2);
        let r3 = centered_rect(u16::MAX, u16::MAX, t_r3);
        let r4 = centered_rect(u16::MAX, u16::MAX, t_r4);
        bounds_check_rect(r1, t_r1);
        bounds_check_rect(r2, t_r2);
        bounds_check_rect(r3, t_r3);
        bounds_check_rect(r4, t_r4);
    }
    #[test]
    fn test_bottom_of_rect_basic() {
        let r = Rect::new(5, 10, 20, 30);
        let result = bottom_of_rect(r);
        assert_eq!(result.x, 6);
        assert_eq!(result.y, 39);
        assert_eq!(result.width, 18);
        assert_eq!(result.height, 1);
    }
    #[test]
    fn test_bottom_of_rect_narrow() {
        let r = Rect::new(5, 10, 1, 30);
        let result = bottom_of_rect(r);
        assert_eq!(result.x, 6);
        assert_eq!(result.y, 39);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 1);
    }
    #[test]
    fn test_bottom_of_rect_zero_width() {
        let r = Rect::new(5, 10, 0, 30);
        let result = bottom_of_rect(r);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 1);
    }
    #[test]
    fn test_middle_of_rect() {
        let r1 = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 3,
        };
        assert_eq!(
            middle_of_rect(r1),
            Rect {
                x: 0,
                y: 1,
                width: 10,
                height: 1
            }
        );
        let r2 = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        assert_eq!(
            middle_of_rect(r2),
            Rect {
                x: 0,
                y: 4,
                width: 10,
                height: 1
            }
        );
        let r3 = Rect {
            x: 0,
            y: 10,
            width: 10,
            height: 5,
        };
        assert_eq!(
            middle_of_rect(r3),
            Rect {
                x: 0,
                y: 12,
                width: 10,
                height: 1
            }
        );
    }
    #[test]
    fn bounds_check_below_left_rect() {
        let cases = [
            (
                Rect::new(0, 0, 50, 50),
                Rect::new(100, 100, 1050, 1050),
            ),
            (
                Rect::new(0, 50, 50, 50),
                Rect::new(100, 1050, 1050, 1050),
            ),
            (
                Rect::new(50, 0, 50, 50),
                Rect::new(1050, 100, 1050, 1050),
            ),
            (
                Rect::new(50, 50, 50, 50),
                Rect::new(1050, 1050, 1050, 1050),
            ),
        ];
        for (r, max) in &cases {
            let result = below_left_rect(u16::MAX, u16::MAX, *r, *max);
            bounds_check_rect(result, *max);
            assert!(result.x >= r.x);
            assert!(result.y >= r.y.saturating_add(r.height).saturating_sub(1));
        }
    }
}
