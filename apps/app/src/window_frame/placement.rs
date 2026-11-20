use tauri::{
    LogicalSize, LogicalUnit, PhysicalPosition, PhysicalRect, PhysicalSize,
};

pub(super) type WindowRect = PhysicalRect<i32, u32>;

pub(super) fn title_bar_is_reachable(
    window: WindowRect,
    work_area: WindowRect,
    scale_factor: f64,
) -> bool {
    let caption_buttons_width =
        LogicalUnit(144.0).to_physical::<u32>(scale_factor).0;
    let title_height = LogicalUnit(36.0).to_physical::<u32>(scale_factor).0;
    let min_visible_drag_area =
        LogicalSize::new(120.0, 16.0).to_physical::<u32>(scale_factor);
    let drag_width = window.size.width.saturating_sub(caption_buttons_width);
    let drag_height = window.size.height.min(title_height);
    drag_width > 0
        && drag_height > 0
        && overlap(
            window.position.x,
            drag_width,
            work_area.position.x,
            work_area.size.width,
        ) >= i64::from(drag_width.min(min_visible_drag_area.width))
        && overlap(
            window.position.y,
            drag_height,
            work_area.position.y,
            work_area.size.height,
        ) >= i64::from(drag_height.min(min_visible_drag_area.height))
}

fn overlap(
    window_start: i32,
    window_length: u32,
    work_start: i32,
    work_length: u32,
) -> i64 {
    let window_start = i64::from(window_start);
    let work_start = i64::from(work_start);
    (window_start + i64::from(window_length))
        .min(work_start + i64::from(work_length))
        - window_start.max(work_start)
}

pub(super) fn center_in_work_area(
    size: PhysicalSize<u32>,
    work_area: WindowRect,
) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        work_area.position.x.saturating_add(
            (work_area.size.width.saturating_sub(size.width) / 2) as i32,
        ),
        work_area.position.y.saturating_add(
            (work_area.size.height.saturating_sub(size.height) / 2) as i32,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, width: u32, height: u32) -> WindowRect {
        WindowRect {
            position: PhysicalPosition::new(left, top),
            size: PhysicalSize::new(width, height),
        }
    }

    #[test]
    fn unplugging_a_negative_origin_monitor_leaves_no_reachable_title_bar() {
        let main = rect(0, 0, 1920, 1040);
        let secondary = rect(-1920, 0, 1920, 1080);
        let window = rect(-1680, 100, 1280, 800);
        assert!(title_bar_is_reachable(window, secondary, 1.0));
        assert!(!title_bar_is_reachable(window, main, 1.0));
        assert_eq!(
            center_in_work_area(window.size, main),
            PhysicalPosition::new(320, 120)
        );
    }

    #[test]
    fn taskbar_and_small_corner_overlap_do_not_make_the_title_reachable() {
        let work_area = rect(0, 40, 1920, 1000);
        for window in [
            rect(0, 0, 1280, 800),
            rect(1880, 100, 1280, 800),
            rect(100, 1032, 1280, 800),
        ] {
            assert!(!title_bar_is_reachable(window, work_area, 1.0));
        }
        assert!(title_bar_is_reachable(
            rect(100, 40, 1280, 800),
            work_area,
            1.0,
        ));
    }

    #[test]
    fn visible_caption_buttons_without_draggable_space_require_repositioning() {
        let work_area = rect(0, 0, 1920, 1040);
        let buttons_only = rect(-1160, 100, 1280, 800);
        let with_drag_space = rect(-960, 100, 1280, 800);
        assert!(!title_bar_is_reachable(buttons_only, work_area, 1.0));
        assert!(title_bar_is_reachable(with_drag_space, work_area, 1.0));
    }

    #[test]
    fn reachability_uses_logical_pixels_at_the_window_scale() {
        let work_area = rect(0, 0, 1920, 1040);
        let window = rect(1786, 0, 1280, 800);
        assert!(title_bar_is_reachable(window, work_area, 1.0));
        assert!(!title_bar_is_reachable(window, work_area, 1.25));
    }

    #[test]
    fn oversized_windows_with_reachable_titles_need_no_repositioning() {
        let portrait = rect(80, 44, 1000, 650);
        let window = rect(80, 44, 1300, 900);
        assert!(title_bar_is_reachable(window, portrait, 1.0));
        assert_eq!(
            center_in_work_area(PhysicalSize::new(1100, 700), portrait),
            portrait.position,
        );
    }
}
