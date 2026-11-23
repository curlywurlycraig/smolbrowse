# Rotating cube

A smolbrowse site written in AssemblyScript. A floating cube rotates above a
perspective grid, with colored faces, directional lighting, outlines, a shadow,
and a star field. Everything is software-rendered by the site through the two
smolbrowse drawing functions.

## Compile

Requires Node.js 20+ and npm 10+. From this directory:

```sh
npm ci
npm run build
```

This compiles [assembly/index.ts](assembly/index.ts) to `build/cube.wasm` using
the pinned AssemblyScript compiler. The generated Wasm is checked in so the
example can be opened without a JavaScript toolchain. Rebuild it after changing
the source. There is no JavaScript runtime or glue code required by the site.

To open it in the reference browser, from the repository root:

```sh
cargo run --release --manifest-path ref/Cargo.toml -- examples/cube/build/cube.wasm
```

After rebuilding, press **R** in that browser to restart the animation.

## How it works

`render()` advances a persistent frame counter, rotates eight vertices,
projects them onto the 320 × 240 canvas, sorts six faces by depth, and fills
them using scanline triangles. Face normals rotate with the cube and determine
the lighting. The host calls `render()` at a target of 60 frames per second;
each call advances the site's animation by one tick. Static UTF-8 labels are
encoded once during startup. No allocations occur during rendering.

The compiler uses its stub runtime and a 256-page maximum memory. The unused
AssemblyScript abort import is disabled, leaving only `smolbrowse.pixel` and
`smolbrowse.text` as imports. See the
[AssemblyScript compiler documentation](https://www.assemblyscript.org/compiler.html)
for the build flags. The source uses fixed-size arrays with bounded indices.

## Test

```sh
npm run build
npm test
```

The Node test checks the compiled imports, UTF-8 labels, visible changes across
frames, distinct face colors, and stable memory over 600 frames. The reference
browser also tests this Wasm artifact in its own runtime.
