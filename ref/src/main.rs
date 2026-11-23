mod framebuffer;
mod runtime;

use std::{env, fs::File, io::Read, path::PathBuf};

use anyhow::{bail, Context, Result};
use framebuffer::{Framebuffer, HEIGHT, WIDTH};
use minifb::{Key, KeyRepeat, ScaleMode, Window, WindowOptions};

const DEMO: &str = include_str!("../examples/hello.wat");

fn load(path: Option<&PathBuf>) -> Result<Framebuffer> {
    let source = if let Some(path) = path {
        let mut bytes = Vec::new();
        File::open(path)
            .with_context(|| format!("opening {}", path.display()))?
            .take((runtime::MAX_MODULE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > runtime::MAX_MODULE_BYTES {
            bail!("page exceeds 4 MiB");
        }
        bytes
    } else {
        DEMO.as_bytes().to_vec()
    };
    let wasm = wat::parse_bytes(&source).context("could not read Wasm or WAT page")?;
    runtime::render(&wasm)
}

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help")) {
        println!("Usage: smolbrowse [page.wasm|page.wat]\n\nNo path opens the bundled demo. R reloads; Escape quits.");
        return Ok(());
    }
    if args.next().is_some() {
        bail!("expected at most one page path; see --help");
    }
    let path = first.map(PathBuf::from);
    let name = path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "welcome".into());
    let title = format!("smolbrowse — {name}");
    let mut frame = load(path.as_ref())?;
    let mut window = Window::new(
        &title,
        WIDTH * 3,
        HEIGHT * 3,
        WindowOptions {
            scale_mode: ScaleMode::AspectRatioStretch,
            resize: true,
            ..WindowOptions::default()
        },
    )
    .context("opening macOS window")?;
    window.set_target_fps(60);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            match load(path.as_ref()) {
                Ok(next) => {
                    frame = next;
                    window.set_title(&title);
                }
                Err(error) => {
                    eprintln!("Reload failed: {error:#}");
                    window.set_title(&format!("{title} — reload failed (see terminal)"));
                }
            }
        }
        window.update_with_buffer(&frame.pixels, WIDTH, HEIGHT)?;
    }
    Ok(())
}
