use anyhow::{bail, Context, Result};
use wasmi::{
    Caller, Config, Engine, Error as WasmError, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder, TypedFunc,
};

use crate::{framebuffer::Framebuffer, input::Event};

pub const MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const FUEL: u64 = 10_000_000;

struct Page {
    frame: Framebuffer,
    limits: StoreLimits,
    event: Option<Event>,
}

fn charge(caller: &mut Caller<'_, Page>, cost: u64) -> Result<(), WasmError> {
    let fuel = caller
        .get_fuel()
        .map_err(|e| WasmError::new(e.to_string()))?;
    let remaining = fuel
        .checked_sub(cost)
        .ok_or_else(|| WasmError::new("drawing fuel exhausted"))?;
    caller
        .set_fuel(remaining)
        .map_err(|e| WasmError::new(e.to_string()))
}

pub struct Site {
    store: Store<Page>,
    render: TypedFunc<(), ()>,
    event: Option<TypedFunc<i32, ()>>,
    committed: Framebuffer,
}

impl Site {
    pub fn frame(&self) -> &Framebuffer {
        &self.committed
    }

    pub fn interactive(&self) -> bool {
        self.event.is_some()
    }

    pub fn dispatch(&mut self, events: &[Event]) -> Result<()> {
        let Some(handler) = self.event else {
            return Ok(());
        };
        // One shared budget per batch, so flooding input cannot multiply fuel.
        self.store.set_fuel(FUEL)?;
        for event in events {
            self.store.data_mut().event = Some(*event);
            let result = handler.call(&mut self.store, event.kind);
            self.store.data_mut().event = None;
            result.context("page event failed")?;
        }
        Ok(())
    }

    pub fn advance(&mut self) -> Result<()> {
        self.store.set_fuel(FUEL)?;
        self.store.data_mut().frame.pixels.fill(0);
        self.render
            .call(&mut self.store, ())
            .context("page render failed")?;
        self.committed
            .pixels
            .copy_from_slice(&self.store.data().frame.pixels);
        Ok(())
    }

