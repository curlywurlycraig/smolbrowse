# smolbrowse reference implementation

A Rust implementation of [smolbrowse v2](../docs/plans/002_interface_v2.md),
compatible with [v1](../docs/plans/001_interface_v1.md).
It uses Wasmi to execute pages, minifb for a native macOS window, and font8x8
for the built-in bitmap font. It displays static and animated pages from local
files or HTTP(S) URLs. Browser controls stay hidden until invoked; sites use two drawing imports and an optional event-detail import. The viewer uses an antialiased macOS monospace
font for its own UI, separate from the site bitmap font.
UI text is rasterized at the window's native backing scale on Retina displays.
The presentation buffer uses physical pixels while UI dimensions remain in
logical points; moving between displays updates the scale and glyph cache.

## Build and run

Requires Rust 1.80+ and the Xcode command-line tools on macOS. SDL and Homebrew
packages are not required. Run these commands from this directory:

```sh
cd ref # when starting at the repository root
cargo run --release                                       # empty viewer; : or Cmd+L opens commands
cargo run --release -- ../examples/cube/build/cube.wasm    # animated v1 site
cargo run --release -- ../examples/tip/build/tip.wasm      # interactive v2 puzzle
cargo run --release -- path/to/page.wasm                   # another local page
cargo run --release -- https://example.org/page.wasm       # hosted page
```

The viewer contains no embedded site and has no build dependency on the example.
Paths are relative to the current working directory; absolute paths, `~/` paths,
and `file://` URLs are also accepted. URLs must include `http://` or `https://`.
WAT is accepted as an authoring convenience and compiled to Wasm before execution. Start with
[the AssemblyScript example site](../examples/cube/README.md) when authoring a
site. Its compiled Wasm is checked in; Node.js is only needed to rebuild the
example after editing its source. Release builds are recommended for animation.

## Command line and controls

Pass a path or URL on the CLI to launch straight into that experience, with no
visible browser controls. From the repository root:

```sh
cargo run --release --manifest-path ref/Cargo.toml -- examples/cube/build/cube.wasm
```

After building, you can also invoke the executable directly:

```sh
ref/target/release/smolbrowse examples/cube/build/cube.wasm
ref/target/release/smolbrowse https://example.org/page.wasm
```

The page uses the whole viewport. Press **:** to open a small command line at
its bottom; **Enter** executes and hides it, and **Escape** cancels and hides it.
Commands:

- `:open <path-or-url>` (also `:o`, `:edit`, or `:e`) opens a site. Paths with
  spaces do not need quoting inside the viewer.
- `:reload` (or `:r`) rereads the current file or fetches its URL again.
- `:quit` (or `:q`) exits.

**Cmd+L** opens the prompt with the current address selected; paste or type a
replacement and press **Enter**. A bare path or URL is accepted without `open`.
The editor supports **Cmd+A**, **Cmd+C**, **Cmd+V**, arrows, Home, End, Backspace,
and Delete. Ctrl may be used in place of Cmd. For interactive sites, **Cmd/Ctrl+L** opens browser controls;
**Cmd/Ctrl+R** or **F5** reloads. All ordinary keys, including `:`, `q`, `r`, and
Escape, go to the site. For v1 sites without an `event` export, the old **:**,
**r**, and **q** shortcuts remain available. Closing the window also exits.

The 320 × 240 site canvas starts at 3× scale. Resizing preserves its aspect
ratio with black letterboxing. The command line overlays the bottom of the
page temporarily; showing it never resizes or shifts the canvas. No toolbar,
buttons, or success status remain visible while a site is running.

Reload creates a fresh Wasm instance and reads the site again; rebuilding a
site and pressing **r** requires no Rust rebuild. Fetching runs in background
threads while the current page keeps animating. Loading and errors appear as
plain text at the bottom; successful loads clear that line. **Escape** dismisses
an error. Failed loads keep the current page visible, and only the latest
navigation may replace it. If rendering traps, animation stops with the last
successful frame visible; reload restarts it. Full errors also go to stderr.

## Site input

V2 sites optionally export `event(i32) -> ()` and import
`smolbrowse.event_get(i32) -> i32`. Events arrive in order before rendering;
details are readable only during the callback. The host clears drawing done
inside handlers before rendering. A handler trap stops input and animation and
preserves the last committed frame. Opening browser controls or losing window
focus resets site input, so held keys and mouse drags cannot get stuck.

On macOS, a local AppKit event monitor preserves mouse presses and releases
between frames, handles all three buttons, and maps window coordinates onto
the canvas including Retina scaling and letterboxing. An invisible AppKit text
client handles composition and emits only committed Unicode scalars. Physical
key codes remain independent of text layout. Mouse drags remain captured outside
the canvas; loss of window focus cancels capture with an input reset.
The non-macOS fallback samples mouse buttons once per frame and does not yet
provide the same mouse-transition fidelity; macOS is the supported native host.

Try [TIP](../examples/tip/README.md) for a site that owns all its game state and
UI using this interface.

## macOS app bundle

```sh
sh scripts/bundle-macos.sh
open target/release/smolbrowse.app
```

Opening the bundle shows an empty viewer with a small command hint; the
prompt stays hidden until invoked. This is a development bundle with a local ad-hoc signature, not a notarized distribution
release. The script builds with the checked-in Cargo lockfile.

## Implementation

- `src/framebuffer.rs`: pixel clipping, RGB masking, and transparent 8 × 8 text.
- `src/runtime.rs`: Wasm imports, event dispatch, instance lifecycle, memory validation, and limits.
- `src/input.rs`: input enums, key translation, focus suppression, and pointer routing.
- `src/native_input.rs`: macOS event capture and committed text through AppKit.
- `src/main.rs`: window presentation, input, clipboard, and navigation controls.
- `src/chrome.rs`: command parsing/editing, monospace UI rendering, and canvas scaling.
- `src/display.rs`: logical window size and macOS Retina backing-scale detection.
- `src/browser.rs`: navigation state, background loading, and frame advancement.
- `src/loader.rs`: local files, file URLs, HTTP(S), and source decoding.
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
- Execution: 10 million Wasmi fuel units for startup, each frame, and each input batch.
- Input: at most 4,096 pending native events; overflow stops an interactive site.
- Event detail reads: one fuel unit each.
- Host drawing: one fuel unit per pixel call; one plus 64 per UTF-8 byte for text.
- Text: at most 64 KiB per call.
- HTTP(S): a 20-second overall request timeout and at most five redirects.

HTTPS uses certificate verification. Reloads send `Cache-Control: no-cache`;
there is no viewer cache. The source-size cap is enforced while reading both
local files and response bodies, regardless of `Content-Length`.

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
event callback scope, signature validation, handler traps, v1/v2 example
compatibility, and execution and memory limits. Input tests cover paired key/text
events, repeat suppression, merged equivalent keys, focus resets, pointer
capture, letterboxing, and Retina coordinate mapping. Browser tests cover Unicode address editing,
hidden chrome and prompt overlays, command parsing, local-file reloads, file URLs, size limits, HTTP redirects
and error statuses, stale navigation results, and failed navigation preservation.
HTTP tests use a local test server and need no internet connection. The example
site's own tests live under `../examples/cube/`.
