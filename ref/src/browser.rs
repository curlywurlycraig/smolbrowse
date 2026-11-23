use crate::{input::Event, loader, runtime::Site};
use anyhow::Result;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

struct Loaded {
    id: u64,
    address: String,
    result: Result<Vec<u8>>,
}

pub struct Browser {
    pub site: Option<Site>,
    pub current_address: String,
    pub status: String,
    pub generation: u64,
    animating: bool,
    request: u64,
    sender: Sender<Loaded>,
    receiver: Receiver<Loaded>,
}

impl Browser {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            site: None,
            current_address: String::new(),
            status: String::new(),
            generation: 0,
            animating: false,
            request: 0,
            sender,
            receiver,
        }
    }

    pub fn navigate(&mut self, address: String) {
        self.request += 1;
        let id = self.request;
        let sender = self.sender.clone();
        self.status = format!("Loading {address} ...");
        thread::spawn(move || {
            let result = loader::load(&address);
            let _ = sender.send(Loaded {
                id,
                address,
                result,
            });
        });
    }

    fn accept(&mut self, loaded: Loaded) {
        // A slower previous download must never replace a newer navigation.
        if loaded.id != self.request {
            return;
        }
        match loaded.result.and_then(|bytes| Site::new(&bytes)) {
            Ok(site) => {
                self.site = Some(site);
                self.generation += 1;
                self.animating = true;
                self.current_address = loaded.address;
                self.status.clear();
            }
            Err(error) => {
                self.status = format!("Load failed: {error:#}");
                eprintln!("{}", self.status);
            }
        }
    }

    pub fn poll(&mut self) {
        while let Ok(loaded) = self.receiver.try_recv() {
            self.accept(loaded);
        }
    }

    pub fn interactive(&self) -> bool {
        self.site.as_ref().is_some_and(Site::interactive)
    }

    pub fn input(&mut self, events: &[Event], overflow: bool) {
        if !self.animating || !self.interactive() {
            return;
        }
        let result = if overflow {
            Err(anyhow::anyhow!("input queue exceeds 4096 events"))
        } else {
            self.site.as_mut().unwrap().dispatch(events)
        };
        if let Err(error) = result {
            self.animating = false;
            self.status = format!("Input stopped: {error:#}. Reload to restart.");
            eprintln!("{}", self.status);
        }
    }

    pub fn advance(&mut self) {
        if self.animating {
            if let Some(site) = &mut self.site {
                if let Err(error) = site.advance() {
                    self.animating = false;
                    self.status = format!("Render stopped: {error:#}. Reload to restart.");
                    eprintln!("{}", self.status);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_failure_stops_rendering_and_preserves_last_frame() {
        let bytes = wat::parse_str(
            r#"(module
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (func (export "event") (param i32) unreachable)
          (func (export "render") (call $p (i32.const 0) (i32.const 0) (i32.const 42))))"#,
        )
        .unwrap();
        let mut browser = Browser::new();
        browser.accept(Loaded {
            id: 0,
            address: "input.wasm".into(),
            result: Ok(bytes),
        });
        browser.input(&[Event::reset()], false);
        assert!(!browser.animating);
        assert!(browser.status.starts_with("Input stopped:"));
        browser.advance();
        assert_eq!(browser.site.as_ref().unwrap().frame().pixels[0], 42);
    }

    #[test]
    fn navigation_commits_only_latest_success_and_preserves_page_on_error() {
        let mut browser = Browser::new();
        let bytes = wat::parse_str(
            r#"(module
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (func (export "render") (call $p (i32.const 0) (i32.const 0) (i32.const 42))))"#,
        )
        .unwrap();
        browser.request = 1;
        browser.accept(Loaded {
            id: 1,
            address: "first.wasm".into(),
            result: Ok(bytes.clone()),
        });
        assert_eq!(browser.site.as_ref().unwrap().frame().pixels[0], 42);
        browser.request = 3;
        browser.accept(Loaded {
            id: 2,
            address: "stale.wasm".into(),
            result: Ok(bytes),
        });
        browser.accept(Loaded {
            id: 3,
            address: "bad.wasm".into(),
            result: Ok(vec![0]),
        });
        assert_eq!(browser.current_address, "first.wasm");
        assert!(browser.status.starts_with("Load failed:"));
        assert_eq!(browser.site.as_ref().unwrap().frame().pixels[0], 42);
        assert!(browser.animating);
    }
}
