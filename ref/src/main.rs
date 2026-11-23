mod browser;
mod chrome;
mod display;
mod framebuffer;
mod input;
mod loader;
#[cfg(target_os = "macos")]
mod native_input;
mod runtime;

use anyhow::{bail, Context, Result};
use browser::Browser;
use chrome::{AddressBar, Command, UiFont};
#[cfg(not(target_os = "macos"))]
use input::Callback;
use input::{Capture, Event, Raw, Router};
use minifb::{Key, ScaleMode, Window, WindowOptions};
#[cfg(not(target_os = "macos"))]
use minifb::{MouseButton, MouseMode};
use std::{cell::RefCell, env, rc::Rc};

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
    let capture = Rc::new(RefCell::new(Capture::default()));
    #[cfg(not(target_os = "macos"))]
    window.set_input_callback(Box::new(Callback(capture.clone())));
    #[cfg(target_os = "macos")]
    let native_input = native_input::Monitor::new(&window, capture.clone())?;
    let mut router = Router::default();
    let mut focused = false;
    let mut generation = 0;
    let mut clipboard = arboard::Clipboard::new().ok();
    let mut title_address = String::new();
    let mut quit = false;
    while window.is_open() && !quit {
        browser.poll();
        let mut open = false;
        let mut reload = false;
        #[cfg(target_os = "macos")]
        let buttons = native_input.buttons();
        #[cfg(not(target_os = "macos"))]
        let buttons = [MouseButton::Left, MouseButton::Right, MouseButton::Middle]
            .map(|b| window.get_mouse_down(b));
        #[cfg(target_os = "macos")]
        let active = native_input.focused();
        #[cfg(not(target_os = "macos"))]
        let active = window.is_active();
        let mut site_events = Vec::new();
        let mut state = capture.borrow_mut();
        let mut pending = std::mem::take(&mut state.events);
        let overflow = std::mem::take(&mut state.overflow);
        #[cfg(not(target_os = "macos"))]
        let modifiers = state.modifiers();
        #[cfg(target_os = "macos")]
        if generation != browser.generation || focused != (active && !bar.focused) {
            state.keys = native_input.pressed_keys();
        }
        if generation != browser.generation {
            generation = browser.generation;
            pending.clear();
            #[cfg(target_os = "macos")]
            native_input.reset_text();
            router.reset(&state.keys, buttons);
        }
        let wants_focus = active && !bar.focused;
        if focused != wants_focus {
            #[cfg(target_os = "macos")]
            native_input.reset_text();
            router.reset(&state.keys, buttons);
            if focused {
                site_events.push(Event::reset());
            }
            focused = wants_focus;
        }
        drop(state);
        for raw in pending {
            if !active {
                continue;
            }
            let (key, command) = match raw {
                Raw::Mouse(..) => {
                    if focused && !bar.focused {
                        if let Some(event) = router.raw(raw) {
                            site_events.push(event);
                        }
                    }
                    continue;
                }
                Raw::Text(ch, _) => {
                    if bar.focused {
                        bar.insert(&ch.to_string());
                    } else if !browser.interactive() && ch == ':' {
                        bar.set("");
                        bar.focused = true;
                        browser.status.clear();
                    } else if !browser.interactive() && ch == 'q' {
                        quit = true;
                    } else if focused {
                        if let Some(event) = router.raw(raw) {
                            site_events.push(event);
                        }
                    }
                    continue;
                }
                Raw::Key(key, down, mods) => {
                    let command = mods & (2 | 8) != 0;
                    let reserved = (command && matches!(key, Key::L | Key::R)) || key == Key::F5;
                    if focused && !bar.focused && !reserved && browser.interactive() {
                        if let Some(event) = router.raw(raw) {
                            site_events.push(event);
                        }
                        continue;
                    }
                    if !down {
                        continue;
                    }
                    (key, command)
                }
            };
            match key {
                Key::L if command => {
                    bar.set(&browser.current_address);
                    bar.select_all();
                    browser.status.clear();
                    #[cfg(target_os = "macos")]
                    native_input.reset_text();
                    site_events.clear();
                    site_events.push(Event::reset());
                    router.reset(&capture.borrow().keys, buttons);
                    focused = false;
                }
                Key::Escape => {
                    bar.focused = false;
                    bar.set(&browser.current_address);
                    browser.status.clear();
                }
                Key::R if command || (!bar.focused && !browser.interactive()) => reload = true,
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
        let wants_focus = active && !bar.focused;
        if focused != wants_focus {
            #[cfg(target_os = "macos")]
            native_input.reset_text();
            router.reset(&capture.borrow().keys, buttons);
            if focused {
                site_events.clear();
                site_events.push(Event::reset());
            }
            focused = wants_focus;
        }
        let viewport = display::viewport(&window);
        #[cfg(not(target_os = "macos"))]
        if focused && browser.interactive() {
            let position = window
                .get_mouse_pos(MouseMode::Pass)
                .and_then(|p| input::canvas_position(viewport, p));
            if position.is_none() && router.capturing() {
                site_events.clear();
                site_events.push(Event::reset());
                router.reset(&capture.borrow().keys, buttons);
            } else {
                site_events.extend(router.mouse(position, buttons, modifiers));
            }
        }
        browser.input(&site_events, overflow);
        if browser.current_address != title_address {
            title_address = browser.current_address.clone();
            window.set_title(&format!("smolbrowse — {title_address}"));
        }
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
