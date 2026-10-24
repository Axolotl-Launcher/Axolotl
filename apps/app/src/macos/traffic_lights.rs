use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ptr::NonNull,
    rc::Rc,
};

use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSButton, NSView, NSViewFrameDidChangeNotification, NSWindow,
    NSWindowButton, NSWindowDidChangeBackingPropertiesNotification,
    NSWindowDidChangeScreenNotification, NSWindowDidEndLiveResizeNotification,
    NSWindowDidEnterFullScreenNotification,
    NSWindowDidExitFullScreenNotification, NSWindowDidMoveNotification,
    NSWindowDidResizeNotification, NSWindowDidUpdateNotification,
    NSWindowWillCloseNotification,
};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSObjectProtocol,
    NSOperationQueue, NSPoint, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize,
};
use serde::Deserialize;
use tauri::WebviewWindow;

type ObserverToken = Retained<ProtocolObject<dyn NSObjectProtocol>>;

thread_local! {
    static CONTROLLERS: RefCell<HashMap<usize, Rc<TrafficLightController>>> =
        RefCell::new(HashMap::new());
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomAnchorRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_width: f64,
    viewport_height: f64,
}

impl DomAnchorRect {
    fn is_valid(self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.viewport_width,
            self.viewport_height,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.width > 0.0
            && self.height > 0.0
            && self.viewport_width > 0.0
            && self.viewport_height > 0.0
    }
}

struct ObservedButton {
    button: Retained<NSButton>,
    previously_posted_frame_changes: bool,
    token: ObserverToken,
}

struct ButtonSnapshot {
    button: Retained<NSButton>,
    superview: Retained<NSView>,
    frame_in_window: NSRect,
}

struct TrafficLightController {
    key: usize,
    window: Retained<NSWindow>,
    webview: RefCell<Retained<NSView>>,
    anchor: Cell<Option<DomAnchorRect>>,
    scheduled: Cell<bool>,
    repositioning: Cell<bool>,
    closed: Cell<bool>,
    window_observers: RefCell<Vec<ObserverToken>>,
    button_observers: RefCell<Vec<ObservedButton>>,
}

