use anyhow::{anyhow, Result};
use fontdue::{Font, FontSettings, Metrics};
use std::{collections::HashMap, fs};

use crate::{
    display::Viewport,
    framebuffer::{Framebuffer, HEIGHT, WIDTH},
};

const LINE_HEIGHT: usize = 30;

#[derive(Default)]
pub struct AddressBar {
    chars: Vec<char>,
    cursor: usize,
    pub focused: bool,
    selected: bool,
}

impl AddressBar {
    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn set(&mut self, value: &str) {
        self.chars = value.chars().take(4096).collect();
        self.cursor = self.chars.len();
        self.selected = false;
    }

    pub fn select_all(&mut self) {
        self.focused = true;
        self.selected = true;
        self.cursor = self.chars.len();
    }

    fn delete_selection(&mut self) -> bool {
        if !self.selected {
            return false;
        }
        self.chars.clear();
        self.cursor = 0;
        self.selected = false;
        true
    }

    pub fn insert(&mut self, value: &str) {
        self.delete_selection();
        for ch in value.chars().filter(|ch| !ch.is_control()) {
            if self.chars.len() == 4096 {
                break;
            }
            self.chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if !self.delete_selection() && self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if !self.delete_selection() && self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn move_cursor(&mut self, step: isize) {
        if self.selected {
            self.cursor = if step < 0 { 0 } else { self.chars.len() };
        } else {
            self.cursor = self
                .cursor
                .saturating_add_signed(step)
                .min(self.chars.len());
        }
        self.selected = false;
    }

    pub fn home(&mut self) {
        self.cursor = 0;
        self.selected = false;
    }
    pub fn end(&mut self) {
        self.cursor = self.chars.len();
        self.selected = false;
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Open(String),
    Reload,
    Quit,
    Cancel,
}

pub fn parse_command(value: &str) -> Result<Command> {
    let value = value.trim();
    match value {
        "" => Ok(Command::Cancel),
        "q" | "quit" => Ok(Command::Quit),
        "r" | "reload" => Ok(Command::Reload),
        "open" | "o" | "edit" | "e" => Err(anyhow!("Usage: open <path-or-url>")),
        _ => {
            let address = ["open ", "o ", "edit ", "e "]
                .iter()
                .find_map(|prefix| value.strip_prefix(prefix))
                .unwrap_or(value)
                .trim();
            Ok(Command::Open(address.to_string()))
        }
    }
}

pub struct UiFont {
    font: Font,
    glyphs: HashMap<char, (Metrics, Vec<u8>)>,
    cell: usize,
    scale: f64,
}

impl UiFont {
    pub fn load() -> Result<Self> {
        let bytes = [
            "/System/Library/Fonts/Menlo.ttc",
            "/System/Library/Fonts/SFNSMono.ttf",
        ]
        .iter()
        .find_map(|path| fs::read(path).ok())
        .ok_or_else(|| anyhow!("Could not find the macOS monospace font"))?;
        let font =
            Font::from_bytes(bytes, FontSettings::default()).map_err(|error| anyhow!(error))?;
        let cell = font.metrics('M', 14.0).advance_width.ceil() as usize;
        Ok(Self {
            font,
            glyphs: HashMap::new(),
            cell,
            scale: 1.0,
        })
    }

    fn set_scale(&mut self, scale: f64) {
        if self.scale != scale {
            self.scale = scale;
            self.glyphs.clear();
            let logical_cell = self.font.metrics('M', 14.0).advance_width.ceil();
            self.cell = (f64::from(logical_cell) * scale).round() as usize;
        }
    }

    fn text(&mut self, surface: &mut Surface, x: usize, y: usize, text: &str, color: u32) {
        let mut x = x;
        for ch in text.chars() {
            if x + self.cell > surface.width {
                break;
            }
            let (metrics, bitmap) = self
                .glyphs
                .entry(ch)
                .or_insert_with(|| self.font.rasterize(ch, 14.0 * self.scale as f32));
            let top = y as i64 + (16.0 * self.scale).round() as i64
                - metrics.height as i64
                - i64::from(metrics.ymin);
            let left = x as i64 + i64::from(metrics.xmin);
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let (px, py) = (left + col as i64, top + row as i64);
                    if px < 0 || py < 0 || px >= surface.width as i64 || py >= surface.height as i64
                    {
                        continue;
                    }
                    let alpha = u32::from(bitmap[row * metrics.width + col]);
                    let index = py as usize * surface.width + px as usize;
                    let background = surface.pixels[index];
                    let mut mixed = 0;
                    for shift in [0, 8, 16] {
                        let fg = (color >> shift) & 255;
                        let bg = (background >> shift) & 255;
                        mixed |= ((fg * alpha + bg * (255 - alpha)) / 255) << shift;
                    }
                    surface.pixels[index] = mixed;
                }
            }
            x += self.cell;
        }
    }
}

struct Surface {
    pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl Surface {
    fn rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        for row in y..(y + height).min(self.height) {
            let start = x.min(self.width);
            let end = (x + width).min(self.width);
            self.pixels[row * self.width + start..row * self.width + end].fill(color);
        }
    }
}

pub fn compose(
    viewport: Viewport,
    bar: &AddressBar,
    status: &str,
    frame: Option<&Framebuffer>,
    font: &mut UiFont,
) -> Vec<u32> {
    let (width, height) = (viewport.width, viewport.height);
    let px = |points| viewport.pixels(points);
    font.set_scale(viewport.scale);
    let mut surface = Surface {
        pixels: vec![0; width * height],
        width,
        height,
    };
    if let Some(frame) = frame {
        let scale = (width as f64 / WIDTH as f64).min(height as f64 / HEIGHT as f64);
        let draw_width = (WIDTH as f64 * scale) as usize;
        let draw_height = (HEIGHT as f64 * scale) as usize;
        let left = (width - draw_width) / 2;
        let top = (height - draw_height) / 2;
        for y in 0..draw_height {
            for x in 0..draw_width {
                surface.pixels[(top + y) * width + left + x] =
                    frame.pixels[(y * HEIGHT / draw_height) * WIDTH + x * WIDTH / draw_width];
            }
        }
    } else if width > px(320) && height > px(100) {
        font.text(&mut surface, px(24), height / 2, "smolbrowse", 0xbcbcbc);
        font.text(
            &mut surface,
            px(24),
            height / 2 + px(26),
            ":open <path-or-url>   |   Cmd+L",
            0x777777,
        );
    }
    // Commands overlay the canvas; entering command mode never resizes the site.
    let line_height = px(LINE_HEIGHT);
    let top = height.saturating_sub(line_height);
    if bar.focused {
        surface.rect(0, top, width, line_height, 0x171717);
        let cells = width.saturating_sub(px(36)) / font.cell;
        let start = bar.cursor.saturating_sub(cells.saturating_sub(1));
        let visible: String = bar.chars.iter().skip(start).take(cells).collect();
        if bar.selected {
            surface.rect(
                px(22),
                top + px(5),
                visible.chars().count() * font.cell,
                px(20),
                0x414141,
            );
        }
        font.text(&mut surface, px(8), top + px(5), ":", 0xdddddd);
        font.text(&mut surface, px(22), top + px(5), &visible, 0xdddddd);
        if !bar.selected && cells > 0 {
            surface.rect(
                px(22) + (bar.cursor - start) * font.cell,
                top + px(6),
                px(1),
                px(18),
                0xdddddd,
            );
        }
    } else if !status.is_empty() {
        surface.rect(0, top, width, line_height, 0x171717);
        font.text(&mut surface, px(8), top + px(5), status, 0xbcbcbc);
    }
    surface.pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_paths_and_replaces_selected_address() {
        let mut bar = AddressBar::default();
        bar.set("/tmp/café.wasm");
        bar.home();
        bar.move_cursor(9);
        bar.backspace();
        assert_eq!(bar.value(), "/tmp/caf.wasm");
        bar.insert("é");
        assert_eq!(bar.value(), "/tmp/café.wasm");
        bar.select_all();
        bar.insert("https://example.org/cube.wasm\n");
        assert_eq!(bar.value(), "https://example.org/cube.wasm");
        bar.home();
        bar.delete();
        assert!(bar.value().starts_with("ttps"));
    }

    #[test]
    fn hidden_chrome_uses_entire_viewport_and_prompt_is_an_overlay() {
        let mut font = UiFont::load().unwrap();
        let mut frame = Framebuffer::default();
        frame.pixels.fill(0xff1234);
        let mut bar = AddressBar::default();
        let hidden = compose(
            Viewport::new(640, 480, 1.0),
            &bar,
            "",
            Some(&frame),
            &mut font,
        );
        assert!(hidden.iter().all(|pixel| *pixel == 0xff1234));
        bar.focused = true;
        let shown = compose(
            Viewport::new(640, 480, 1.0),
            &bar,
            "",
            Some(&frame),
            &mut font,
        );
        assert_eq!(&shown[..450 * 640], &hidden[..450 * 640]);
        assert_ne!(shown[479 * 640], hidden[479 * 640]);
        for (w, h) in [(1, 1), (100, 50), (320, 30)] {
            assert_eq!(
                compose(Viewport::new(w, h, 1.0), &bar, "", Some(&frame), &mut font).len(),
                w * h
            );
        }
    }

    #[test]
    fn retina_glyphs_use_native_pixels_while_logical_geometry_stays_the_same() {
        let mut font = UiFont::load().unwrap();
        let mut bar = AddressBar::default();
        bar.set("open /tmp/café.wasm");
        bar.focused = true;
        let standard = compose(Viewport::new(320, 240, 1.0), &bar, "", None, &mut font);
        let cell = font.cell;
        let retina = compose(Viewport::new(320, 240, 2.0), &bar, "", None, &mut font);
        assert_eq!(font.cell, cell * 2);
        assert_eq!(
            font.glyphs[&'o'].0.height,
            font.font.metrics('o', 28.0).height
        );
        // Native glyph rasterization contains detail within a logical pixel,
        // unlike a nearest-neighbor enlargement of the low-resolution UI.
        assert!((420..480).any(|y| (0..640)
            .step_by(2)
            .any(|x| retina[y * 640 + x] != retina[y * 640 + x + 1])));
        assert_eq!(standard.len() * 4, retina.len());
        compose(Viewport::new(320, 240, 1.0), &bar, "", None, &mut font);
        assert_eq!(font.cell, cell);
        assert_eq!(
            font.glyphs[&'o'].0.height,
            font.font.metrics('o', 14.0).height
        );
    }

    #[test]
    fn parses_commands_and_preserves_spaces_in_file_paths() {
        assert_eq!(
            parse_command("open /tmp/my site.wasm").unwrap(),
            Command::Open("/tmp/my site.wasm".into())
        );
        assert_eq!(
            parse_command("https://example.org/page.wasm").unwrap(),
            Command::Open("https://example.org/page.wasm".into())
        );
        assert_eq!(parse_command("q").unwrap(), Command::Quit);
        assert_eq!(parse_command("reload").unwrap(), Command::Reload);
        assert_eq!(parse_command("").unwrap(), Command::Cancel);
        assert!(parse_command("open").is_err());
    }
}
