// The complete v2 import surface. These declarations add no runtime or objects.
@external("smolbrowse", "pixel")
export declare function pixel(x: i32, y: i32, color: i32): void;
@external("smolbrowse", "text")
export declare function text(x: i32, y: i32, ptr: usize, length: i32, color: i32): void;
@external("smolbrowse", "event_get")
export declare function event_get(field: i32): i32;

export enum EventKind {
  MouseMove = 1,
  MouseDown = 2,
  MouseUp = 3,
  KeyDown = 4,
  KeyUp = 5,
  TextInput = 6,
  InputReset = 7,
}
export enum Field {
  X = 1,
  Y = 2,
  Button = 3,
  Key = 4,
  Codepoint = 5,
  Modifiers = 6,
}
export enum Key {
  A = 65,
  D = 68,
  R = 82,
  S = 83,
  W = 87,
  Z = 90,
  Enter = 256,
  Backspace = 258,
  Left = 260,
  Right = 261,
  Up = 262,
  Down = 263,
}
export enum MouseButton { Primary = 1, Secondary = 2, Middle = 3 }