    pub fn new(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_MODULE_BYTES {
            bail!("page exceeds the 4 MiB module limit");
        }
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).context("invalid Wasm module")?;
        let mut store = Store::new(
            &engine,
            Page {
                frame: Framebuffer::default(),
                event: None,
                limits: StoreLimitsBuilder::new()
                    .memory_size(16 * 1024 * 1024)
                    .table_elements(4096)
                    .memories(1)
                    .tables(1)
                    .instances(1)
                    .build(),
            },
        );
        store.limiter(|page| &mut page.limits);
        store.set_fuel(FUEL)?;
        let mut linker = Linker::new(&engine);
        linker.func_wrap(
            "smolbrowse",
            "event_get",
            |mut caller: Caller<'_, Page>, field: i32| -> Result<i32, WasmError> {
                charge(&mut caller, 1)?;
                Ok(caller.data().event.map_or(0, |event| event.get(field)))
            },
        )?;
        linker.func_wrap(
            "smolbrowse",
            "pixel",
            |mut caller: Caller<'_, Page>, x: i32, y: i32, color: i32| -> Result<(), WasmError> {
                charge(&mut caller, 1)?;
                caller
                    .data_mut()
                    .frame
                    .pixel(i64::from(x), i64::from(y), color as u32);
                Ok(())
            },
        )?;
        linker.func_wrap(
            "smolbrowse",
            "text",
            |mut caller: Caller<'_, Page>,
             x: i32,
             y: i32,
             ptr: i32,
             len: i32,
             color: i32|
             -> Result<(), WasmError> {
                let len = len as u32 as usize;
                if len > MAX_TEXT_BYTES {
                    return Err(WasmError::new("text exceeds 64 KiB"));
                }
                charge(&mut caller, 1 + len as u64 * 64)?;
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| WasmError::new("text requires exported memory"))?;
                let start = ptr as u32 as usize;
                let end = start
                    .checked_add(len)
                    .ok_or_else(|| WasmError::new("text range overflow"))?;
                let bytes = memory
                    .data(&caller)
                    .get(start..end)
                    .ok_or_else(|| WasmError::new("text range outside memory"))?;
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| WasmError::new("text is not valid UTF-8"))?
                    .to_owned();
                caller.data_mut().frame.text(x, y, &text, color as u32);
                Ok(())
            },
        )?;
        let instance = linker
            .instantiate(&mut store, &module)
            .context("could not instantiate page")?
            .start(&mut store)
            .context("page start function failed")?;
        let render = instance
            .get_typed_func::<(), ()>(&store, "render")
            .context("page must export render: () -> ()")?;
        let event = if instance.get_export(&store, "event").is_some() {
            Some(
                instance
                    .get_typed_func::<i32, ()>(&store, "event")
                    .context("event must have signature (i32) -> ()")?,
            )
        } else {
            None
        };
        let mut site = Self {
            store,
            render,
            event,
            committed: Framebuffer::default(),
        };
        site.advance()?;
        Ok(site)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(source: &str) -> Result<Framebuffer> {
        Site::new(&wat::parse_str(source).unwrap()).map(|site| site.committed)
    }

    #[test]
    fn frames_keep_state_clear_canvas_and_commit_only_on_success() {
        let bytes = wat::parse_str(
            r#"(module
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (global $n (mut i32) (i32.const 0))
          (func (export "render")
            (call $p (global.get $n) (i32.const 0) (i32.const 42))
            (if (i32.eq (global.get $n) (i32.const 2)) (then unreachable))
            (global.set $n (i32.add (global.get $n) (i32.const 1)))))"#,
        )
        .unwrap();
        let mut site = Site::new(&bytes).unwrap();
        assert_eq!(site.frame().pixels[0], 42);
        site.advance().unwrap();
        assert_eq!(site.frame().pixels[0], 0);
        assert_eq!(site.frame().pixels[1], 42);
        assert!(site.advance().is_err());
        assert_eq!(site.frame().pixels[1], 42);
        assert_eq!(site.frame().pixels[2], 0);
        // Reload starts the module's globals over.
        assert_eq!(Site::new(&bytes).unwrap().frame().pixels[0], 42);
    }

    #[test]
    fn pixel_only_page_needs_no_memory_and_start_is_cleared() {
        let frame = page(
            r#"(module
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (func $start (call $p (i32.const 0) (i32.const 0) (i32.const 1)))
          (start $start)
          (func (export "render")
            (call $p (i32.const 319) (i32.const 239) (i32.const -1))))"#,
        )
        .unwrap();
        assert_eq!(frame.pixels[0], 0);
        assert_eq!(frame.pixels[320 * 240 - 1], 0xffffff);
    }

    fn text_page(memory: &str, ptr: i32, len: i32) -> Result<Framebuffer> {
        page(&format!(
            r#"(module
          (import "smolbrowse" "text" (func $t (param i32 i32 i32 i32 i32)))
          {memory}
          (func (export "render")
            (call $t (i32.const 0) (i32.const 0)
              (i32.const {ptr}) (i32.const {len}) (i32.const 1))))"#
        ))
    }

    #[test]
    fn validates_text_memory_ranges_and_encoding() {
        let memory = r#"(memory (export "memory") 1) (data (i32.const 0) "A\ff")"#;
        assert!(text_page(memory, 0, 1).is_ok());
        assert!(text_page(memory, 65536, 0).is_ok());
        for (ptr, len) in [(0, 2), (-1, 1), (65536, 1), (65537, 0), (0, -1), (0, 65537)] {
            assert!(
                text_page(memory, ptr, len).is_err(),
                "pointer {ptr}, length {len}"
            );
        }
        assert!(text_page("", 0, 0).is_err());
    }

    #[test]
    fn rejects_incompatible_contracts_and_traps() {
        for source in [
            "(module)",
            r#"(module (func (export "render") (param i32)))"#,
            r#"(module (import "env" "unknown" (func)) (func (export "render")))"#,
            r#"(module (func (export "render") unreachable))"#,
        ] {
            assert!(page(source).is_err());
        }
        assert!(Site::new(b"invalid wasm").is_err());
        assert!(Site::new(&vec![0; MAX_MODULE_BYTES + 1]).is_err());
    }

    #[test]
    fn event_details_are_scoped_and_state_survives_until_render() {
        let bytes = wat::parse_str(
            r#"(module
          (import "smolbrowse" "event_get" (func $get (param i32) (result i32)))
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (global $x (mut i32) (i32.const 0))
          (func (export "event") (param $kind i32)
            (global.set $x (i32.add (local.get $kind) (call $get (i32.const 1))))
            (if (call $get (i32.const -1)) (then unreachable))
            (if (call $get (i32.const 99)) (then unreachable))
            (call $p (i32.const 1) (i32.const 0) (i32.const 123)))
          (func (export "render")
            (if (call $get (i32.const 1)) (then unreachable))
            (call $p (i32.const 0) (i32.const 0) (global.get $x))))"#,
        )
        .unwrap();
        let mut site = Site::new(&bytes).unwrap();
        assert!(site.interactive());
        site.dispatch(&[Event::new(2, [40, 60, 1, 0, 0, 0])])
            .unwrap();
        assert_eq!(site.frame().pixels[0], 0);
        site.advance().unwrap();
        assert_eq!(site.frame().pixels[0], 42);
        assert_eq!(site.frame().pixels[1], 0);
    }

    #[test]
    fn validates_event_export_and_limits_handlers() {
        for export in [
            r#"(func (export "event"))"#,
            r#"(global (export "event") i32 (i32.const 0))"#,
        ] {
            assert!(page(&format!(r#"(module {export} (func (export "render")))"#)).is_err());
        }
        let bytes = wat::parse_str(
            r#"(module
          (func (export "event") (param i32) (loop $l (br $l)))
          (func (export "render")))"#,
        )
        .unwrap();
        let mut site = Site::new(&bytes).unwrap();
        assert!(site.dispatch(&[Event::reset()]).is_err());
        assert!(site.store.data().event.is_none());
    }

    #[test]
    fn puzzle_runs_in_reference_runtime_within_frame_and_input_limits() {
        let mut site = Site::new(include_bytes!("../../examples/tip/build/tip.wasm")).unwrap();
        assert!(site.interactive());
        // Optional actual-font framebuffer capture for visual inspection.
        if let Ok(path) = std::env::var("SMOLBROWSE_PREVIEW") {
            let mut ppm = b"P6\n320 240\n255\n".to_vec();
            for pixel in &site.frame().pixels {
                ppm.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]);
            }
            std::fs::write(path, ppm).unwrap();
        }
        site.dispatch(&[Event::new(4, [0, 0, 0, 261, 0, 0])])
            .unwrap();
        for _ in 0..20 {
            site.advance().unwrap();
        }
        site.dispatch(&[Event::new(4, [0, 0, 0, 90, 0, 0]), Event::reset()])
            .unwrap();
        site.advance().unwrap();
        // Solve the first board, including the sink and complete particle burst.
        // This exercises the busiest new frames under the real host fuel budget.
        for key in [261, 261, 261, 263, 263, 261, 263, 263] {
            site.dispatch(&[Event::new(4, [0, 0, 0, key, 0, 0])])
                .unwrap();
            for _ in 0..14 {
                site.advance().unwrap();
            }
        }
        for _ in 0..80 {
            site.advance().unwrap();
        }
        assert_eq!(site.frame().pixels[166 * 320 + 160], 0xc18b45);
        // The existing v1 example remains compatible and noninteractive.
        let old = Site::new(include_bytes!("../../examples/cube/build/cube.wasm")).unwrap();
        assert!(!old.interactive());
    }

    #[test]
    fn limits_execution_and_memory() {
        assert!(page(r#"(module (func (export "render") (loop $l (br $l))))"#).is_err());
        assert!(page(r#"(module (memory 257) (func (export "render")))"#).is_err());
        let frame = page(
            r#"(module
          (import "smolbrowse" "pixel" (func $p (param i32 i32 i32)))
          (memory 1)
          (func (export "render")
            (call $p (i32.const 0) (i32.const 0)
              (memory.grow (i32.const 256)))))"#,
        );
        // A refused memory.grow returns -1 rather than growing past the limit.
        assert_eq!(frame.unwrap().pixels[0], 0xffffff);
    }
}