impl TrafficLightController {
    fn new(
        key: usize,
        window: Retained<NSWindow>,
        webview: Retained<NSView>,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            key,
            window,
            webview: RefCell::new(webview),
            anchor: Cell::new(None),
            scheduled: Cell::new(false),
            repositioning: Cell::new(false),
            closed: Cell::new(false),
            window_observers: RefCell::new(Vec::new()),
            button_observers: RefCell::new(Vec::new()),
        });
        controller.install_window_observers();
        controller
    }

    fn install_window_observers(self: &Rc<Self>) {
        let notification_names = unsafe {
            [
                NSWindowDidResizeNotification,
                NSWindowDidEndLiveResizeNotification,
                NSWindowDidMoveNotification,
                NSWindowDidChangeScreenNotification,
                NSWindowDidChangeBackingPropertiesNotification,
                NSWindowDidEnterFullScreenNotification,
                NSWindowDidExitFullScreenNotification,
                NSWindowDidUpdateNotification,
            ]
        };
        for name in notification_names {
            let key = self.key;
            let block =
                RcBlock::new(move |_notification: NonNull<NSNotification>| {
                    with_controller(
                        key,
                        TrafficLightController::schedule_reposition,
                    );
                });
            let token = unsafe {
                NSNotificationCenter::defaultCenter()
                    .addObserverForName_object_queue_usingBlock(
                        Some(name),
                        Some(&self.window),
                        Some(&NSOperationQueue::mainQueue()),
                        &block,
                    )
            };
            self.window_observers.borrow_mut().push(token);
        }

        let key = self.key;
        let block =
            RcBlock::new(move |_notification: NonNull<NSNotification>| {
                CONTROLLERS.with(|controllers| {
                    if let Some(controller) =
                        controllers.borrow_mut().remove(&key)
                    {
                        controller.cleanup();
                    }
                });
            });
        let token = unsafe {
            NSNotificationCenter::defaultCenter()
                .addObserverForName_object_queue_usingBlock(
                    Some(NSWindowWillCloseNotification),
                    Some(&self.window),
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
        };
        self.window_observers.borrow_mut().push(token);
    }

    fn schedule_reposition(self: &Rc<Self>) {
        assert_main_thread();
        if self.closed.get() || self.scheduled.replace(true) {
            return;
        }

        let key = self.key;
        let block = RcBlock::new(move || {
            with_controller(key, |controller| {
                controller.scheduled.set(false);
                controller.reposition();
            });
        });
        unsafe {
            let modes = NSArray::from_slice(&[NSRunLoopCommonModes]);
            NSRunLoop::mainRunLoop().performInModes_block(&modes, &block);
        }
    }

    fn reposition(self: &Rc<Self>) {
        assert_main_thread();
        let Some(anchor) = self.anchor.get() else {
            return;
        };
        if self.closed.get() || self.repositioning.replace(true) {
            return;
        }

        let webview = self.webview.borrow();
        let anchor_in_webview =
            dom_rect_in_view(anchor, webview.bounds(), webview.isFlipped());
        let anchor_in_window =
            webview.convertRect_toView(anchor_in_webview, None);
        self.expand_titlebar_container(anchor_in_window);
        // Resizing the container can move the buttons through autoresizing.
        let Some(buttons) = self.current_buttons() else {
            self.repositioning.set(false);
            return;
        };
        self.refresh_button_observers(&buttons);

        let group_min_x = buttons
            .iter()
            .map(|button| button.frame_in_window.origin.x)
            .fold(f64::INFINITY, f64::min);
        let group_min_y = buttons
            .iter()
            .map(|button| button.frame_in_window.origin.y)
            .fold(f64::INFINITY, f64::min);
        let group_max_x = buttons
            .iter()
            .map(|button| {
                button.frame_in_window.origin.x
                    + button.frame_in_window.size.width
            })
            .fold(f64::NEG_INFINITY, f64::max);
        let group_max_y = buttons
            .iter()
            .map(|button| {
                button.frame_in_window.origin.y
                    + button.frame_in_window.size.height
            })
            .fold(f64::NEG_INFINITY, f64::max);
        let group_frame = NSRect::new(
            NSPoint::new(group_min_x, group_min_y),
            NSSize::new(group_max_x - group_min_x, group_max_y - group_min_y),
        );
        let target_group_origin = NSPoint::new(
            group_frame.origin.x,
            anchor_in_window.origin.y
                + (anchor_in_window.size.height - group_frame.size.height)
                    / 2.0,
        );
        let translation = NSPoint::new(
            target_group_origin.x - group_frame.origin.x,
            target_group_origin.y - group_frame.origin.y,
        );

        if translation.x != 0.0 || translation.y != 0.0 {
            for snapshot in buttons {
                let mut target_in_window = snapshot.frame_in_window;
                target_in_window.origin.x += translation.x;
                target_in_window.origin.y += translation.y;
                let target_in_superview = snapshot
                    .superview
                    .convertRect_fromView(target_in_window, None);
                snapshot.button.setFrameOrigin(target_in_superview.origin);
            }
        }

        self.repositioning.set(false);
    }

    fn expand_titlebar_container(&self, anchor_in_window: NSRect) {
        let Some(close_button) = self
            .window
            .standardWindowButton(NSWindowButton::CloseButton)
        else {
            return;
        };
        // Keep the native titlebar's hit-test area around the moved buttons.
        let Some((container, superview)) = (unsafe {
            close_button.superview().and_then(|titlebar| {
                let container = titlebar.superview()?;
                let superview = container.superview()?;
                Some((container, superview))
            })
        }) else {
            return;
        };
        let frame_in_window =
            superview.convertRect_toView(container.frame(), None);
        let expanded_frame =
            expanded_titlebar_frame(frame_in_window, anchor_in_window);
        if expanded_frame != frame_in_window {
            container
                .setFrame(superview.convertRect_fromView(expanded_frame, None));
        }
    }

    fn current_buttons(&self) -> Option<Vec<ButtonSnapshot>> {
        let buttons = [
            self.window
                .standardWindowButton(NSWindowButton::CloseButton)?,
            self.window
                .standardWindowButton(NSWindowButton::MiniaturizeButton)?,
            self.window
                .standardWindowButton(NSWindowButton::ZoomButton)?,
        ];

        buttons
            .into_iter()
            .map(|button| {
                let superview = unsafe { button.superview() }?;
                let frame_in_window =
                    superview.convertRect_toView(button.frame(), None);
                Some(ButtonSnapshot {
                    button,
                    superview,
                    frame_in_window,
                })
            })
            .collect()
    }

    fn refresh_button_observers(self: &Rc<Self>, buttons: &[ButtonSnapshot]) {
        let current_ids: Vec<_> = buttons
            .iter()
            .map(|snapshot| Retained::as_ptr(&snapshot.button) as usize)
            .collect();
        let observed_ids: Vec<_> = self
            .button_observers
            .borrow()
            .iter()
            .map(|observed| Retained::as_ptr(&observed.button) as usize)
            .collect();
        if current_ids == observed_ids {
            return;
        }

        self.remove_button_observers();
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let mut observers = self.button_observers.borrow_mut();
        for snapshot in buttons {
            let previously_posted_frame_changes =
                snapshot.button.postsFrameChangedNotifications();
            snapshot.button.setPostsFrameChangedNotifications(true);
            let key = self.key;
            let block =
                RcBlock::new(move |_notification: NonNull<NSNotification>| {
                    with_controller(key, |controller| {
                        if !controller.repositioning.get() {
                            controller.schedule_reposition();
                        }
                    });
                });
            let token = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSViewFrameDidChangeNotification),
                    Some(&snapshot.button),
                    Some(&queue),
                    &block,
                )
            };
            observers.push(ObservedButton {
                button: snapshot.button.clone(),
                previously_posted_frame_changes,
                token,
            });
        }
    }

    fn remove_button_observers(&self) {
        let center = NSNotificationCenter::defaultCenter();
        for observed in self.button_observers.borrow_mut().drain(..) {
            let token: &AnyObject =
                AsRef::<AnyObject>::as_ref(&*observed.token);
            unsafe {
                center.removeObserver(token);
            }
            observed.button.setPostsFrameChangedNotifications(
                observed.previously_posted_frame_changes,
            );
        }
    }

    fn cleanup(&self) {
        assert_main_thread();
        if self.closed.replace(true) {
            return;
        }
        self.remove_button_observers();
        let center = NSNotificationCenter::defaultCenter();
        for token in self.window_observers.borrow_mut().drain(..) {
            let token: &AnyObject = AsRef::<AnyObject>::as_ref(&*token);
            unsafe {
                center.removeObserver(token);
            }
        }
    }
}

