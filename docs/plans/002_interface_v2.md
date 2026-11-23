# Interface v2: input

The [reference implementation](../../ref/README.md) implements v2 and remains
compatible with [v1](001_interface_v1.md).

V2 adds input to the same small canvas. Sites still draw with `pixel` and `text`
and export `render`. There is one optional event handler and one import for
reading its details. No listeners, event objects, allocation, or shared memory
layout are required. Navigation, networking, audio, and additional drawing
primitives remain outside this version.

Everything in v1 applies except where this document extends it. V1 sites run
unchanged on a v2 host; a site using the new import requires a v2 host.

## Wasm contract

```wat
(import "smolbrowse" "event_get" (func $event_get (param i32) (result i32)))
(func (export "event") (param $kind i32)
  ;; Switch on kind; read details with event_get.
)
```

Equivalent C declarations, using the same import convention as v1:

```c
// Host import.
int32_t event_get(int32_t field);

// Optional site export.
void event(int32_t kind);
```

`kind` is the event ID: a small enum identifying what happened. There is no
separate ID for an individual occurrence. Only one event is active at a time;
`event_get` reads that event's immutable details until the handler returns.
Sites copy values into their own state if they need them later.

Unknown fields, fields absent from the current event, and calls outside an
event handler return zero. Unknown event kinds must be ignored by sites, so
future versions can add kinds without changing the handler signature.

Omitting `event` means the site receives no input. If present, it must have
signature `(i32) -> ()`; an incompatible export is a page failure. Importing
`event_get` does not require exported memory.

## Events and fields

Event IDs and field IDs are separate enums. Their numeric values are fixed.
Zero is reserved in both enums.

| Event ID | Name | Available fields |
| --- | --- | --- |
| 1 | `MOUSE_MOVE` | `X`, `Y`, `MODIFIERS` |
| 2 | `MOUSE_DOWN` | `X`, `Y`, `BUTTON`, `MODIFIERS` |
| 3 | `MOUSE_UP` | `X`, `Y`, `BUTTON`, `MODIFIERS` |
| 4 | `KEY_DOWN` | `KEY`, `MODIFIERS` |
| 5 | `KEY_UP` | `KEY`, `MODIFIERS` |
| 6 | `TEXT_INPUT` | `CODEPOINT`, `MODIFIERS` |
| 7 | `INPUT_RESET` | None |

| Field ID | Name | Value |
| --- | --- | --- |
| 1 | `X` | Signed canvas x coordinate |
| 2 | `Y` | Signed canvas y coordinate |
| 3 | `BUTTON` | 1 = primary, 2 = secondary, 3 = middle |
| 4 | `KEY` | Key code defined below |
| 5 | `CODEPOINT` | One committed Unicode scalar value |
| 6 | `MODIFIERS` | Bitmask: Shift = 1, Control = 2, Alt = 4, Super/Command = 8 |

Modifier bits describe the state after the transition for key events, and at
the time of the event otherwise. Left and right modifier keys share a bit.

## Mouse

Coordinates use the canvas's 320 × 240 coordinate system, independent of window
size or backing scale. The host removes scaling and letterboxing, then rounds
down to the containing logical pixel. A mouse down is delivered only when it
starts inside the canvas; clicks on browser controls or letterboxing do not
reach the site. Button numbers follow the user's primary/secondary preference.

Once a site receives a mouse down, the host continues delivering movement and
the corresponding mouse up even outside the canvas, until all delivered
buttons are released. Coordinates outside the canvas remain signed and
unclamped, so a site can distinguish releasing outside from releasing on an
edge. Without a held button, movement is delivered only inside the canvas.
V2 does not provide hover enter/leave events.

A click is a mouse down followed by a mouse up. Sites decide what counts as
activation, dragging, or a double click; the host does not emit separate click
events. Only the three listed buttons are exposed. Wheel input, multiple
pointers, pressure, gestures, and pointer locking are left for later versions.

