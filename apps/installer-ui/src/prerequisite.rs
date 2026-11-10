#[cfg(test)]
use std::cell::Cell;
use std::{cell::RefCell, ffi::OsString, mem::size_of};

use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, WPARAM},
        Graphics::Gdi::{
            CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DC_BRUSH,
            DC_PEN, DEFAULT_CHARSET, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
            DeleteObject, DrawTextW, FillRect, GetStockObject, HBRUSH, HDC,
            HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, RestoreDC, RoundRect, SaveDC,
            SelectObject, SetBkMode, SetDCBrushColor, SetDCPenColor,
            SetTextColor, TRANSPARENT, WHITE_BRUSH,
        },
        UI::{
            Controls::{
                DRAWITEMSTRUCT, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX,
                InitCommonControlsEx, ODS_FOCUS, ODS_SELECTED,
            },
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                BS_OWNERDRAW, CreateIconFromResourceEx, DI_NORMAL, DLGTEMPLATE,
                DS_CENTER, DS_FIXEDSYS, DS_MODALFRAME, DS_SETFONT, DestroyIcon,
                DialogBoxIndirectParamW, DrawIconEx, EndDialog, GUITHREADINFO,
                GWLP_USERDATA, GetDlgCtrlID, GetDlgItem, GetGUIThreadInfo,
                GetWindowLongPtrW, GetWindowThreadProcessId, HICON,
                IMAGE_FLAGS, SendMessageW, SetWindowLongPtrW, SetWindowTextW,
                WM_CLOSE, WM_COMMAND, WM_CTLCOLORDLG, WM_CTLCOLORSTATIC,
                WM_DRAWITEM, WM_INITDIALOG, WM_SETFONT, WS_CAPTION, WS_CHILD,
                WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, Result},
};

const DOWNLOAD_BUTTON: i32 = 1001;
const CANCEL_BUTTON: i32 = 2;
const DOWNLOAD_ICON: &[u8] =
    include_bytes!("../../app/nsis/assets/webview2-download.ico");
const CANCEL_ICON: &[u8] =
    include_bytes!("../../app/nsis/assets/webview2-cancel.ico");

pub fn requested_language() -> Option<bool> {
    native_language(&std::env::args_os().skip(1).collect::<Vec<_>>())
}

fn native_language(args: &[OsString]) -> Option<bool> {
    args.iter()
        .any(|value| value == "--webview2-required")
        .then(|| {
            args.windows(2)
                .any(|pair| pair[0] == "--language" && pair[1] == "2052")
        })
}

struct Copy {
    face: &'static str,
    title: &'static str,
    message: &'static str,
    download: &'static str,
    cancel: &'static str,
}

fn copy(chinese: bool) -> Copy {
    if chinese {
        Copy {
            face: "Microsoft YaHei UI",
            title: "需要安装 WebView2 Runtime",
            message: "未检测到 Microsoft Edge WebView2 Runtime。Axolotl Launcher 及其安装界面需要此组件。\r\n\r\n请前往微软官网下载并安装 Evergreen WebView2 Runtime，然后重新运行本安装程序。",
            download: "前往下载",
            cancel: "取消",
        }
    } else {
        Copy {
            face: "Segoe UI",
            title: "WebView2 Runtime is required",
            message: "Microsoft Edge WebView2 Runtime was not found. Axolotl Launcher and its installer require this component.\r\n\r\nDownload and install the Evergreen WebView2 Runtime from Microsoft, then run this installer again.",
            download: "Go to download",
            cancel: "Cancel",
        }
    }
}

struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

struct Font(HFONT);

impl Font {
    fn new(dpi: u32, pixels: i32, weight: i32, face: &str) -> Result<Self> {
        let face: Vec<u16> = face.encode_utf16().chain(Some(0)).collect();
        let font = unsafe {
            CreateFontW(
                -(pixels * dpi as i32 / 96),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                PCWSTR(face.as_ptr()),
            )
        };
        if font.0.is_null() {
            Err(windows::core::Error::from_win32())
        } else {
            Ok(Self(font))
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(HGDIOBJ(self.0.0)) };
    }
}

