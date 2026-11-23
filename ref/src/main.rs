mod browser;
mod chrome;
mod display;
mod framebuffer;
mod loader;
mod runtime;

use anyhow::{bail, Context, Result};
use browser::Browser;
use chrome::{AddressBar, Command, UiFont};
use minifb::{InputCallback, Key, ScaleMode, Window, WindowOptions};
use std::{cell::RefCell, env, rc::Rc};

#[derive(Debug, PartialEq)]
enum Input {
    Character(char),
    Key(Key, bool),
}

struct TextInput {
    events: Rc<RefCell<Vec<Input>>>,
    modifiers: [bool; 4],
}

impl InputCallback for TextInput {
    fn add_char(&mut self, value: u32) {
        if !self.modifiers.iter().any(|down| *down) {
            if let Some(ch) = char::from_u32(value).filter(|ch| !ch.is_control()) {
                self.events.borrow_mut().push(Input::Character(ch));
            }
        }
    }
    fn set_key_state(&mut self, key: Key, state: bool) {
        let index = match key {
            Key::LeftSuper => 0,
            Key::RightSuper => 1,
            Key::LeftCtrl => 2,
            Key::RightCtrl => 3,
            _ => 4,
        };
        if index < 4 {
            self.modifiers[index] = state;
        }
        if state {
            self.events
                .borrow_mut()
                .push(Input::Key(key, self.modifiers.iter().any(|down| *down)));
        }
    }
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let address = args.next();
    if address.as_deref() == Some("--help") {
        println!("Usage: smolbrowse [path/to/page.wasm | https://host/page.wasm]\n\nNo argument opens an empty viewer.\nPress : for commands, or Cmd+L to open an address. Enter opens. Cmd+R reloads.\nLocal WAT source and file:// URLs are also supported.");
        return Ok(());
    }
    if args.next().is_some() {
        bail!("Expected at most one address; see --help.");
    }
    let mut browser = Browser::new();
    let mut bar = AddressBar::default();
    if let Some(address) = address {
        bar.set(&address);
        browser.navigate(address);
    }
    let mut font = UiFont::load()?;
    let mut window = Window::new(
        "smolbrowse",
        960,
        720,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::Stretch,
            ..WindowOptions::default()
        },
    )
    .context("Opening macOS window")?;
    window.set_target_fps(60);
    let events = Rc::new(RefCell::new(Vec::new()));
    window.set_input_callback(Box::new(TextInput {
        events: events.clone(),
        modifiers: [false; 4],
    }));
    let mut clipboard = arboard::Clipboard::new().ok();
    let mut title_address = String::new();
    let mut quit = false;
    while window.is_open() && !quit {
        browser.poll();
        let mut open = false;
        let mut reload = false;
        // Capture key events and their modifiers together, including presses that
        // begin and end between two display frames.
        for event in std::mem::take(&mut *events.borrow_mut()) {
            let (key, command) = match event {
                Input::Character(ch) => {
                    if bar.focused {
                        bar.insert(&ch.to_string());
                    } else if ch == ':' {
                        bar.set("");
                        bar.focused = true;
                        browser.status.clear();
                    } else if ch == 'q' {
                        quit = true;
                    }
                    continue;
                }
                Input::Key(key, command) => (key, command),
            };
            match key {
                Key::L if command => {
                    bar.set(&browser.current_address);
                    bar.select_all();
                    browser.status.clear();
                }
                Key::Escape => {
                    bar.focused = false;
                    bar.set(&browser.current_address);
                    browser.status.clear();
                }
                Key::R if command || !bar.focused => reload = true,
                Key::F5 => reload = true,
                _ if !bar.focused => {}
                Key::A if command => bar.select_all(),
                Key::V if command => {
                    match clipboard.as_mut().map(|clipboard| clipboard.get_text()) {
                        Some(Ok(text)) => bar.insert(&text),
                        _ => {
                            browser.status =
                                "Could not read clipboard. Type a path or URL instead.".into()
                        }
                    }
                }
                Key::C if command => {
                    if let Some(clipboard) = &mut clipboard {
                        let _ = clipboard.set_text(bar.value());
                    }
                }
                Key::Backspace => bar.backspace(),
                Key::Delete => bar.delete(),
                Key::Left => bar.move_cursor(-1),
                Key::Right => bar.move_cursor(1),
                Key::Home => bar.home(),
                Key::End => bar.end(),
                Key::Enter | Key::NumPadEnter => open = true,
                _ => {}
            }
        }
        if open {
            bar.focused = false;
            match chrome::parse_command(&bar.value()) {
                Ok(Command::Open(address)) => {
                    bar.set(&address);
                    browser.navigate(address);
                }
                Ok(Command::Reload) => reload = true,
                Ok(Command::Quit) => quit = true,
                Ok(Command::Cancel) => browser.status.clear(),
                Err(error) => browser.status = error.to_string(),
            }
        }
        if reload {
            bar.focused = false;
            if !browser.current_address.is_empty() {
                let address = browser.current_address.clone();
                bar.set(&address);
                browser.navigate(address);
            }
        }
        if browser.current_address != title_address {
            title_address = browser.current_address.clone();
            window.set_title(&format!("smolbrowse — {title_address}"));
        }
        let viewport = display::viewport(&window);
        if viewport.width > 0 && viewport.height > 0 {
            let pixels = chrome::compose(
                viewport,
                &bar,
                &browser.status,
                browser.site.as_ref().map(|site| site.frame()),
                &mut font,
            );
            window.update_with_buffer(&pixels, viewport.width, viewport.height)?;
        }
        browser.advance();
    }
    Ok(())
}

#[cfg(test)]
mod input_tests {
    use super::*;
    #[test]
    fn keeps_shortcuts_when_modifiers_are_released_before_the_next_frame() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut input = TextInput {
            events: events.clone(),
            modifiers: [false; 4],
        };
        input.set_key_state(Key::LeftSuper, true);
        input.set_key_state(Key::L, true);
        input.add_char('l' as u32);
        input.set_key_state(Key::L, false);
        input.set_key_state(Key::LeftSuper, false);
        input.set_key_state(Key::Enter, true);
        assert!(events.borrow().contains(&Input::Key(Key::L, true)));
        assert!(events.borrow().contains(&Input::Key(Key::Enter, false)));
        assert!(!events.borrow().contains(&Input::Character('l')));
        input.add_char('é' as u32);
        assert_eq!(events.borrow().last(), Some(&Input::Character('é')));
    }
}
