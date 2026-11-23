import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import test from "node:test";

const bytes = readFileSync(new URL("./build/tip.wasm", import.meta.url));
const boards = [
  "1111111111111111111111111",
  "1111110011111111100111111",
  "1111101010111110111011111",
];
const directions = [
  [1, 0, 261],
  [-1, 0, 260],
  [0, 1, 263],
  [0, -1, 262],
];
function roll(o, dx, dy) {
  const result = [...o];
  const a = dx ? (dx > 0 ? 2 : 3) : dy > 0 ? 4 : 5;
  const b = dx ? (dx > 0 ? 3 : 2) : dy > 0 ? 5 : 4;
  [result[0], result[1], result[a], result[b]] = [o[b], o[a], o[0], o[1]];
  return result;
}
function solution(board) {
  const queue = [[0, 0, [0, 1, 2, 3, 4, 5], []]],
    seen = new Set();
  for (let i = 0; i < queue.length; i++) {
    const [x, y, o, path] = queue[i];
    if (x === 4 && y === 4) return path;
    const id = [x, y, ...o].join();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const [dx, dy, key] of directions) {
      const nx = x + dx,
        ny = y + dy,
        no = roll(o, dx, dy);
      if (nx < 0 || ny < 0 || nx >= 5 || ny >= 5 || board[ny * 5 + nx] !== "1")
        continue;
      if (nx === 4 && ny === 4 && no[1] !== 0) continue;
      queue.push([nx, ny, no, [...path, key]]);
    }
  }
  throw new Error("unsolvable board");
}
async function site() {
  let fields = [],
    instance;
  const pixels = new Uint32Array(320 * 240),
    labels = [];
  instance = await WebAssembly.instantiate(bytes, {
    smolbrowse: {
      pixel(x, y, c) {
        if (x >= 0 && x < 320 && y >= 0 && y < 240)
          pixels[y * 320 + x] = c & 0xffffff;
      },
      text(x, y, p, n) {
        labels.push(
          new TextDecoder("utf-8", { fatal: true }).decode(
            new Uint8Array(instance.instance.exports.memory.buffer, p, n),
          ),
        );
      },
      event_get(field) {
        return fields[field - 1] ?? 0;
      },
    },
  });
  const exports = instance.instance.exports;
  const render = (n = 1) => {
    for (let i = 0; i < n; i++) {
      pixels.fill(0);
      labels.length = 0;
      exports.render();
    }
  };
  const event = (kind, values) => {
    fields = values;
    exports.event(kind);
    fields = [];
  };
  const key = (k) => {
    event(4, [0, 0, 0, k]);
    event(5, [0, 0, 0, k]);
  };
  return { exports, pixels, labels, render, event, key };
}

test("v2-only imports, visible animation, invalid moves, undo and restart", async () => {
  const module = await WebAssembly.compile(bytes);
  assert.deepEqual(
    WebAssembly.Module.imports(module)
      .map((i) => `${i.module}.${i.name}`)
      .sort(),
    ["smolbrowse.event_get", "smolbrowse.pixel", "smolbrowse.text"],
  );
  const s = await site();
  s.render();
  const first = s.pixels.slice();
  s.key(260);
  s.render();
  assert.ok(s.labels.includes("000"));
  assert.ok(s.labels.includes("ONLY THE STONE CAN HOLD YOU"));
  s.key(261);
  s.render(6);
  assert.notDeepEqual(s.pixels, first);
  s.render(10);
  assert.ok(s.labels.includes("001"));
  s.key(90);
  s.render();
  assert.deepEqual(s.pixels, first);
  assert.ok(s.labels.includes("000"));
  s.key(261);
  s.render(14);
  s.key(82);
  s.render();
  assert.deepEqual(s.pixels, first);
  s.event(6, [0, 0, 0, 0, 97]);
  s.event(7, []);
  s.event(999, []);
  s.render();
  assert.deepEqual(s.pixels, first);
});

test("all three boards are solvable through real event/render calls", async () => {
  const s = await site();
  for (let level = 0; level < 3; level++) {
    const path = solution(boards[level]);
    for (const key of path) {
      s.key(key);
      s.render(14);
    }
    s.render(30);
    assert.ok(
      s.labels.includes("SLOTTED!  ENTER / CLICK: NEXT"),
      `board ${level + 1}, path ${path}`,
    );
    s.key(256);
    s.render();
    assert.ok(s.labels.includes("000"));
  }
});