struct Appearance {
    dpi: u32,
    fonts: [Font; 3],
    icons: [Icon; 2],
}

impl Appearance {
    fn new(dpi: u32, face: &str) -> Result<Self> {
        let pixels = (dpi * 20 / 96) as i32;
        let load = |data| -> Result<Icon> {
            let frame = icon_frame(data, pixels as u32).expect(
                "embedded prerequisite icon must contain a valid frame",
            );
            Ok(Icon(unsafe {
                CreateIconFromResourceEx(
                    frame,
                    true,
                    0x0003_0000,
                    pixels,
                    pixels,
                    IMAGE_FLAGS(0),
                )
            }?))
        };
        Ok(Self {
            dpi,
            fonts: [
                Font::new(dpi, 14, 400, face)?,
                Font::new(dpi, 22, 600, face)?,
                Font::new(dpi, 15, 400, face)?,
            ],
            icons: [load(DOWNLOAD_ICON)?, load(CANCEL_ICON)?],
        })
    }
}

struct DialogState {
    copy: Copy,
    appearance: RefCell<Option<Appearance>>,
    #[cfg(test)]
    action: Option<i32>,
    #[cfg(test)]
    verified: Cell<bool>,
}

impl DialogState {
    fn new(chinese: bool) -> Self {
        Self {
            copy: copy(chinese),
            appearance: RefCell::new(None),
            #[cfg(test)]
            action: None,
            #[cfg(test)]
            verified: Cell::new(false),
        }
    }
}

pub fn show(chinese: bool) -> i32 {
    let result = run_dialog(&mut DialogState::new(chinese));
    if result == DOWNLOAD_BUTTON as isize {
        10
    } else {
        if result == -1 {
            eprintln!("creating the WebView2 prerequisite dialog failed");
        }
        2
    }
}

fn run_dialog(state: &DialogState) -> isize {
    let controls = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES,
    };
    if !unsafe { InitCommonControlsEx(&controls) }.as_bool() {
        return -1;
    }
    let template = dialog_template();
    unsafe {
        DialogBoxIndirectParamW(
            None,
            template.as_ptr().cast::<DLGTEMPLATE>(),
            None,
            Some(dialog_proc),
            LPARAM(std::ptr::from_ref(state) as isize),
        )
    }
}

unsafe extern "system" fn dialog_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        WM_INITDIALOG => {
            let Some(state) =
                (unsafe { (lparam.0 as *const DialogState).as_ref() })
            else {
                return 0;
            };
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, lparam.0);
            }
            if initialize_dialog(window, state).is_err() {
                let _ = unsafe { EndDialog(window, -1) };
            }
            #[cfg(test)]
            if let Some(action) = state.action {
                state.verified.set(verify_buttons(window, state));
                let _ = unsafe {
                    windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                        Some(window),
                        WM_COMMAND,
                        WPARAM(action as usize),
                        LPARAM(0),
                    )
                };
            }
            1
        }
        WM_CTLCOLORDLG | WM_CTLCOLORSTATIC => {
            let dc = HDC(wparam.0 as *mut _);
            unsafe {
                SetBkMode(dc, TRANSPARENT);
                let is_body = message == WM_CTLCOLORSTATIC
                    && GetDlgCtrlID(HWND(lparam.0 as *mut _)) == 1003;
                SetTextColor(
                    dc,
                    color(if is_body { 0x505b6b } else { 0x17212f }),
                );
                GetStockObject(WHITE_BRUSH).0 as isize
            }
        }
        WM_DRAWITEM => {
            let state = unsafe {
                (GetWindowLongPtrW(window, GWLP_USERDATA) as *const DialogState)
                    .as_ref()
            };
            let item = unsafe { (lparam.0 as *const DRAWITEMSTRUCT).as_ref() };
            if let (Some(state), Some(item)) = (state, item) {
                return isize::from(draw_button(state, item));
            }
            0
        }
        WM_COMMAND => {
            let mut button = (wparam.0 & 0xffff) as i32;
            if button == 1 {
                let mut thread = GUITHREADINFO {
                    cbSize: size_of::<GUITHREADINFO>() as u32,
                    ..Default::default()
                };
                if unsafe {
                    GetGUIThreadInfo(
                        GetWindowThreadProcessId(window, None),
                        &raw mut thread,
                    )
                }
                .is_ok()
                {
                    button = unsafe { GetDlgCtrlID(thread.hwndFocus) };
                }
            }
            if matches!(button, DOWNLOAD_BUTTON | CANCEL_BUTTON) {
                let _ = unsafe { EndDialog(window, button as isize) };
                return 1;
            }
            0
        }
        WM_CLOSE => {
            let _ = unsafe { EndDialog(window, CANCEL_BUTTON as isize) };
            1
        }
        _ => 0,
    }
}

