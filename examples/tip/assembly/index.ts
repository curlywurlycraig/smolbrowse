import {
  pixel,
  text,
  event_get,
  EventKind,
  Field,
  Key,
  MouseButton,
} from "./api";

// A site owns all widgets, movement, projection, and puzzle state.
const title = String.UTF8.encode("TIP / A SMALL CUBE PUZZLE");
const rule = String.UTF8.encode("GOLD FACE DOWN INTO THE HOLE.");
const help = String.UTF8.encode("ARROWS/WASD  Z UNDO  R RESET");
const clickHelp = String.UTF8.encode("OR CLICK A NEIGHBORING TILE");
const solved = String.UTF8.encode("SLOTTED!  ENTER / CLICK: NEXT");
const wrong = String.UTF8.encode("THE GOLD FACE MUST POINT DOWN");
const edge = String.UTF8.encode("ONLY THE STONE CAN HOLD YOU");
const empty = String.UTF8.encode("FIND A ROUTE. MIND YOUR FACE.");
const levelNames: StaticArray<string> = [
  "01 / FIRST TURN",
  "02 / THE DETOUR",
  "03 / LITTLE ISLANDS",
];
const boards: StaticArray<string> = [
  "1111111111111111111111111",
  "1111110011111111100111111",
  "1111101010111110111011111",
];
const labels = new StaticArray<ArrayBuffer>(3);
for (let i = 0; i < 3; i++) labels[i] = String.UTF8.encode(levelNames[i]);
let level: i32 = 0,
  x: i32 = 0,
  y: i32 = 0,
  moves: i32 = 0;
// Face identity at top, bottom, east, west, south, north. Face 0 is gold.
const orientation = new StaticArray<i32>(6);
const history = new StaticArray<i32>(8 * 256);
let historySize: i32 = 0;
let anim: i32 = 0,
  dx: i32 = 0,
  dy: i32 = 0,
  won: bool = false;
let notice: i32 = 0,
  noticeTicks: i32 = 0;
const vx = new StaticArray<f64>(8),
  vy = new StaticArray<f64>(8),
  vz = new StaticArray<f64>(8);
const faces: StaticArray<i32> = [
  4, 5, 7, 6, 0, 2, 3, 1, 1, 3, 7, 5, 0, 4, 6, 2, 2, 6, 7, 3, 0, 1, 5, 4,
];
const order = new StaticArray<i32>(6),
  depths = new StaticArray<f64>(6);
const faceColors: StaticArray<i32> = [
  0xdce8df, 0x607a83, 0x9bb6b5, 0x496a7a, 0x75979e, 0xb7ceca,
];
const digits = new StaticArray<u8>(3);
// Fixed scratch space for clipping faces against the board's surface.
const corners = new StaticArray<i32>(4);
const clippedX = new StaticArray<f64>(8),
  clippedY = new StaticArray<f64>(8);
const particleX = new StaticArray<f64>(48),
  particleY = new StaticArray<f64>(48),
  particleVX = new StaticArray<f64>(48),
  particleVY = new StaticArray<f64>(48);
let burstAge: i32 = -1;