fn with_controller(
    key: usize,
    callback: impl FnOnce(&Rc<TrafficLightController>),
) {
    CONTROLLERS.with(|controllers| {
        if let Some(controller) = controllers.borrow().get(&key) {
            callback(controller);
        }
    });
}

fn dom_rect_in_view(
    anchor: DomAnchorRect,
    bounds: NSRect,
    flipped: bool,
) -> NSRect {
    let scale_x = bounds.size.width / anchor.viewport_width;
    let scale_y = bounds.size.height / anchor.viewport_height;
    let origin_x = bounds.origin.x + anchor.x * scale_x;
    let origin_y = if flipped {
        bounds.origin.y + anchor.y * scale_y
    } else {
        bounds.origin.y + bounds.size.height
            - (anchor.y + anchor.height) * scale_y
    };
    NSRect::new(
        NSPoint::new(origin_x, origin_y),
        NSSize::new(anchor.width * scale_x, anchor.height * scale_y),
    )
}

fn expanded_titlebar_frame(frame: NSRect, anchor: NSRect) -> NSRect {
    let min_y = frame.origin.y.min(anchor.origin.y);
    let max_y = (frame.origin.y + frame.size.height)
        .max(anchor.origin.y + anchor.size.height);
    NSRect::new(
        NSPoint::new(frame.origin.x, min_y),
        NSSize::new(frame.size.width, max_y - min_y),
    )
}

