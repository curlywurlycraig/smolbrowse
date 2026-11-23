//! Translation from native input to the small, platform-independent site ABI.
use crate::display::Viewport;
use minifb::Key;
use std::collections::HashSet;
#[cfg(any(not(target_os = "macos"), test))]
use {
    minifb::InputCallback,
    std::{cell::RefCell, rc::Rc},
};

pub const MAX_EVENTS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub kind: i32,
    fields: [i32; 6],
}
impl Event {
    pub fn new(kind: i32, fields: [i32; 6]) -> Self {
        Self { kind, fields }
    }
    pub fn reset() -> Self {
        Self::new(7, [0; 6])
    }
    pub fn get(self, field: i32) -> i32 {
        field
            .checked_sub(1)
            .and_then(|i| self.fields.get(i as usize))
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Raw {
    Key(Key, bool, i32),
    Text(char, i32),
    Mouse(i32, (i32, i32), i32, i32),
}

#[derive(Default)]
pub struct Capture {
    pub events: Vec<Raw>,
    pub keys: HashSet<Key>,
    pub overflow: bool,
    #[cfg(any(not(target_os = "macos"), test))]
    surrogate: Option<u16>,
}
impl Capture {
    pub fn modifiers(&self) -> i32 {
        [
            (Key::LeftShift, Key::RightShift),
            (Key::LeftCtrl, Key::RightCtrl),
            (Key::LeftAlt, Key::RightAlt),
            (Key::LeftSuper, Key::RightSuper),
        ]
        .iter()
        .enumerate()
        .fold(0, |bits, (i, (a, b))| {
            bits | if self.keys.contains(a) || self.keys.contains(b) {
                1 << i
            } else {
                0
            }
        })
    }
    pub fn push(&mut self, event: Raw) {
        if self.events.len() < MAX_EVENTS {
            self.events.push(event);
        } else {
            self.overflow = true;
        }
    }
}
#[cfg(any(not(target_os = "macos"), test))]
pub struct Callback(pub Rc<RefCell<Capture>>);
#[cfg(any(not(target_os = "macos"), test))]
impl InputCallback for Callback {
    fn add_char(&mut self, value: u32) {
        let mut state = self.0.borrow_mut();
        let modifiers = state.modifiers();
        if (0xd800..=0xdbff).contains(&value) {
            state.surrogate = Some(value as u16);
            return;
        }
        let value = if let Some(high) = state.surrogate.take() {
            if (0xdc00..=0xdfff).contains(&value) {
                0x10000 + ((u32::from(high) - 0xd800) << 10) + value - 0xdc00
            } else {
                value
            }
        } else {
            value
        };
        if modifiers & (2 | 8) == 0 {
            if let Some(ch) = char::from_u32(value)
                .filter(|ch| !ch.is_control() && !(0xf700..=0xf747).contains(&(*ch as u32)))
            {
                state.push(Raw::Text(ch, modifiers));
            }
        }
    }
    fn set_key_state(&mut self, key: Key, down: bool) {
        let mut state = self.0.borrow_mut();
        if down {
            state.keys.insert(key);
        } else {
            state.keys.remove(&key);
        }
        let modifiers = state.modifiers();
        state.push(Raw::Key(key, down, modifiers));
    }
}

pub fn key_code(key: Key) -> Option<i32> {
    use Key::*;
    Some(match key {
        k if (Key0 as u8..=Key9 as u8).contains(&(k as u8)) => 48 + key as i32 - Key0 as i32,
        k if (A as u8..=Z as u8).contains(&(k as u8)) => 65 + key as i32 - A as i32,
        k if (NumPad0 as u8..=NumPad9 as u8).contains(&(k as u8)) => {
            48 + key as i32 - NumPad0 as i32
        }
        Space => 32,
        Apostrophe => 39,
        Backquote => 96,
        Backslash => 92,
        Comma => 44,
        Equal => 61,
        LeftBracket => 91,
        Minus | NumPadMinus => 45,
        Period | NumPadDot => 46,
        RightBracket => 93,
        Semicolon => 59,
        Slash | NumPadSlash => 47,
        Enter | NumPadEnter => 256,
        Escape => 257,
        Backspace => 258,
        Tab => 259,
        Left => 260,
        Right => 261,
        Up => 262,
        Down => 263,
        Delete => 264,
        Home => 265,
        End => 266,
        PageUp => 267,
        PageDown => 268,
        LeftShift | RightShift => 269,
        LeftCtrl | RightCtrl => 270,
        LeftAlt | RightAlt => 271,
        LeftSuper | RightSuper => 272,
        _ => return None,
    })
}

#[derive(Default)]
pub struct Router {
    held: HashSet<Key>,
    blocked: HashSet<Key>,
    buttons: [bool; 3],
    blocked_buttons: [bool; 3],
    #[cfg(any(not(target_os = "macos"), test))]
    position: Option<(i32, i32)>,
}
impl Router {
    pub fn reset(&mut self, keys: &HashSet<Key>, buttons: [bool; 3]) {
        self.held.clear();
        self.blocked = keys.clone();
        self.buttons = [false; 3];
        self.blocked_buttons = buttons;
        #[cfg(any(not(target_os = "macos"), test))]
        {
            self.position = None;
        }
    }
    pub fn raw(&mut self, raw: Raw) -> Option<Event> {
        match raw {
            Raw::Mouse(kind, position, button, modifiers) => {
                self.mouse_event(kind, position, button, modifiers)
            }
            Raw::Text(ch, modifiers) => {
                if self
                    .blocked
                    .iter()
                    .any(|k| key_code(*k).is_some_and(|c| c < 256))
                {
                    return None;
                }
                Some(Event::new(6, [0, 0, 0, 0, ch as i32, modifiers]))
            }
            Raw::Key(key, down, modifiers) => {
                if self.blocked.contains(&key) {
                    if !down {
                        self.blocked.remove(&key);
                    }
                    return None;
                }
                let code = key_code(key)?;
                let was_down = self.held.iter().any(|k| key_code(*k) == Some(code));
                if down {
                    self.held.insert(key);
                } else {
                    self.held.remove(&key);
                }
                let now_down = self.held.iter().any(|k| key_code(*k) == Some(code));
                (was_down != now_down).then(|| {
                    Event::new(if now_down { 4 } else { 5 }, [0, 0, 0, code, 0, modifiers])
                })
            }
        }
    }
    pub fn mouse_event(
        &mut self,
        kind: i32,
        (x, y): (i32, i32),
        button: i32,
        modifiers: i32,
    ) -> Option<Event> {
        let inside = (0..320).contains(&x) && (0..240).contains(&y);
        if kind == 1 {
            return (inside || self.capturing()).then(|| Event::new(1, [x, y, 0, 0, 0, modifiers]));
        }
        let i = usize::try_from(button.checked_sub(1)?).ok()?;
        if i >= 3 {
            return None;
        }
        if self.blocked_buttons[i] {
            if kind == 3 {
                self.blocked_buttons[i] = false;
            }
            return None;
        }
        if kind == 2 && !self.buttons[i] {
            if !inside {
                self.blocked_buttons[i] = true;
                return None;
            }
            self.buttons[i] = true;
        } else if kind == 3 && self.buttons[i] {
            self.buttons[i] = false;
        } else {
            return None;
        }
        Some(Event::new(kind, [x, y, button, 0, 0, modifiers]))
    }
    #[cfg(any(not(target_os = "macos"), test))]
    pub fn mouse(
        &mut self,
        #[cfg(any(not(target_os = "macos"), test))] position: Option<(i32, i32)>,
        down: [bool; 3],
        modifiers: i32,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let Some((x, y)) = position else {
            return events;
        };
        let inside = (0..320).contains(&x) && (0..240).contains(&y);
        if self.position != position && (inside || self.buttons.iter().any(|b| *b)) {
            events.push(Event::new(1, [x, y, 0, 0, 0, modifiers]));
        }
        self.position = position;
        for (i, pressed) in down.iter().copied().enumerate() {
            if self.blocked_buttons[i] {
                self.blocked_buttons[i] = pressed;
                continue;
            }
            if pressed && !self.buttons[i] && inside {
                self.buttons[i] = true;
                events.push(Event::new(2, [x, y, i as i32 + 1, 0, 0, modifiers]));
            } else if !pressed && self.buttons[i] {
                self.buttons[i] = false;
                events.push(Event::new(3, [x, y, i as i32 + 1, 0, 0, modifiers]));
            } else if pressed && !self.buttons[i] {
                // A press starting in letterboxing must not become a press on entry.
                self.blocked_buttons[i] = true;
            }
        }
        events
    }
    pub fn capturing(&self) -> bool {
        self.buttons.iter().any(|b| *b)
    }
}

pub fn canvas_position(viewport: Viewport, position: (f32, f32)) -> Option<(i32, i32)> {
    let scale = (viewport.width as f64 / 320.0).min(viewport.height as f64 / 240.0);
    let width = (320.0 * scale) as usize;
    let height = (240.0 * scale) as usize;
    if width == 0 || height == 0 {
        return None;
    }
    let left = (viewport.width - width) / 2;
    let top = (viewport.height - height) / 2;
    Some((
        ((f64::from(position.0) * viewport.scale - left as f64) * 320.0 / width as f64).floor()
            as i32,
        ((f64::from(position.1) * viewport.scale - top as f64) * 240.0 / height as f64).floor()
            as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeats_equivalent_keys_and_focus_reset() {
        let mut r = Router::default();
        let key = |k, down| Raw::Key(k, down, 0);
        assert_eq!(r.raw(key(Key::LeftShift, true)).unwrap().get(4), 269);
        assert!(r.raw(key(Key::RightShift, true)).is_none());
        assert!(r.raw(key(Key::LeftShift, false)).is_none());
        assert_eq!(r.raw(key(Key::RightShift, false)).unwrap().kind, 5);
        assert!(r.raw(key(Key::RightShift, false)).is_none());
        assert!(r.raw(key(Key::A, true)).is_some());
        assert!(r.raw(key(Key::A, true)).is_none());
        r.reset(&HashSet::from([Key::A]), [false; 3]);
        assert!(r.raw(Raw::Text('a', 0)).is_none());
        assert!(r.raw(key(Key::A, false)).is_none());
        assert!(r.raw(key(Key::A, true)).is_some());
        assert_eq!(r.raw(Raw::Text('é', 0)).unwrap().get(5), 233);
    }
    #[test]
    fn native_mouse_preserves_quick_clicks_and_cancels_capture_on_reset() {
        let mut r = Router::default();
        let down = r.raw(Raw::Mouse(2, (12, 34), 1, 1)).unwrap();
        let up = r.raw(Raw::Mouse(3, (-5, 240), 1, 0)).unwrap();
        assert_eq!((down.kind, up.kind), (2, 3));
        assert_eq!(down.get(6), 1);
        assert_eq!(up.get(1), -5);
        r.raw(Raw::Mouse(2, (12, 34), 1, 0));
        r.reset(&HashSet::new(), [true, false, false]);
        assert!(r.raw(Raw::Mouse(3, (12, 34), 1, 0)).is_none());
        assert!(r.raw(Raw::Mouse(2, (12, 34), 1, 0)).is_some());
    }

    #[test]
    fn pointer_capture_letterboxing_and_retina() {
        let p = canvas_position(Viewport::new(1000, 720, 2.0), (20.0, 0.0));
        assert_eq!(p, Some((0, 0)));
        assert_eq!(
            canvas_position(Viewport::new(1000, 720, 1.0), (19.0, 0.0)),
            Some((-1, 0))
        );
        let mut r = Router::default();
        r.mouse(Some((10, 20)), [true, false, false], 0);
        let events = r.mouse(Some((-10, 250)), [false; 3], 0);
        assert_eq!(events.last().unwrap().kind, 3);
        assert_eq!(events.last().unwrap().get(1), -10);
        r.mouse(Some((-1, 0)), [true, false, false], 0);
        assert!(r
            .mouse(Some((10, 20)), [true, false, false], 0)
            .iter()
            .all(|e| e.kind != 2));
    }
    #[test]
    fn callbacks_keep_both_text_and_transitions_and_bound_queue() {
        let state = Rc::new(RefCell::new(Capture::default()));
        let mut callback = Callback(state.clone());
        callback.set_key_state(Key::A, true);
        callback.add_char('a' as u32);
        callback.set_key_state(Key::A, false);
        assert_eq!(state.borrow().events.len(), 3);
        for _ in 0..MAX_EVENTS {
            callback.add_char('a' as u32);
        }
        assert!(state.borrow().overflow);
        assert_eq!(state.borrow().events.len(), MAX_EVENTS);
    }
}