function reset(): void {
  x = 0;
  y = 0;
  moves = 0;
  historySize = 0;
  anim = 0;
  won = false;
  noticeTicks = 0;
  burstAge = -1;
  for (let i = 0; i < 6; i++) orientation[i] = i;
}
reset();
function label(x: i32, y: i32, value: ArrayBuffer, color: i32): void {
  text(x, y, changetype<usize>(value), value.byteLength, color);
}
function number(x: i32, y: i32, n: i32): void {
  digits[0] = <u8>(48 + ((n / 100) % 10));
  digits[1] = <u8>(48 + ((n / 10) % 10));
  digits[2] = <u8>(48 + (n % 10));
  text(x, y, changetype<usize>(digits), 3, 0xf2c66d);
}
function line(x0: i32, y0: i32, x1: i32, y1: i32, c: i32): void {
  const ax = abs(x1 - x0),
    ay = -abs(y1 - y0),
    sx = x0 < x1 ? 1 : -1,
    sy = y0 < y1 ? 1 : -1;
  let e = ax + ay;
  while (true) {
    pixel(x0, y0, c);
    if (x0 == x1 && y0 == y1) break;
    const e2 = e * 2;
    if (e2 >= ay) {
      e += ay;
      x0 += sx;
    }
    if (e2 <= ax) {
      e += ax;
      y0 += sy;
    }
  }
}
function triangle(
  ax: f64,
  ay: f64,
  bx: f64,
  by: f64,
  cx: f64,
  cy: f64,
  c: i32,
): void {
  const area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
  if (abs(area) < 0.01) return;
  const left = max(0, <i32>Math.floor(min(ax, min(bx, cx)))),
    right = min(319, <i32>Math.ceil(max(ax, max(bx, cx))));
  const top = max(0, <i32>Math.floor(min(ay, min(by, cy)))),
    bottom = min(239, <i32>Math.ceil(max(ay, max(by, cy))));
  for (let yy = top; yy <= bottom; yy++)
    for (let xx = left; xx <= right; xx++) {
      const a =
        ((bx - ax) * (<f64>yy + 0.5 - ay) - (by - ay) * (<f64>xx + 0.5 - ax)) /
        area;
      const b =
        ((cx - bx) * (<f64>yy + 0.5 - by) - (cy - by) * (<f64>xx + 0.5 - bx)) /
        area;
      const d =
        ((ax - cx) * (<f64>yy + 0.5 - cy) - (ay - cy) * (<f64>xx + 0.5 - cx)) /
        area;
      if (a >= 0 && b >= 0 && d >= 0) pixel(xx, yy, c);
    }
}
function quad(
  ax: f64,
  ay: f64,
  bx: f64,
  by: f64,
  cx: f64,
  cy: f64,
  ex: f64,
  ey: f64,
  c: i32,
): void {
  triangle(ax, ay, bx, by, cx, cy, c);
  triangle(ax, ay, cx, cy, ex, ey, c);
}
function screenX(xx: f64, yy: f64): f64 {
  return 160 + (xx - yy) * 19;
}
function screenY(xx: f64, yy: f64, z: f64 = 0): f64 {
  return 66 + (xx + yy) * 10 - z * 23;
}
function tile(xx: i32, yy: i32): bool {
  return (
    xx >= 0 &&
    yy >= 0 &&
    xx < 5 &&
    yy < 5 &&
    boards[level].charCodeAt(yy * 5 + xx) == 49
  );
}
function nextBottom(mx: i32, my: i32): i32 {
  return mx > 0
    ? orientation[2]
    : mx < 0
      ? orientation[3]
      : my > 0
        ? orientation[4]
        : orientation[5];
}
function turn(mx: i32, my: i32): void {
  const top = orientation[0],
    bottom = orientation[1];
  if (mx != 0) {
    const a = mx > 0 ? 2 : 3,
      b = mx > 0 ? 3 : 2;
    orientation[0] = orientation[b];
    orientation[1] = orientation[a];
    orientation[a] = top;
    orientation[b] = bottom;
  } else {
    const a = my > 0 ? 4 : 5,
      b = my > 0 ? 5 : 4;
    orientation[0] = orientation[b];
    orientation[1] = orientation[a];
    orientation[a] = top;
    orientation[b] = bottom;
  }
}
function move(mx: i32, my: i32): void {
  if (anim > 0 || won) return;
  if (!tile(x + mx, y + my)) {
    notice = 2;
    noticeTicks = 75;
    return;
  }
  if (x + mx == 4 && y + my == 4 && nextBottom(mx, my) != 0) {
    notice = 1;
    noticeTicks = 100;
    return;
  }
  if (historySize == 256) {
    for (let i = 0; i < 255 * 8; i++) history[i] = history[i + 8];
    historySize--;
  }
  const start = historySize * 8;
  history[start] = x;
  history[start + 1] = y;
  for (let i = 0; i < 6; i++) history[start + 2 + i] = orientation[i];
  historySize++;
  moves++;
  dx = mx;
  dy = my;
  anim = 1;
  noticeTicks = 0;
}
function undo(): void {
  if (historySize == 0) return;
  anim = 0;
  won = false;
  burstAge = -1;
  historySize--;
  moves--;
  const start = historySize * 8;
  x = history[start];
  y = history[start + 1];
  for (let i = 0; i < 6; i++) orientation[i] = history[start + 2 + i];
  noticeTicks = 0;
}
export function event(kind: i32): void {
  switch (kind) {
    case EventKind.KeyDown: {
      const key = event_get(Field.Key);
      if (key == Key.Z || key == Key.Backspace) {
        undo();
        return;
      }
      if (key == Key.R) {
        reset();
        return;
      }
      if (won && key == Key.Enter) {
        level = (level + 1) % 3;
        reset();
        return;
      }
      if (key == Key.Up || key == Key.W) move(0, -1);
      if (key == Key.Down || key == Key.S) move(0, 1);
      if (key == Key.Left || key == Key.A) move(-1, 0);
      if (key == Key.Right || key == Key.D) move(1, 0);
      break;
    }
    case EventKind.MouseDown: {
      if (event_get(Field.Button) != MouseButton.Primary) return;
      if (won) {
        level = (level + 1) % 3;
        reset();
        return;
      }
      const mx = <f64>event_get(Field.X),
        my = <f64>event_get(Field.Y);
      // Invert the ground-plane projection; each diamond is one logical tile.
      const a = (mx - 160) / 19,
        b = (my - 66) / 10;
      const tx = <i32>Math.floor((a + b) * 0.5),
        ty = <i32>Math.floor((b - a) * 0.5);
      if (abs(tx - x) + abs(ty - y) == 1) move(tx - x, ty - y);
      break;
    }
    default:
      // Movement is discrete: the site has no held-key state to clear on reset.
      break;
  }
}
function drawTile(xx: i32, yy: i32): void {
  const a = screenX(xx, yy),
    b = screenY(xx, yy),
    c = a + 19,
    d = b + 10,
    e = a,
    f = b + 20,
    g = a - 19;
  quad(g, d, e, f, e, f + 6, g, d + 6, 0x263f4c);
  quad(e, f, c, d, c, d + 6, e, f + 6, 0x385561);
  const hole = xx == 4 && yy == 4;
  const near = abs(xx - x) + abs(yy - y) == 1;
  const color = hole
    ? 0x101f2b
    : near && !won
      ? 0x678d88
      : (xx + yy) % 2 == 0
        ? 0x506c73
        : 0x47646d;
  quad(a, b, c, d, e, f, g, d, color);
  line(<i32>a, <i32>b, <i32>c, <i32>d, 0x83a19c);
  line(<i32>a, <i32>b, <i32>g, <i32>d, 0x83a19c);
  if (hole) {
    quad(a, b + 4, c - 5, d, e, f - 4, g + 5, d, 0x08131e);
    line(<i32>a, <i32>b, <i32>c, <i32>d, 0xf2c66d);
    line(<i32>c, <i32>d, <i32>e, <i32>f, 0xc18b45);
    line(<i32>e, <i32>f, <i32>g, <i32>d, 0xc18b45);
    line(<i32>g, <i32>d, <i32>a, <i32>b, 0xf2c66d);
  }
}
function drawCube(): void {
  let t = anim == 0 ? 0 : <f64>(anim - 1) / 12;
  t = t * t * (3 - 2 * t);
  const angle = t * Math.PI * 0.5,
    cs = Math.cos(angle),
    sn = Math.sin(angle);
  const sink = won ? min(1.0, <f64>(anim - 1) / 24) * 1.3 : 0.0;
  for (let i = 0; i < 8; i++) {
    let xx = <f64>x + <f64>(i & 1),
      yy = <f64>y + <f64>((i >> 1) & 1),
      zz = <f64>((i >> 2) & 1);
    if (!won && anim > 0) {
      if (dx != 0) {
        const pivot = <f64>x + (dx > 0 ? 1 : 0),
          a = xx - pivot;
        xx = pivot + a * cs + zz * sn * dx;
        zz = -a * sn * dx + zz * cs;
      } else {
        const pivot = <f64>y + (dy > 0 ? 1 : 0),
          a = yy - pivot;
        yy = pivot + a * cs + zz * sn * dy;
        zz = -a * sn * dy + zz * cs;
      }
    }
    zz -= sink;
    vx[i] = xx;
    vy[i] = yy;
    vz[i] = zz;
  }
  for (let f = 0; f < 6; f++) {
    order[f] = f;
    let depth = 0.0;
    for (let j = 0; j < 4; j++) {
      const v = faces[f * 4 + j];
      depth += vx[v] + vy[v] + vz[v] * 0.87;
    }
    depths[f] = depth;
  }
  for (let i = 1; i < 6; i++) {
    const f = order[i];
    let j = i;
    while (j > 0 && depths[order[j - 1]] > depths[f]) {
      order[j] = order[j - 1];
      j--;
    }
    order[j] = f;
  }
  for (let i = 0; i < 6; i++) {
    const f = order[i],
      a = faces[f * 4],
      b = faces[f * 4 + 1],
      c = faces[f * 4 + 2],
      d = faces[f * 4 + 3];
    paintCubeFace(a, b, c, d, faceColors[f]);
    if (orientation[f] == 0) paintCubeFace(a, b, c, d, 0xf2c66d, 0.55, false);
  }
}
// Clip in world space, before projection. Merely drawing the rim later cannot
// hide faces that have sunk below the platform and would spill over its front.
function paintCubeFace(
  a: i32,
  b: i32,
  c: i32,
  d: i32,
  color: i32,
  inset: f64 = 1,
  outline: bool = true,
): void {
  corners[0] = a;
  corners[1] = b;
  corners[2] = c;
  corners[3] = d;
  const cx = (vx[a] + vx[b] + vx[c] + vx[d]) * 0.25;
  const cy = (vy[a] + vy[b] + vy[c] + vy[d]) * 0.25;
  const cz = (vz[a] + vz[b] + vz[c] + vz[d]) * 0.25;
  let count = 0;
  for (let i = 0; i < 4; i++) {
    const prev = corners[(i + 3) % 4],
      next = corners[i];
    const ax = cx + (vx[prev] - cx) * inset,
      ay = cy + (vy[prev] - cy) * inset;
    const az = cz + (vz[prev] - cz) * inset;
    const bx = cx + (vx[next] - cx) * inset,
      by = cy + (vy[next] - cy) * inset;
    const bz = cz + (vz[next] - cz) * inset;
    if (az >= 0 != bz >= 0) {
      const t = az / (az - bz);
      clippedX[count] = screenX(ax + (bx - ax) * t, ay + (by - ay) * t);
      clippedY[count] = screenY(ax + (bx - ax) * t, ay + (by - ay) * t);
      count++;
    }
    if (bz >= 0) {
      clippedX[count] = screenX(bx, by);
      clippedY[count] = screenY(bx, by, bz);
      count++;
    }
  }
  for (let i = 1; i + 1 < count; i++) {
    triangle(
      clippedX[0],
      clippedY[0],
      clippedX[i],
      clippedY[i],
      clippedX[i + 1],
      clippedY[i + 1],
      color,
    );
  }
  if (outline)
    for (let i = 0; i < count; i++) {
      const j = (i + 1) % count;
      line(
        <i32>clippedX[i],
        <i32>clippedY[i],
        <i32>clippedX[j],
        <i32>clippedY[j],
        0x263f4c,
      );
    }
}