test("clicking a neighboring tile matches keyboard movement; memory stays bounded", async () => {
  const s = await site(),
    keyboard = await site();
  s.render();
  keyboard.render();
  // Ground center of tile (1,0).
  s.event(2, [179, 86, 1]);
  s.render(14);
  keyboard.key(261);
  keyboard.render(14);
  assert.deepEqual(s.pixels, keyboard.pixels);
  const size = s.exports.memory.buffer.byteLength;
  s.render(600);
  assert.equal(s.exports.memory.buffer.byteLength, size);
});

test("rejects the hole unless the gold face points down", async () => {
  const s = await site();
  for (const key of [261, 261, 261, 261, 263, 263, 263, 263]) {
    s.key(key);
    s.render(14);
  }
  assert.ok(s.labels.includes("THE GOLD FACE MUST POINT DOWN"));
  assert.ok(s.labels.includes("007"));
  assert.ok(!s.labels.includes("SLOTTED!  ENTER / CLICK: NEXT"));
});

test("winning clips the falling cube behind the rim and emits a finite particle burst", async () => {
  const s = await site();
  const path = solution(boards[0]);
  for (const key of path.slice(0, -1)) {
    s.key(key);
    s.render(14);
  }
  s.key(path.at(-1));
  s.render(13); // The tip has finished; the next frame begins the sink.
  const memorySize = s.exports.memory.buffer.byteLength;
  const cubeColors = new Set([
    0xdce8df, 0x607a83, 0x9bb6b5, 0x496a7a, 0x75979e, 0xb7ceca,
  ]);
  const sparkColors = new Set([0x8ee3c2, 0xfff1c2, 0xf8d978]);
  let sawSparks = false;
  let initialCubePixels = 0;
  let settled;
  for (let frame = 0; frame < 100; frame++) {
    s.render();
    assert.equal(
      s.pixels[166 * 320 + 160],
      0xc18b45,
      `front rim at frame ${frame}`,
    );
    for (let yy = 167; yy < 190; yy++)
      for (let xx = 130; xx <= 190; xx++) {
        assert.ok(
          !cubeColors.has(s.pixels[yy * 320 + xx]),
          `cube leaked below the platform at frame ${frame}`,
        );
      }
    const cubePixels = s.pixels.reduce((n, c) => n + cubeColors.has(c), 0);
    if (frame === 0) initialCubePixels = cubePixels;
    if (frame >= 24)
      assert.equal(cubePixels, 0, "fully submerged cube must be hidden");
    if (s.pixels.some((c) => sparkColors.has(c))) sawSparks = true;
    if (frame >= 75)
      assert.ok(
        !s.pixels.some((c) => sparkColors.has(c)),
        "sparks must expire",
      );
    if (frame === 80) settled = s.pixels.slice();
    if (frame === 99)
      assert.deepEqual(s.pixels, settled, "win animation must settle");
    if (process.env.TIP_PREVIEW && [0, 8, 16, 28].includes(frame)) {
      const rgb = Buffer.alloc(320 * 240 * 3);
      for (let i = 0; i < s.pixels.length; i++) {
        rgb[i * 3] = s.pixels[i] >>> 16;
        rgb[i * 3 + 1] = s.pixels[i] >>> 8;
        rgb[i * 3 + 2] = s.pixels[i];
      }
      writeFileSync(
        `${process.env.TIP_PREVIEW}-${frame}.ppm`,
        Buffer.concat([Buffer.from("P6\n320 240\n255\n"), rgb]),
      );
    }
  }
  assert.ok(
    initialCubePixels > 100,
    "cube must be visible at the start of sinking",
  );
  assert.ok(sawSparks, "winning should emit visible sparks");
  assert.equal(s.exports.memory.buffer.byteLength, memorySize);
  // Undo and restart must discard the burst, even while it is active.
  s.key(90);
  s.render();
  s.key(path.at(-1));
  s.render(16);
  assert.ok(s.pixels.some((c) => sparkColors.has(c)));
  s.key(90);
  s.render();
  assert.ok(!s.pixels.some((c) => sparkColors.has(c)));
  s.key(82);
  s.render();
  assert.ok(!s.pixels.some((c) => sparkColors.has(c)));
});