## Keyboard and text

Key events describe controls; text events describe writing. Sites must not
derive typed text from key codes. A key press may produce both a `KEY_DOWN`
and one or more `TEXT_INPUT` events, or no text at all.

Key codes identify physical positions on a US keyboard, independent of the
active text layout. Letters use ASCII `A`–`Z` (65–90), digits use `0`–`9`
(48–57), and punctuation uses its unshifted ASCII value on that keyboard:
`` ` - = [ ] \ ; ' , . / ``. Space is 32. Shift does not change a key code.

| Code | Key |
| --- | --- |
| 256 | Enter |
| 257 | Escape |
| 258 | Backspace |
| 259 | Tab |
| 260 | Left |
| 261 | Right |
| 262 | Up |
| 263 | Down |
| 264 | Delete |
| 265 | Home |
| 266 | End |
| 267 | Page Up |
| 268 | Page Down |
| 269 | Shift |
| 270 | Control |
| 271 | Alt |
| 272 | Super/Command |

Unlisted keys are not delivered. Left/right modifiers and keypad equivalents
are merged into their listed key: a down occurs when the first equivalent is
pressed, and an up when the last is released. Each key has one down/up pair;
OS key repeat does not generate extra key downs. Sites can implement held-key
behavior using their render ticks.

`TEXT_INPUT` supplies committed text in order, one Unicode scalar per event,
including OS text repeat. Composition previews are not exposed. Control
characters U+0000–U+001F and U+007F–U+009F are excluded; Enter, Tab, and editing
commands use key events. Input does not extend v1's font coverage: a site can
store Unicode even though the built-in font displays unsupported characters
as `?`. There is no clipboard import or automatic paste behavior in v2.

## Delivery and focus

The host first runs startup and a successful initial `render`. It then delivers
pending events in observed order before each subsequent `render`. Handlers
run synchronously, without nesting or overlap with rendering. Events arriving
during a callback wait for a later batch. Input does not add animation ticks.

Handlers update site state; drawing belongs in `render`. Drawing calls during
an event handler do not affect the displayed canvas and are cleared before the
next render. Only a successful render commits a frame, as in v1.

Hosts may coalesce consecutive mouse moves into the latest move, but must not
coalesce across another event. Key, text, and button transitions retain their
order. Hosts may bound event queues and execution; exhaustion is a page
failure rather than silently losing transitions. A handler trap stops both
input and rendering, reports a page failure, and leaves the last successful
frame visible. Input cannot bypass host resource limits.

Only the focused page receives input. Browser UI and OS shortcuts take
precedence, and their input is consumed by the host. When the page loses input
focus, including when browser controls open, pending site events are discarded
and the host delivers `INPUT_RESET` before any later site input or render.
Sites must clear held keys, held buttons, and gestures on reset. If a host
cannot continue a mouse capture, it uses the same reset rule.

On resuming, the site starts with no held inputs. A key or button already held
must be released and pressed again before generating site input; unmatched up
events and text repeat from those held keys are suppressed. Reload and
navigation discard queued events from the old instance.

Browser controls must leave ordinary typing and gameplay keys available to
interactive sites. In particular, the reference viewer's v1 bare `q`, `r`,
and `:` shortcuts cannot retain their current meaning while a site with an
`event` export has focus. A host must keep a way to focus browser controls
that a site cannot intercept; the exact shortcut is a host choice.

## Example

This C-style sketch moves a mark to each primary-button press. The enum names
are site-side constants with the values above; they add no ABI functions.
`render` continues to draw the entire frame.

```c
static int32_t mark_x = 160, mark_y = 120;

void event(int32_t kind) {
    switch (kind) {
    case MOUSE_DOWN:
        if (event_get(BUTTON) == 1) {
            mark_x = event_get(X);
            mark_y = event_get(Y);
        }
        break;
    default:
        break;
    }
}

void render(void) {
    pixel(mark_x, mark_y, 0xffffff);
}
```
