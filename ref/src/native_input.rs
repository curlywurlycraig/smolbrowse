//! minifb exposes mouse snapshots only. Preserve native transitions on macOS,
//! including a complete down/up pair between two display frames.
use crate::{
    display::Viewport,
    input::{canvas_position, Capture, Raw},
};
use block2::RcBlock;
use minifb::{Key, Window};
use objc2::{
    class, declare_class, msg_send, msg_send_id, mutability,
    rc::Retained,
    runtime::{AnyObject, Sel},
    ClassType, DeclaredClass,
};
use objc2_app_kit::NSTextView;
use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, rc::Rc};

declare_class!(
    struct TextSink;
    // NSTextView owns composition; this subclass exposes only committed text.
    unsafe impl ClassType for TextSink {
        type Super = NSTextView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "SmolbrowseTextSink";
    }
    impl DeclaredClass for TextSink { type Ivars = Rc<RefCell<Capture>>; }
    unsafe impl TextSink {
        #[method(insertText:replacementRange:)]
        fn insert_text(&self, value: &AnyObject, _range: NSRange) {
            unsafe {
                let is_string: bool = msg_send![value, isKindOfClass: NSString::class()];
                let string: *const NSString = if is_string { std::ptr::from_ref(value).cast() }
                    else { msg_send![value, string] };
                if let Some(string) = string.as_ref() {
                    let mut state = self.ivars().borrow_mut();
                    let mods = state.modifiers();
                    for ch in string.to_string().chars().filter(|c| !c.is_control()) {
                        state.push(Raw::Text(ch, mods));
                    }
                }
                let _: () = msg_send![self, unmarkText];
                self.setString(&NSString::from_str(""));
            }
        }
        #[method(doCommandBySelector:)]
        fn command(&self, _selector: Sel) { /* Editing controls belong to the site. */ }
    }
);

pub struct Monitor {
    token: Retained<AnyObject>,
    sink: Retained<TextSink>,
}
impl Monitor {
    pub fn pressed_keys(&self) -> std::collections::HashSet<Key> {
        // Refresh on focus changes: releases while another app has focus do
        // not necessarily arrive in this application's event stream.
        (0..128)
            .filter(|code| unsafe { CGEventSourceKeyState(0, *code) })
            .map(physical_key)
            .filter(|key| *key != Key::Unknown)
            .collect()
    }

    pub fn buttons(&self) -> [bool; 3] {
        unsafe {
            let bits: usize = msg_send![class!(NSEvent), pressedMouseButtons];
            [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0]
        }
    }

    pub fn focused(&self) -> bool {
        // minifb 0.29's macOS is_active() inverts the native boolean.
        unsafe {
            let window: *mut AnyObject = msg_send![&*self.sink, window];
            let key: bool = msg_send![window, isKeyWindow];
            key
        }
    }

    pub fn reset_text(&self) {
        unsafe {
            let context: *mut AnyObject = msg_send![&*self.sink, inputContext];
            let _: () = msg_send![context, discardMarkedText];
            self.sink.setString(&NSString::from_str(""));
        }
    }