function startBurst(): void {
  burstAge = 0;
  for (let i = 0; i < 48; i++) {
    const angle = <f64>i * 2.399963; // Evenly spread, deterministic sparks.
    const speed = 0.65 + <f64>((i * 17) % 19) * 0.085;
    particleX[i] = 160 + Math.cos(angle) * 5;
    particleY[i] = 153 + Math.sin(angle) * 2;
    particleVX[i] = Math.cos(angle) * speed;
    particleVY[i] = -1.5 - <f64>((i * 11) % 23) * 0.105;
  }
}
function drawParticles(): void {
  if (burstAge < 0) return;
  for (let i = 0; i < 48; i++) {
    const lifetime = 36 + ((i * 13) % 35);
    if (burstAge >= lifetime) continue;
    particleX[i] += particleVX[i];
    particleY[i] += particleVY[i];
    particleVY[i] += 0.085;
    const xx = <i32>Math.round(particleX[i]),
      yy = <i32>Math.round(particleY[i]);
    if (yy < 65 || yy >= 188) continue;
    const fading = burstAge > lifetime - 12;
    const color = fading
      ? 0x807c63
      : i % 3 == 0
        ? 0x8ee3c2
        : i % 3 == 1
          ? 0xfff1c2
          : 0xf8d978;
    pixel(xx, yy, color);
    if (!fading && i % 4 == 0) {
      pixel(xx - 1, yy, color);
      pixel(xx + 1, yy, color);
      pixel(xx, yy - 1, color);
      pixel(xx, yy + 1, color);
    } else if (!fading && i % 3 == 0) pixel(xx, yy + 1, color);
  }
  burstAge++;
  if (burstAge >= 71) burstAge = -1;
}
function drawFrontRim(): void {
  // The near lip sits in front of the falling cube.
  line(179, 156, 160, 166, 0xc18b45);
  line(160, 166, 141, 156, 0xc18b45);
}

