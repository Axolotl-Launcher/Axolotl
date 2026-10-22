use serde::Serialize;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameResult {
    pub css_border: bool,
    pub warnings: Vec<String>,
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum FrameOperation {
    Shadow(bool),
    RoundedCorners(bool),
    Border(bool),
    Caption(bool),
    Text(bool),
}

/// Cosmetic compatibility failures are independent; only confirmed border suppression enables CSS.
#[cfg(any(windows, test))]
fn configure_frame(
    custom_frame: bool,
    maximized: bool,
    dwm_colors_supported: bool,
    mut apply: impl FnMut(FrameOperation) -> Result<(), String>,
) -> FrameResult {
    let mut result = FrameResult::default();
    let mut operation = |op| match apply(op) {
        Ok(()) => true,
        Err(error) => {
            result.warnings.push(format!("{op:?}: {error}"));
            false
        }
    };
    let shadow_applied = operation(FrameOperation::Shadow(!custom_frame));
    let mut colors_hidden = true;
    if dwm_colors_supported {
        operation(FrameOperation::RoundedCorners(custom_frame && !maximized));
        let border = operation(FrameOperation::Border(custom_frame));
        let caption = operation(FrameOperation::Caption(custom_frame));
        operation(FrameOperation::Text(custom_frame));
        colors_hidden = border && caption;
    }
    result.css_border =
        custom_frame && !maximized && shadow_applied && colors_hidden;
    result
}

pub async fn apply_frame(
    enabled: bool,
    window: &tauri::Window,
) -> Result<FrameResult, String> {
    let decorated = window.is_decorated().map_err(|error| error.to_string())?;
    let maximized = window.is_maximized().map_err(|error| error.to_string())?;
    let custom_frame = enabled && !decorated;

    #[cfg(windows)]
    {
        let shadow_result = window
            .set_shadow(!custom_frame)
            .map_err(|error| error.to_string());
        let (send, receive) = tokio::sync::oneshot::channel();
        let frame_window = window.clone();
        let colors_supported =
            windows_version::OsVersion::current().build >= 22000;
        window
            .run_on_main_thread(move || {
                let result = frame_window
                    .hwnd()
                    .map_err(|error| error.to_string())
                    .map(|hwnd| {
                        configure_frame(
                            custom_frame,
                            maximized,
                            colors_supported,
                            |operation| match operation {
                                FrameOperation::Shadow(_) => {
                                    shadow_result.clone()
                                }
                                other => apply_dwm_attribute(hwnd, other),
                            },
                        )
                    });
                let _ = send.send(result);
            })
            .map_err(|error| error.to_string())?;
        receive.await.map_err(|error| error.to_string())?
    }
    #[cfg(not(windows))]
    Ok(FrameResult {
        css_border: custom_frame && !maximized,
        warnings: vec![],
    })
}

#[cfg(windows)]
fn apply_dwm_attribute(
    hwnd: windows::Win32::Foundation::HWND,
    operation: FrameOperation,
) -> Result<(), String> {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_COLOR_DEFAULT,
        DWMWA_COLOR_NONE, DWMWA_TEXT_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_DEFAULT, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let color = |hidden| {
        if hidden {
            DWMWA_COLOR_NONE
        } else {
            DWMWA_COLOR_DEFAULT
        }
    };
    let (attribute, value) = match operation {
        FrameOperation::RoundedCorners(rounded) => (
            DWMWA_WINDOW_CORNER_PREFERENCE,
            if rounded {
                DWMWCP_ROUND.0 as u32
            } else {
                DWMWCP_DEFAULT.0 as u32
            },
        ),
        FrameOperation::Border(hidden) => (DWMWA_BORDER_COLOR, color(hidden)),
        FrameOperation::Caption(hidden) => (DWMWA_CAPTION_COLOR, color(hidden)),
        FrameOperation::Text(hidden) => (DWMWA_TEXT_COLOR, color(hidden)),
        FrameOperation::Shadow(_) => unreachable!(),
    };
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            std::ptr::from_ref(&value).cast(),
            std::mem::size_of_val(&value) as u32,
        )
    }
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_failure_does_not_skip_border_or_caption_suppression() {
        let mut applied = Vec::new();
        let result = configure_frame(true, false, true, |op| {
            applied.push(op);
            if matches!(op, FrameOperation::RoundedCorners(_)) {
                Err("unsupported corner preference".into())
            } else {
                Ok(())
            }
        });
        assert!(result.css_border);
        assert_eq!(result.warnings.len(), 1);
        assert!(applied.contains(&FrameOperation::Border(true)));
        assert!(applied.contains(&FrameOperation::Caption(true)));
    }

    #[test]
    fn failed_native_suppression_never_adds_a_css_border() {
        for failure in [
            FrameOperation::Shadow(false),
            FrameOperation::Border(true),
            FrameOperation::Caption(true),
        ] {
            let result = configure_frame(true, false, true, |op| {
                if op == failure {
                    Err("failed".into())
                } else {
                    Ok(())
                }
            });
            assert!(!result.css_border, "{failure:?}");
        }
    }

    #[test]
    fn windows_10_disables_shadow_without_unsupported_dwm_colors() {
        let mut applied = Vec::new();
        let result = configure_frame(true, false, false, |op| {
            applied.push(op);
            Ok(())
        });
        assert!(result.css_border);
        assert_eq!(applied, vec![FrameOperation::Shadow(false)]);
    }

    #[test]
    fn native_decorations_opaque_mode_and_maximization_do_not_draw_css() {
        for (custom, maximized) in [(false, false), (false, true), (true, true)]
        {
            let mut applied = Vec::new();
            let result = configure_frame(custom, maximized, true, |op| {
                applied.push(op);
                Ok(())
            });
            assert!(!result.css_border);
            assert_eq!(applied[0], FrameOperation::Shadow(!custom));
            assert!(applied.contains(&FrameOperation::RoundedCorners(false)));
        }
        let restored = configure_frame(true, false, true, |_| Ok(()));
        assert!(restored.css_border);
    }
}
