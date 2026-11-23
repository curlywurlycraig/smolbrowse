# smolbrowse reference implementation

A Rust implementation of the [smolbrowse v1 interface](../docs/plans/001_interface_v1.md).
It uses Wasmi to execute pages, minifb for a native macOS window, and font8x8
for the built-in bitmap font. It displays static and animated local pages; links and network
loading are not implemented.

## Build and run

Requires Rust 1.80+ and the Xcode command-line tools on macOS. SDL and Homebrew
packages are not required. Run these commands from this directory:

```sh
cd ref # when starting at the repository root
cargo run --release                                       # bundled rotating cube
cargo run --release -- ../examples/cube/build/cube.wasm    # example site from disk
cargo run --release -- path/to/page.wasm                   # another local page
```

Paths are relative to the current working directory. WAT is accepted as an
authoring convenience and compiled to Wasm before execution. Start with
[the AssemblyScript example site](../examples/cube/README.md) when authoring a
site. Its compiled Wasm is checked in; Node.js is only needed to rebuild the
example after editing its source. Release builds are recommended for animation.

The 320 × 240 canvas starts at 3× scale. Resizing preserves its aspect ratio
with black letterboxing. Press **R** to reload into a fresh instance or
**Escape** to quit. The normal window close action also exits. A failed reload
keeps the last successful image and reports the error in the terminal and
window title. An initial load failure exits with an error. If a later frame
traps, animation stops with the last successful frame visible; **R** restarts it.

## macOS app bundle

```sh
sh scripts/bundle-macos.sh
open target/release/smolbrowse.app
```

Opening the bundle shows the rotating cube; use the command line to select a
local page. This is an unsigned development bundle, not a packaged distribution
release. The script builds with the checked-in Cargo lockfile.

## Implementation

- `src/framebuffer.rs`: pixel clipping, RGB masking, and transparent 8 × 8 text.
- `src/runtime.rs`: Wasm imports, instance lifecycle, memory validation, and limits.
- `src/main.rs`: local file loading, window presentation, and reload handling.
- `../examples/cube/`: independent example site, embedded as the default page.
- `scripts/bundle-macos.sh`: builds a local macOS application bundle.

Loading creates a Wasm instance without WASI or other ambient host capabilities
and runs its start function once. The browser targets 60 calls to `render` per
second, preserving Wasm globals and memory between frames. Each call gets a
black canvas and a fresh fuel budget; its pixels are committed only on success.
Slower rendering slows the animation rather than issuing catch-up calls.
Reload creates a new instance. The committed canvas is separate from the one
being drawn so a trap cannot expose a partial frame.

Implementation resource limits:

- Source files and compiled modules: 4 MiB each.
- Linear memory: 16 MiB, at most one memory.
- Tables: at most one, limited to 4,096 elements.
- Execution: 10 million Wasmi fuel units for startup and separately for each frame.
- Host drawing: one fuel unit per pixel call; one plus 64 per UTF-8 byte for text.
- Text: at most 64 KiB per call.

These limits are reference implementation choices. They do not add to the
smolbrowse interface.

## Validation

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

The rendering core is independent of the window, so tests need no display.
Tests cover clipping, RGB masking, font placement and transparency, newlines,
unsupported characters, UTF-8 and pointer validation, the Wasm contract,
startup clearing, state persistence, frame clearing, failed frame preservation,
the cube animating across 120 frames, and execution and memory limits.