fn assert_main_thread() {
    assert!(
        MainThreadMarker::new().is_some(),
        "traffic-light AppKit access must run on the main thread"
    );
}

unsafe fn upsert_controller(
    window_ptr: *mut std::ffi::c_void,
    webview_ptr: *mut std::ffi::c_void,
) -> Result<Rc<TrafficLightController>, String> {
    assert_main_thread();
    let key = window_ptr as usize;
    let window = unsafe { Retained::retain(window_ptr.cast::<NSWindow>()) }
        .ok_or_else(|| "NSWindow was released".to_string())?;
    let webview = unsafe { Retained::retain(webview_ptr.cast::<NSView>()) }
        .ok_or_else(|| "WKWebView was released".to_string())?;

    Ok(CONTROLLERS.with(|controllers| {
        let mut controllers = controllers.borrow_mut();
        if let Some(controller) = controllers.get(&key) {
            controller.webview.replace(webview);
            return controller.clone();
        }
        let controller = TrafficLightController::new(key, window, webview);
        controllers.insert(key, controller.clone());
        controller
    }))
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    window.with_webview(|platform_webview| {
        if let Err(error) = unsafe {
            upsert_controller(
                platform_webview.ns_window(),
                platform_webview.inner(),
            )
        } {
            tracing::warn!(
                "Failed to install native traffic-light positioning: {error}"
            );
        }
    })
}

#[tauri::command]
pub async fn update_traffic_lights_anchor(
    window: WebviewWindow,
    anchor: DomAnchorRect,
) -> Result<(), String> {
    if !anchor.is_valid() {
        return Err("invalid titlebar anchor geometry".to_string());
    }

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    window
        .with_webview(move |platform_webview| {
            let result = unsafe {
                upsert_controller(
                    platform_webview.ns_window(),
                    platform_webview.inner(),
                )
            }
            .map(|controller| {
                controller.anchor.set(Some(anchor));
                controller.schedule_reposition();
            });
            let _ = result_tx.send(result);
        })
        .map_err(|error| error.to_string())?;

    result_rx.await.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_native_titlebar_to_contain_the_dom_titlebar() {
        let frame =
            NSRect::new(NSPoint::new(0.0, 772.0), NSSize::new(1280.0, 28.0));
        let anchor =
            NSRect::new(NSPoint::new(0.0, 752.0), NSSize::new(1280.0, 48.0));

        assert_eq!(expanded_titlebar_frame(frame, anchor), anchor);
        assert_eq!(expanded_titlebar_frame(anchor, frame), anchor);
        assert_eq!(expanded_titlebar_frame(anchor, anchor), anchor);
    }

    #[test]
    fn converts_dom_rect_for_unflipped_appkit_view() {
        let rect = dom_rect_in_view(
            DomAnchorRect {
                x: 20.0,
                y: 10.0,
                width: 100.0,
                height: 40.0,
                viewport_width: 400.0,
                viewport_height: 300.0,
            },
            NSRect::new(NSPoint::new(5.0, 7.0), NSSize::new(800.0, 600.0)),
            false,
        );

        assert_eq!(rect.origin, NSPoint::new(45.0, 507.0));
        assert_eq!(rect.size, NSSize::new(200.0, 80.0));
    }

    #[test]
    fn converts_dom_rect_for_flipped_appkit_view() {
        let rect = dom_rect_in_view(
            DomAnchorRect {
                x: 20.0,
                y: 10.0,
                width: 100.0,
                height: 40.0,
                viewport_width: 400.0,
                viewport_height: 300.0,
            },
            NSRect::new(NSPoint::new(5.0, 7.0), NSSize::new(800.0, 600.0)),
            true,
        );

        assert_eq!(rect.origin, NSPoint::new(45.0, 27.0));
        assert_eq!(rect.size, NSSize::new(200.0, 80.0));
    }
}
