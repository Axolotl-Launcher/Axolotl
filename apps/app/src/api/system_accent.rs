use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

const ACCENT_COLOR_CHANGED: &str = "system-accent-color-changed";

#[cfg(target_os = "macos")]
thread_local! {
    static STOP_ACCENT_COLOR_WATCHER: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Debug, Clone, Serialize)]
pub struct AccentColor {
    hex: String,
    r: u8,
    g: u8,
    b: u8,
}

impl AccentColor {
    fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            hex: format!("#{r:02X}{g:02X}{b:02X}"),
            r,
            g,
            b,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn unit_to_u8(value: f64) -> Result<u8, String> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!("Invalid RGB component: {value}"));
    }

    Ok((value * 255.0).round() as u8)
}

fn emit_accent_color<R: Runtime>(app: &AppHandle<R>, color: AccentColor) {
    if let Err(error) = app.emit(ACCENT_COLOR_CHANGED, color) {
        tracing::warn!("Failed to emit system accent color: {error}");
    }
}

#[cfg(target_os = "windows")]
fn read_accent_color() -> Result<AccentColor, String> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    let settings = UISettings::new()
        .map_err(|error| format!("Failed to initialize UISettings: {error}"))?;
    let color =
        settings
            .GetColorValue(UIColorType::Accent)
            .map_err(|error| {
                format!("Failed to read Windows accent color: {error}")
            })?;

    Ok(AccentColor::new(color.R, color.G, color.B))
}

/// The Windows watcher thread parks until it is unparked. Its handle is kept so
/// a shutdown path can unpark and join it; nothing does that yet.
#[cfg(target_os = "windows")]
static ACCENT_COLOR_WATCHER: std::sync::OnceLock<std::thread::JoinHandle<()>> =
    std::sync::OnceLock::new();

#[cfg(target_os = "windows")]
fn start_accent_color_watcher<R: Runtime>(app: AppHandle<R>) {
    let watcher = std::thread::spawn(move || {
        use windows::{
            Foundation::TypedEventHandler, UI::ViewManagement::UISettings,
            core::IInspectable,
        };

        let settings = match UISettings::new() {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(
                    "Failed to initialize system accent color watcher: {error}"
                );
                return;
            }
        };
        let app_handle = app;
        let handler = TypedEventHandler::<UISettings, IInspectable>::new(
            move |_sender, _args| {
                if let Ok(color) = read_accent_color() {
                    emit_accent_color(&app_handle, color);
                }
                Ok(())
            },
        );
        let token = match settings.ColorValuesChanged(&handler) {
            Ok(token) => token,
            Err(error) => {
                tracing::warn!(
                    "Failed to listen for Windows accent color changes: {error}"
                );
                return;
            }
        };

        std::thread::park();
        let _ = settings.RemoveColorValuesChanged(token);
    });
    let _ = ACCENT_COLOR_WATCHER.set(watcher);
}

#[cfg(target_os = "macos")]
fn read_accent_color() -> Result<AccentColor, String> {
    use objc2_app_kit::{NSColor, NSColorSpace};

    let color = NSColor::controlAccentColor();
    let srgb = NSColorSpace::sRGBColorSpace();
    let color = color.colorUsingColorSpace(&srgb).ok_or_else(|| {
        "Unable to convert the macOS accent color to sRGB".to_string()
    })?;

    Ok(AccentColor::new(
        unit_to_u8(color.redComponent() as f64)?,
        unit_to_u8(color.greenComponent() as f64)?,
        unit_to_u8(color.blueComponent() as f64)?,
    ))
}

#[cfg(target_os = "macos")]
fn start_accent_color_watcher<R: Runtime>(app: AppHandle<R>) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::NSSystemColorsDidChangeNotification;
    use objc2_foundation::{
        NSNotification, NSNotificationCenter, NSOperationQueue,
    };

    let center = NSNotificationCenter::defaultCenter();
    let queue = NSOperationQueue::mainQueue();
    let block: RcBlock<dyn Fn(NonNull<NSNotification>)> =
        RcBlock::new(move |_notification| {
            if let Ok(color) = read_accent_color() {
                emit_accent_color(&app, color);
            }
        });

    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSSystemColorsDidChangeNotification),
            None,
            Some(&queue),
            &block,
        )
    };
    let stop = Box::new(move || unsafe {
        center.removeObserver((&*observer).as_ref());
    });

    STOP_ACCENT_COLOR_WATCHER.with(|watcher| {
        if let Some(stop) = watcher.borrow_mut().replace(stop) {
            stop();
        }
    });
}

#[cfg(target_os = "macos")]
fn stop_accent_color_watcher() {
    STOP_ACCENT_COLOR_WATCHER.with(|watcher| {
        if let Some(stop) = watcher.borrow_mut().take() {
            stop();
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn stop_accent_color_watcher() {}

#[cfg(target_os = "linux")]
async fn read_accent_color() -> Result<AccentColor, String> {
    use ashpd::desktop::settings::Settings;

    let settings = Settings::new().await.map_err(|error| {
        format!("Unable to connect to the XDG portal: {error}")
    })?;
    let color = settings.accent_color().await.map_err(|error| {
        format!("XDG portal does not provide accent-color: {error}")
    })?;

    Ok(AccentColor::new(
        unit_to_u8(color.red())?,
        unit_to_u8(color.green())?,
        unit_to_u8(color.blue())?,
    ))
}

#[cfg(target_os = "linux")]
fn start_accent_color_watcher<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        use ashpd::desktop::settings::Settings;
        use futures::StreamExt;

        let settings = match Settings::new().await {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!("Unable to connect to the XDG portal: {error}");
                return;
            }
        };
        let stream = match settings.receive_accent_color_changed().await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::warn!(
                    "Unable to listen for XDG accent-color changes: {error}"
                );
                return;
            }
        };

        futures::pin_mut!(stream);
        while let Some(color) = stream.next().await {
            let color = match (
                unit_to_u8(color.red()),
                unit_to_u8(color.green()),
                unit_to_u8(color.blue()),
            ) {
                (Ok(r), Ok(g), Ok(b)) => AccentColor::new(r, g, b),
                _ => continue,
            };
            emit_accent_color(&app, color);
        }
    });
}

#[tauri::command]
async fn system_accent_color() -> Result<AccentColor, String> {
    #[cfg(target_os = "windows")]
    return read_accent_color();

    #[cfg(target_os = "macos")]
    return read_accent_color();

    #[cfg(target_os = "linux")]
    return read_accent_color().await;

    #[allow(unreachable_code)]
    Err("Unsupported operating system".to_string())
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("system-accent")
        .setup(|app, _api| {
            start_accent_color_watcher(app.clone());
            Ok(())
        })
        .on_drop(|_| stop_accent_color_watcher())
        .invoke_handler(tauri::generate_handler![system_accent_color])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_rgb_components_are_rounded() {
        assert_eq!(unit_to_u8(0.0).unwrap(), 0);
        assert_eq!(unit_to_u8(0.5).unwrap(), 128);
        assert_eq!(unit_to_u8(1.0).unwrap(), 255);
    }

    #[test]
    fn invalid_normalized_rgb_components_are_rejected() {
        for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(unit_to_u8(value).is_err());
        }
    }
}
