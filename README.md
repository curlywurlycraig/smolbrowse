# smolbrowse

smolbrowse is a "fantasy web browser". Much like fantasy consoles like PICO-8,
we define a small interface, and within that interface people can create all
sorts of experiences. This repository defines the smolbrowse specification.

Usual web browsers are _maximalist_. Instead, smolbrowse is designed to be _minimalist_ and _experience and art_ influenced. The modern web has been polluted by business concerns rather than artistic concerns. Fantasy consoles aren't designed to be _web first_. smolbrowse therefore fills a niche of hackerdom, whimsy, and internet reclamation that keeps a corner for ourselves, for fun.

Aesthetically, think of [Teletext](https://en.wikipedia.org/wiki/Teletext),
PICO-8 pixel art, etc. V1 uses a fixed-resolution pixel canvas.

smolbrowse "websites" are `.wasm` that implement the smolbrowse interface. The interface is designed to be minimal. Using WebAssembly allows smolbrowse websites to be written in any language that targets WebAssembly.

## Specification

[Interface v1](docs/plans/001_interface_v1.md) defines static and animated pages with a
320 × 240 RGB canvas, an 8 × 8 monospace font, and two drawing functions:
`smolbrowse.pixel` and `smolbrowse.text`. Sites export a single `render` function.
The host calls `render` repeatedly while site state persists between frames.
Links, networking, and input are left for future versions.

## Example sites

[Examples](examples/README.md) demonstrate the interface. The first is an
AssemblyScript rotating cube with simple shading and a small compilation step.

## Reference implementation

The reference browser lives in [ref/](ref/README.md), with its own build, usage,
and implementation documentation.
