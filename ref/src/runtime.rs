use anyhow::{bail, Context, Result};
use wasmi::{
    Caller, Config, Engine, Error as WasmError, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder,
};

use crate::framebuffer::Framebuffer;

pub const MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const FUEL: u64 = 10_000_000;

struct Page {
    frame: Framebuffer,
    limits: StoreLimits,
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

pub fn render(bytes: &[u8]) -> Result<Framebuffer> {
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
    store.data_mut().frame.pixels.fill(0);
    render.call(&mut store, ()).context("page render failed")?;
    Ok(store.into_data().frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(source: &str) -> Result<Framebuffer> {
        render(&wat::parse_str(source).unwrap())
    }

    #[test]
    fn bundled_page_renders() {
        let frame = page(include_str!("../examples/hello.wat")).unwrap();
        assert_eq!(frame.pixels[0], 0x172438);
        assert_eq!(frame.pixels[239 * 320 + 160], 0x526f83);
        assert!(frame.pixels.contains(&0xf5dbb4));
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
        assert!(render(b"invalid wasm").is_err());
        assert!(render(&vec![0; MAX_MODULE_BYTES + 1]).is_err());
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
