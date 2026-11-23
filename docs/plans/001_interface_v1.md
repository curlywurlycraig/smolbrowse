# Interface v1

smolbrowse is a fantasy web browser: a small, shared medium for playful,
personal, artistic experiences. V1 starts with a canvas and two drawing
functions, supporting static and animated pages. It leaves site-initiated navigation, networking, input,
audio, palettes, and additional drawing primitives for later versions.

## Canvas

- 320 × 240 pixels, initially black before each render.
- Integer coordinates with the origin at the top left; x increases rightward,
  y downward.
- Colors are i32 values interpreted as `0x00RRGGBB`; the high byte is ignored.
- Out-of-bounds drawing is clipped without trapping.
- The browser scales the canvas to its window, preserves its aspect ratio,
  and adds black letterboxing. Sites always see the same logical resolution.

## Wasm contract

Sites are core WebAssembly modules, with no WASI or ambient host capabilities.
All ABI arguments are Wasm i32 values. Imports live in the `smolbrowse` module:

```wat
(import "smolbrowse" "pixel" (func $pixel (param i32 i32 i32)))
(import "smolbrowse" "text" (func $text (param i32 i32 i32 i32 i32)))
```

Equivalent C declarations:

```c
void pixel(int32_t x, int32_t y, int32_t color);
void text(int32_t x, int32_t y, const char *utf8,
          int32_t byte_length, int32_t color);
```

`pixel` sets one pixel. `text` draws transparent-background text with a built-in
8 × 8 monospace bitmap font. Coordinates identify the top-left corner of the
first cell. Each character advances x by 8; newline resets x to the original x
and advances y by 8. There is no automatic wrapping. Printable ASCII U+0020
through U+007E is guaranteed; all other characters render as `?`, except newline.
Spaces advance without painting. Glyph rows use the public-domain font8x8 basic
font, with the least significant bit at the left of each row.

The pointer is an unsigned offset into the site's exported `memory`, and length
is an unsigned byte count. The browser reads the exact range synchronously;
there is no terminator. Invalid UTF-8, invalid memory ranges, or missing memory
on a text call trap the page. Even empty strings must reference a valid range.

The site exports `render` with signature `() -> ()`. On load, the browser creates
an instance and runs any Wasm start function once. It then calls `render`
repeatedly, targeting 60 frames per second. The canvas is cleared to black before
every call; Wasm globals and memory persist between calls. Reloading creates a
fresh instance and resets site state. Sites using text must export their linear
memory as `memory`. Sites using only pixels need not export memory.

Each render call represents one animation tick, nominally 1/60 second. There is
no clock import or elapsed-time argument. On slower hosts, animation slows with
the frame rate; hosts do not issue catch-up ticks. A static site simply draws
the same image each call. Only a successful render is displayed. If a later
render fails, animation stops and the last successful frame remains visible.

## Host behavior

Hosts must report malformed modules, incompatible imports or exports, and
execution traps as page failures. A failed render must not display a partially
drawn canvas. Hosts may impose resource limits and must report exhaustion as a
page failure. Specific limits and browser controls are implementation choices.

The [reference implementation](../../ref/README.md) documents its own runtime,
resource limits, controls, and validation.