fn initialize_dialog(window: HWND, state: &DialogState) -> Result<()> {
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    *state.appearance.borrow_mut() =
        Some(Appearance::new(dpi, state.copy.face)?);
    set_text(window, "Axolotl Launcher")?;
    set_text(unsafe { GetDlgItem(Some(window), 1002) }?, state.copy.title)?;
    set_text(
        unsafe { GetDlgItem(Some(window), 1003) }?,
        state.copy.message,
    )?;
    let (body, heading, button_font) = {
        let appearance = state.appearance.borrow();
        let appearance =
            appearance.as_ref().expect("appearance was just created");
        (
            appearance.fonts[0].0,
            appearance.fonts[1].0,
            appearance.fonts[2].0,
        )
    };
    for (id, font) in [
        (1002, heading),
        (1003, body),
        (DOWNLOAD_BUTTON, button_font),
        (CANCEL_BUTTON, button_font),
    ] {
        unsafe {
            SendMessageW(
                GetDlgItem(Some(window), id)?,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }
    for (button, text) in [
        (DOWNLOAD_BUTTON, state.copy.download),
        (CANCEL_BUTTON, state.copy.cancel),
    ] {
        let control = unsafe { GetDlgItem(Some(window), button) }?;
        set_text(control, text)?;
    }
    Ok(())
}

fn color(rgb: u32) -> COLORREF {
    COLORREF((rgb & 0xff) << 16 | (rgb & 0xff00) | (rgb >> 16))
}

fn draw_button(state: &DialogState, item: &DRAWITEMSTRUCT) -> bool {
    let primary = item.CtlID == DOWNLOAD_BUTTON as u32;
    if !primary && item.CtlID != CANCEL_BUTTON as u32 {
        return false;
    }
    let (dpi, font, icon) = {
        let appearance = state.appearance.borrow();
        let Some(appearance) = appearance.as_ref() else {
            return true;
        };
        (
            appearance.dpi,
            appearance.fonts[2].0,
            appearance.icons[usize::from(!primary)].0,
        )
    };
    let px = |value: i32| value * dpi as i32 / 96;
    let pressed = item.itemState.0 & ODS_SELECTED.0 != 0;
    let focused = item.itemState.0 & ODS_FOCUS.0 != 0;
    let fill = match (primary, pressed) {
        (true, false) => 0xf0f6ff,
        (true, true) => 0xe0ecff,
        (false, false) => 0xffffff,
        (false, true) => 0xf0f2f5,
    };
    let border = if focused {
        0x3478ce
    } else if primary {
        0xcbdcf5
    } else {
        0xdce1e7
    };
    let rect = item.rcItem;
    let offset = if pressed { px(1) } else { 0 };
    let icon_size = px(20);
    let icon_y = rect.top + (rect.bottom - rect.top - icon_size) / 2 + offset;
    let mut text_rect = rect;
    text_rect.left += px(48);
    text_rect.right -= px(16);
    text_rect.top += offset;
    let mut text: Vec<u16> = if primary {
        state.copy.download
    } else {
        state.copy.cancel
    }
    .encode_utf16()
    .collect();
    unsafe {
        let saved = SaveDC(item.hDC);
        FillRect(item.hDC, &rect, HBRUSH(GetStockObject(WHITE_BRUSH).0));
        SelectObject(item.hDC, GetStockObject(DC_BRUSH));
        SelectObject(item.hDC, GetStockObject(DC_PEN));
        SetDCBrushColor(item.hDC, color(fill));
        SetDCPenColor(item.hDC, color(border));
        let _ = RoundRect(
            item.hDC,
            rect.left,
            rect.top,
            rect.right - 1,
            rect.bottom - 1,
            px(12),
            px(12),
        );
        let _ = DrawIconEx(
            item.hDC,
            rect.left + px(16) + offset,
            icon_y,
            icon,
            icon_size,
            icon_size,
            0,
            None,
            DI_NORMAL,
        );
        SelectObject(item.hDC, HGDIOBJ(font.0));
        SetBkMode(item.hDC, TRANSPARENT);
        SetTextColor(item.hDC, color(0x17212f));
        DrawTextW(
            item.hDC,
            &mut text,
            &raw mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
        if saved != 0 {
            let _ = RestoreDC(item.hDC, saved);
        }
    }
    true
}

fn set_text(window: HWND, text: &str) -> Result<()> {
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe { SetWindowTextW(window, PCWSTR(text.as_ptr())) }
}

fn icon_frame(bytes: &[u8], pixels: u32) -> Option<&[u8]> {
    let count = u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?);
    (0..usize::from(count))
        .filter_map(|index| {
            let entry = bytes.get(6 + index * 16..6 + (index + 1) * 16)?;
            let width = if entry[0] == 0 {
                256
            } else {
                u32::from(entry[0])
            };
            let length =
                u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
            let offset =
                u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
            Some((width, bytes.get(offset..offset.checked_add(length)?)?))
        })
        .min_by_key(|(width, _)| width.abs_diff(pixels))
        .map(|(_, frame)| frame)
}

fn dialog_template() -> Vec<u32> {
    fn dword(words: &mut Vec<u16>, value: u32) {
        words.extend([value as u16, (value >> 16) as u16]);
    }
    fn align(words: &mut Vec<u16>) {
        if words.len() % 2 != 0 {
            words.push(0);
        }
    }
    let mut words = Vec::new();
    dword(
        &mut words,
        WS_POPUP.0
            | WS_CAPTION.0
            | WS_SYSMENU.0
            | DS_CENTER as u32
            | DS_MODALFRAME as u32
            | DS_SETFONT as u32
            | DS_FIXEDSYS as u32,
    );
    dword(&mut words, 0);
    words.extend([4, 0, 0, 400, 150, 0, 0, 0, 9]);
    words.extend("Segoe UI".encode_utf16().chain(Some(0)));
    let child = WS_CHILD.0 | WS_VISIBLE.0;
    for (style, position, id, class) in [
        (child, [20, 14, 360, 18], 1002, 0x0082),
        (child, [20, 38, 360, 44], 1003, 0x0082),
        (
            child | WS_TABSTOP.0 | BS_OWNERDRAW as u32,
            [20, 118, 360, 24],
            CANCEL_BUTTON as u16,
            0x0080,
        ),
        (
            child | WS_TABSTOP.0 | BS_OWNERDRAW as u32,
            [20, 90, 360, 24],
            DOWNLOAD_BUTTON as u16,
            0x0080,
        ),
    ] {
        align(&mut words);
        dword(&mut words, style);
        dword(&mut words, 0);
        words.extend(position);
        words.extend([id, 0xffff, class, 0, 0]);
    }
    align(&mut words);
    words
        .chunks_exact(2)
        .map(|pair| u32::from(pair[0]) | (u32::from(pair[1]) << 16))
        .collect()
}

#[cfg(test)]
fn verify_buttons(window: HWND, state: &DialogState) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    [DOWNLOAD_BUTTON, CANCEL_BUTTON]
        .into_iter()
        .zip([state.copy.download, state.copy.cancel])
        .all(|(button, expected_text)| {
            let Ok(control) = (unsafe { GetDlgItem(Some(window), button) })
            else {
                return false;
            };
            let mut text = [0_u16; 128];
            let length = unsafe { GetWindowTextW(control, &mut text) } as usize;
            String::from_utf16_lossy(&text[..length]) == expected_text
                && button_icon_is_painted(state, control, button)
        })
}

#[cfg(test)]
fn button_icon_is_painted(
    state: &DialogState,
    control: HWND,
    button: i32,
) -> bool {
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC,
            CreateDIBSection, DIB_RGB_COLORS, DeleteDC, GetPixel,
        },
        UI::WindowsAndMessaging::GetClientRect,
    };

    let Some(dpi) = state.appearance.borrow().as_ref().map(|value| value.dpi)
    else {
        return false;
    };
    let mut rect = RECT::default();
    if unsafe { GetClientRect(control, &raw mut rect) }.is_err() {
        return false;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: rect.right,
            biHeight: -rect.bottom,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.0.is_null() {
            return false;
        }
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(
            Some(dc),
            &info,
            DIB_RGB_COLORS,
            &raw mut bits,
            None,
            0,
        ) else {
            let _ = DeleteDC(dc);
            return false;
        };
        let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
        let drawn = draw_button(
            state,
            &DRAWITEMSTRUCT {
                CtlID: button as u32,
                hDC: dc,
                rcItem: rect,
                ..Default::default()
            },
        );
        let px = |value: i32| value * dpi as i32 / 96;
        let top = (rect.bottom - px(20)) / 2;
        let mut ink = 0;
        for y in top..top + px(20) {
            for x in px(16)..px(36) {
                let pixel = GetPixel(dc, x, y).0;
                if pixel & 0xff < 180
                    && (pixel >> 8) & 0xff < 180
                    && (pixel >> 16) & 0xff < 180
                {
                    ink += 1;
                }
            }
        }
        SelectObject(dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        drawn && ink > px(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mode_is_explicit_and_selects_the_requested_language() {
        let args = |values: &[&str]| {
            values.iter().map(OsString::from).collect::<Vec<_>>()
        };
        assert_eq!(native_language(&args(&["--installer", "setup.exe"])), None);
        assert_eq!(
            native_language(&args(&["--webview2-required"])),
            Some(false)
        );
        assert_eq!(
            native_language(&args(&[
                "--webview2-required",
                "--language",
                "2052"
            ])),
            Some(true)
        );
    }

    #[test]
    fn embedded_icons_have_valid_frames_for_scaled_buttons() {
        for icon in [DOWNLOAD_ICON, CANCEL_ICON] {
            for pixels in [16, 20, 24, 25, 32, 40, 48] {
                assert!(
                    icon_frame(icon, pixels)
                        .unwrap()
                        .starts_with(b"\x89PNG\r\n\x1a\n")
                );
            }
        }
        assert!(icon_frame(&[], 20).is_none());
    }

    #[test]
    fn native_dialog_has_real_button_icons_plain_labels_and_returns_actions() {
        for (chinese, action) in
            [(true, CANCEL_BUTTON), (false, DOWNLOAD_BUTTON)]
        {
            let mut state = DialogState::new(chinese);
            state.action = Some(action);
            assert_eq!(run_dialog(&mut state), action as isize);
            assert!(state.verified.get());
        }
    }
}
