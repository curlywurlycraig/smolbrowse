// These are the only host functions. All geometry and animation live in the site.
@external("smolbrowse", "pixel")
declare function pixel(x: i32, y: i32, color: i32): void;
@external("smolbrowse", "text")
declare function text(x: i32, y: i32, ptr: usize, length: i32, color: i32): void;

const title = String.UTF8.encode("SOLID / 001");
const sx = new StaticArray<f64>(8);
const sy = new StaticArray<f64>(8);
const depth = new StaticArray<f64>(8);
const order = new StaticArray<i32>(6);
const faceDepth = new StaticArray<f64>(6);
const faces: StaticArray<i32> = [0,4,6,2, 1,3,7,5, 0,1,5,4, 2,6,7,3, 0,2,3,1, 4,5,7,6];
const colors: StaticArray<i32> = [0x66c9cf, 0xf2ad77, 0x7bd5ba, 0xe3cb86, 0x8cace8, 0xd895af];
let frame: u32 = 0;

function shade(color: i32, light: f64): i32 {
  return (<i32>(((color >> 16) & 255) * light) << 16)
    | (<i32>(((color >> 8) & 255) * light) << 8)
    | <i32>((color & 255) * light);
}

function line(x0: i32, y0: i32, x1: i32, y1: i32, color: i32): void {
  const dx = abs(x1 - x0), dy = -abs(y1 - y0);
  const stepX = x0 < x1 ? 1 : -1, stepY = y0 < y1 ? 1 : -1;
  let error = dx + dy;
  while (true) {
    pixel(x0, y0, color);
    if (x0 == x1 && y0 == y1) break;
    const twice = error * 2;
    if (twice >= dy) { error += dy; x0 += stepX; }
    if (twice <= dx) { error += dx; y0 += stepY; }
  }
}

// Scanline triangle fill. Testing pixel centers makes shared edges meet cleanly.
function triangle(a: i32, b: i32, c: i32, color: i32): void {
  const bottom = min(239, <i32>Math.ceil(max(sy[a], max(sy[b], sy[c]))));
  const top = max(0, <i32>Math.floor(min(sy[a], min(sy[b], sy[c]))));
  for (let y = top; y <= bottom; y++) {
    const scan = <f64>y + 0.5;
    let left = 1e9, right = -1e9;
    for (let edge = 0; edge < 3; edge++) {
      const p = edge == 0 ? a : edge == 1 ? b : c;
      const q = edge == 0 ? b : edge == 1 ? c : a;
      if (scan >= min(sy[p], sy[q]) && scan < max(sy[p], sy[q])) {
        const x = sx[p] + (scan - sy[p]) * (sx[q] - sx[p]) / (sy[q] - sy[p]);
        left = min(left, x); right = max(right, x);
      }
    }
    if (left > right) continue;
    const end = min(319, <i32>Math.ceil(right - 0.5) - 1);
    for (let x = max(0, <i32>Math.ceil(left - 0.5)); x <= end; x++) pixel(x, y, color);
  }
}

export function render(): void {
  // One render call is one 1/60-second tick. No clock or graphics library needed.
  const t = <f64>frame / 60.0;
  frame++;
  const ay = t * 0.65 + 0.55, ax = t * 0.39 + 0.35;
  const cy = Math.cos(ay), syaw = Math.sin(ay);
  const cx = Math.cos(ax), spitch = Math.sin(ax);

  for (let y = 0; y < 240; y++) {
    const color = shade(0x1c3045, 0.42 + <f64>y / 360.0);
    for (let x = 0; x < 320; x++) pixel(x, y, color);
  }
  // A quiet star field and perspective floor frame the object.
  for (let i = 0; i < 42; i++) {
    const x = (i * 97 + 23) % 320, y = (i * 53 + 47) % 164;
    pixel(x, y, i % 5 == 0 ? 0x8099ad : 0x344c63);
  }
  line(0, 174, 319, 174, 0x2c4855);
  for (let x = -320; x <= 640; x += 64) line(160, 174, x, 239, 0x203d4a);
  for (let i = 1; i <= 7; i++) {
    const y = 174 + i * i * 65 / 49;
    line(0, y, 319, y, 0x203d4a);
  }
  // Soft elliptical contact shadow, with the cube floating just above it.
  for (let y = -7; y <= 7; y++) {
    for (let x = -53; x <= 53; x++) {
      const d = <f64>(x * x) / 2809.0 + <f64>(y * y) / 49.0;
      if (d < 1.0) pixel(160 + x, 193 + y, d < 0.65 ? 0x0a1521 : 0x142632);
    }
  }

  for (let v = 0; v < 8; v++) {
    const x: f64 = (v & 1) != 0 ? 1 : -1;
    const y: f64 = (v & 2) != 0 ? 1 : -1;
    const z: f64 = (v & 4) != 0 ? 1 : -1;
    const rx = x * cy + z * syaw, rz = -x * syaw + z * cy;
    const ry = y * cx - rz * spitch, zz = y * spitch + rz * cx;
    depth[v] = zz;
    sx[v] = 160.0 + rx * 145.0 / (4.6 + zz);
    sy[v] = 122.0 + ry * 145.0 / (4.6 + zz) + Math.sin(t * 1.3) * 4.0;
  }
  // Painter's order is sufficient for the six nonintersecting faces of a cube.
  for (let f = 0; f < 6; f++) {
    order[f] = f;
    faceDepth[f] = (depth[faces[f*4]] + depth[faces[f*4+1]]
      + depth[faces[f*4+2]] + depth[faces[f*4+3]]) / 4.0;
  }
  for (let i = 1; i < 6; i++) {
    const f = order[i];
    let j = i;
    while (j > 0 && faceDepth[order[j-1]] < faceDepth[f]) {
      order[j] = order[j-1]; j--;
    }
    order[j] = f;
  }
  for (let i = 0; i < 6; i++) {
    const f = order[i];
    const nx: f64 = f == 0 ? -1 : f == 1 ? 1 : 0;
    const ny: f64 = f == 2 ? -1 : f == 3 ? 1 : 0;
    const nz: f64 = f == 4 ? -1 : f == 5 ? 1 : 0;
    const rx = nx * cy + nz * syaw, rz = -nx * syaw + nz * cy;
    const ry = ny * cx - rz * spitch, zz = ny * spitch + rz * cx;
    const light = 0.32 + 0.68 * max(0.0, -rx * 0.4 - ry * 0.7 - zz * 0.59);
    const color = shade(colors[f], light);
    const a = faces[f*4], b = faces[f*4+1], c = faces[f*4+2], d = faces[f*4+3];
    triangle(a, b, c, color); triangle(a, c, d, color);
    for (let e = 0; e < 4; e++) {
      const p = faces[f*4+e], q = faces[f*4+(e+1)%4];
      line(<i32>sx[p], <i32>sy[p], <i32>sx[q], <i32>sy[q], shade(color, 0.65));
    }
  }
  text(16, 14, changetype<usize>(title), title.byteLength, 0xedd7b0);
}