export function render(): void {
  for (let yy = 0; yy < 240; yy++)
    for (let xx = 0; xx < 320; xx++)
      pixel(xx, yy, yy < 190 ? 0x101e2b : 0x142632);
  for (let i = 0; i < 40; i++)
    pixel((i * 97 + 13) % 320, 40 + ((i * 37) % 142), 0x203744);
  label(16, 12, title, 0xe1e8d9);
  label(16, 27, rule, 0xc9a766);
  label(16, 49, labels[level], 0x8faaa9);
  number(280, 49, moves);
  for (let depth = 0; depth <= 8; depth++)
    for (let xx = 0; xx < 5; xx++) {
      const yy = depth - xx;
      if (tile(xx, yy)) drawTile(xx, yy);
    }
  drawCube();
  drawParticles();
  // While tipping onto the tile the cube is above the lip; once sinking it
  // passes behind the lip, with below-ground geometry already clipped away.
  if (won) drawFrontRim();
  label(
    16,
    194,
    won ? solved : noticeTicks > 0 ? (notice == 1 ? wrong : edge) : empty,
    won ? 0xf2c66d : 0xa5b9b4,
  );
  label(16, 214, help, 0xc5d4cb);
  label(16, 227, clickHelp, 0x7f9c9c);
  if (noticeTicks > 0) noticeTicks--;
  if (anim > 0) {
    anim++;
    if (!won && anim > 13) {
      x += dx;
      y += dy;
      turn(dx, dy);
      anim = 0;
      if (x == 4 && y == 4) {
        won = true;
        anim = 1;
        startBurst();
      }
    } else if (won && anim > 25) anim = 25;
  }
}
