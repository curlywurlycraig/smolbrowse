import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('compiled site uses only the v1 drawing API and animates without allocating per frame', async () => {
  const module = await WebAssembly.compile(readFileSync(new URL('./build/cube.wasm', import.meta.url)));
  assert.deepEqual(WebAssembly.Module.imports(module).map(({ module, name }) => `${module}.${name}`).sort(),
    ['smolbrowse.pixel', 'smolbrowse.text']);
  const pixels = new Uint32Array(320 * 240);
  const labels = [];
  const instance = await WebAssembly.instantiate(module, { smolbrowse: {
    pixel(x, y, color) {
      if (x >= 0 && x < 320 && y >= 0 && y < 240) pixels[y * 320 + x] = color & 0xffffff;
    },
    text(x, y, ptr, length, color) {
      const bytes = new Uint8Array(instance.exports.memory.buffer, ptr, length);
      labels.push(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
    },
  }});
  instance.exports.render();
  assert.deepEqual(labels, ['SOLID / 001', 'A LITTLE WORLD, MADE OF PIXELS.', 'ASSEMBLYSCRIPT  /  SMOLBROWSE']);
  const first = pixels.slice();
  const initialMemory = instance.exports.memory.buffer.byteLength;
  // Sample a full turn and more, exercising long-lived globals and allocations.
  for (let i = 0; i < 600; i++) {
    pixels.fill(0);
    instance.exports.render();
    if (i === 59) {
      assert.ok(pixels.reduce((sum, value, index) => sum + (value !== first[index]), 0) > 1000);
      const faceColors = new Set();
      for (let y = 80; y < 160; y++) {
        for (let x = 120; x < 200; x++) faceColors.add(pixels[y * 320 + x]);
      }
      assert.ok(faceColors.size >= 3, 'cube should contain distinct shaded surfaces');
    }
  }
  assert.equal(instance.exports.memory.buffer.byteLength, initialMemory);
});
