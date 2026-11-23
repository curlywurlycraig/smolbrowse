# TIP

An isometric cube puzzle written in AssemblyScript for smolbrowse v2. Tip the
cube across the stone grid and slot it into the gold-rimmed hole with its gold
face pointing down. Three boards change the routes available to you. There is
no timer, falling penalty, or move limit; experiment and undo.

The site draws its own board, cube, messages, and controls using only `pixel`
and `text`. The browser supplies input events, without built-in widgets or a
game engine. Cube orientation, legal moves, animation, and undo all live in
Wasm state.

## Play

From the repository root:

```sh
cargo run --release --locked --manifest-path ref/Cargo.toml -- examples/tip/build/tip.wasm
```

The compiled Wasm is checked in; Node.js is only needed to rebuild it.

- **Arrows / WASD:** tip one tile. Up/W moves northeast, Right/D southeast,
  Down/S southwest, and Left/A northwest.
- **Click:** move onto a neighboring tile. Available neighbors have lighter tops.
- **Z / Backspace:** undo, including after completing a board.
- **R:** restart the current board.
- **Enter / click after winning:** continue to the next board; the third loops
  back to the first.
- **Cmd/Ctrl+L:** open browser controls. **Cmd/Ctrl+R / F5:** reload the site.

A wrong-face approach to the hole is rejected with a hint. Moves off the stone
are also rejected. Input during a tip is ignored until that animation finishes.
Undo keeps the most recent 256 moves. Winning sinks the cube behind the hole's
rim and releases a short gold-and-mint particle burst. Undo and restart clear
the particles.

## Build and test

Requires Node.js 20+ and npm 10+. From this directory:

```sh
npm ci
npm run build
npm test
```

[Source](assembly/index.ts) imports the small [ABI declarations and enums](assembly/api.ts).
It uses the pinned AssemblyScript compiler, its stub
runtime, and a 256-page maximum memory. No JavaScript glue is needed at runtime.
There are no per-frame allocations. `event(kind)` handles key presses and
primary mouse presses through `smolbrowse.event_get`; `render()` draws the
frame and advances a 13-tick rotation around the cube's contacting edge.

The tests instantiate the actual compiled Wasm. An independent breadth-first
solver finds a route for every board, then sends that route through the site's
input handler and verifies completion. Other checks cover invalid moves,
orientation at the hole, undo, restart, click/keyboard equivalence, imports,
animation, falling-cube occlusion, particle expiry, and stable memory over 600 frames. Rust tests also run the artifact
in the reference runtime to check its execution budget and ABI compatibility.