    pub fn new(window: &Window, capture: Rc<RefCell<Capture>>) -> anyhow::Result<Self> {
        let RawWindowHandle::AppKit(handle) = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("window handle: {e}"))?
            .as_raw()
        else {
            anyhow::bail!("expected an AppKit window");
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();
        // All AppKit access and callback execution stay on the main thread.
        // The monitor is removed before the owning minifb window is dropped.
        unsafe {
            let native_window: *mut AnyObject = msg_send![view, window];
            let _: () = msg_send![native_window, setAcceptsMouseMovedEvents: true];
            let mtm = MainThreadMarker::new()
                .ok_or_else(|| anyhow::anyhow!("input must run on the main thread"))?;
            let allocated = mtm.alloc::<TextSink>().set_ivars(capture.clone());
            let sink: Retained<TextSink> =
                msg_send_id![super(allocated), initWithFrame: NSRect::default()];
            sink.setRichText(false);
            let _: () = msg_send![view, addSubview: &*sink];
            let _: bool = msg_send![native_window, makeFirstResponder: &*sink];
            let text_sink = sink.clone();
            let block = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
                let event_window: *mut AnyObject = msg_send![event, window];
                if event_window != native_window {
                    return event;
                }
                let kind: usize = msg_send![event, type];
                if matches!(kind, 10..=12) {
                    let code: u16 = msg_send![event, keyCode];
                    let key = physical_key(code);
                    let flags: usize = msg_send![event, modifierFlags];
                    let mut state = capture.borrow_mut();
                    let down = if kind == 12 {
                        let bit = match key {
                            Key::LeftShift | Key::RightShift => 17,
                            Key::LeftCtrl | Key::RightCtrl => 18,
                            Key::LeftAlt | Key::RightAlt => 19,
                            Key::LeftSuper | Key::RightSuper => 20,
                            _ => 0,
                        };
                        bit != 0 && flags & (1 << bit) != 0 && !state.keys.contains(&key)
                    } else {
                        kind == 10
                    };
                    if down {
                        state.keys.insert(key);
                    } else {
                        state.keys.remove(&key);
                    }
                    let mods = (((flags >> 17) & 1)
                        | (((flags >> 18) & 1) << 1)
                        | (((flags >> 19) & 1) << 2)
                        | (((flags >> 20) & 1) << 3)) as i32;
                    if mods & 8 != 0 && matches!(key, Key::Q | Key::H | Key::M | Key::W | Key::Tab)
                    {
                        return event; // AppKit handles application/window shortcuts.
                    }
                    state.push(Raw::Key(key, down, mods));
                    drop(state);
                    if kind == 10 && mods & (2 | 8) == 0 {
                        let _: () = msg_send![&*text_sink, keyDown: event];
                    }
                    // Browser editing shortcuts are handled in main, never by
                    // the text client (which would paste a second time).
                    return if kind == 12 {
                        event
                    } else {
                        std::ptr::null_mut()
                    };
                }
                let button = match kind {
                    1 | 2 => 1,
                    3 | 4 => 2,
                    25 | 26 => {
                        let number: isize = msg_send![event, buttonNumber];
                        if number != 2 {
                            return event;
                        }
                        3
                    }
                    _ => 0,
                };
                let bounds: NSRect = msg_send![view, bounds];
                let point: NSPoint = msg_send![event, locationInWindow];
                let point: NSPoint =
                    msg_send![view, convertPoint: point fromView: std::ptr::null::<AnyObject>()];
                let flipped: bool = msg_send![view, isFlipped];
                let yy = if flipped {
                    point.y
                } else {
                    bounds.size.height - point.y
                };
                let scale: f64 = msg_send![native_window, backingScaleFactor];
                let viewport = Viewport::new(
                    bounds.size.width as usize,
                    bounds.size.height as usize,
                    scale,
                );
                let Some(position) = canvas_position(viewport, (point.x as f32, yy as f32)) else {
                    return event;
                };
                let flags: usize = msg_send![event, modifierFlags];
                let mods = ((flags >> 17) & 1)
                    | (((flags >> 18) & 1) << 1)
                    | (((flags >> 19) & 1) << 2)
                    | (((flags >> 20) & 1) << 3);
                let event_kind = match kind {
                    1 | 3 | 25 => 2,
                    2 | 4 | 26 => 3,
                    _ => 1,
                };
                let mut state = capture.borrow_mut();
                state.push(Raw::Mouse(event_kind, position, button, mods as i32));
                event
            });
            let mask: usize = [1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 25, 26, 27]
                .iter()
                .fold(0, |mask, bit| mask | (1 << bit));
            let token: *mut AnyObject = msg_send![class!(NSEvent), addLocalMonitorForEventsMatchingMask: mask handler: &*block];
            Ok(Self {
                token: Retained::retain(token)
                    .ok_or_else(|| anyhow::anyhow!("could not monitor input"))?,
                sink,
            })
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![class!(NSEvent), removeMonitor: &*self.token];
            let _: () = msg_send![&*self.sink, removeFromSuperview];
        }
    }
}

fn physical_key(code: u16) -> Key {
    use Key::*;
    match code {
        0 => A,
        1 => S,
        2 => D,
        3 => F,
        4 => H,
        5 => G,
        6 => Z,
        7 => X,
        8 => C,
        9 => V,
        11 => B,
        12 => Q,
        13 => W,
        14 => E,
        15 => R,
        16 => Y,
        17 => T,
        18 => Key1,
        19 => Key2,
        20 => Key3,
        21 => Key4,
        22 => Key6,
        23 => Key5,
        24 => Equal,
        25 => Key9,
        26 => Key7,
        27 => Minus,
        28 => Key8,
        29 => Key0,
        30 => RightBracket,
        31 => O,
        32 => U,
        33 => LeftBracket,
        34 => I,
        35 => P,
        36 => Enter,
        37 => L,
        38 => J,
        39 => Apostrophe,
        40 => K,
        41 => Semicolon,
        42 => Backslash,
        43 => Comma,
        44 => Slash,
        45 => N,
        46 => M,
        47 => Period,
        48 => Tab,
        49 => Space,
        50 => Backquote,
        51 => Backspace,
        53 => Escape,
        54 => RightSuper,
        55 => LeftSuper,
        56 => LeftShift,
        57 => CapsLock,
        58 => LeftAlt,
        59 => LeftCtrl,
        60 => RightShift,
        61 => RightAlt,
        62 => RightCtrl,
        65 => NumPadDot,
        67 => NumPadAsterisk,
        69 => NumPadPlus,
        71 => NumLock,
        75 => NumPadSlash,
        76 => NumPadEnter,
        78 => NumPadMinus,
        81 => Equal,
        82 => NumPad0,
        83 => NumPad1,
        84 => NumPad2,
        85 => NumPad3,
        86 => NumPad4,
        87 => NumPad5,
        88 => NumPad6,
        89 => NumPad7,
        91 => NumPad8,
        92 => NumPad9,
        96 => F5,
        97 => F6,
        98 => F7,
        99 => F3,
        100 => F8,
        101 => F9,
        103 => F11,
        109 => F10,
        111 => F12,
        114 => Insert,
        115 => Home,
        116 => PageUp,
        117 => Delete,
        118 => F4,
        119 => End,
        120 => F2,
        121 => PageDown,
        122 => F1,
        123 => Left,
        124 => Right,
        125 => Down,
        126 => Up,
        _ => Unknown,
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
}
