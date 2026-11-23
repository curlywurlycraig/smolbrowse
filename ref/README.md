# smolbrowse reference implementation

A Rust implementation of the [smolbrowse v1 interface](../docs/plans/001_interface_v1.md).
It uses Wasmi to execute pages, minifb for a native macOS window, and font8x8
for the built-in bitmap font. It displays static local pages; links and network
loading are not implemented.

## Build and run

Requires Rust 1.80+ and the Xcode command-line tools on macOS. SDL and Homebrew
packages are not required. Run these commands from this directory:

```sh
cd ref # when starting at the repository root
cargo run                       # bundled welcome page
cargo run -- examples/hello.wat  # local WAT source
cargo run -- path/to/page.wasm   # local compiled page
```

Paths are relative to the current working directory. WAT is accepted as an
authoring convenience and compiled to Wasm before execution. Start with
[the example page](examples/hello.wat) when authoring a site.

The 320 × 240 canvas starts at 3× scale. Resizing preserves its aspect ratio
with black letterboxing. Press **R** to reload into a fresh instance or
**Escape** to quit. The normal window close action also exits. A failed reload
keeps the last successful image and reports the error in the terminal and
window title. An initial load failure exits with an error.

## macOS app bundle

```sh
sh scripts/bundle-macos.sh
open target/release/smolbrowse.app
```

Opening the bundle shows the welcome page; use the command line to select a
local page. This is an unsigned development bundle, not a packaged distribution
release. The script builds with the checked-in Cargo lockfile.

## Implementation

- `src/framebuffer.rs`: pixel clipping, RGB masking, and transparent 8 × 8 text.
- `src/runtime.rs`: Wasm imports, instance lifecycle, memory validation, and limits.
- `src/main.rs`: local file loading, window presentation, and reload handling.
- `examples/hello.wat`: bundled page using both drawing functions.
- `scripts/bundle-macos.sh`: builds a local macOS application bundle.

Each render creates a fresh Wasm instance without WASI or other ambient host
capabilities. The browser runs its start function, clears the canvas, calls
`render`, and displays the completed framebuffer on success.

Implementation resource limits:

- Source files and compiled modules: 4 MiB each.
- Linear memory: 16 MiB, at most one memory.
- Tables: at most one, limited to 4,096 elements.
- Execution: 10 million Wasmi fuel units shared by startup and rendering.
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
startup clearing, the example page, and execution and memory limits.
