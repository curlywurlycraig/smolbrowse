use font8x8::{UnicodeFonts, BASIC_FONTS};

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;

pub struct Framebuffer {
    pub pixels: Vec<u32>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }
}

impl Framebuffer {
    pub fn pixel(&mut self, x: i64, y: i64, color: u32) {
        if (0..WIDTH as i64).contains(&x) && (0..HEIGHT as i64).contains(&y) {
            self.pixels[y as usize * WIDTH + x as usize] = color & 0x00ff_ffff;
        }
    }

    pub fn text(&mut self, x: i32, y: i32, text: &str, color: u32) {
        // Wider coordinates prevent wrapping near i32::MAX or after long lines.
        let (mut cursor_x, mut cursor_y) = (i64::from(x), i64::from(y));
        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = i64::from(x);
                cursor_y += 8;
                continue;
            }
            let ch = if (' '..='~').contains(&ch) { ch } else { '?' };
            let glyph = BASIC_FONTS.get(ch).expect("ASCII font is complete");
            for (row, bits) in glyph.iter().enumerate() {
                for column in 0..8 {
                    if bits & (1 << column) != 0 {
                        self.pixel(cursor_x + column, cursor_y + row as i64, color);
                    }
                }
            }
            cursor_x += 8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_and_masks_pixels() {
        let mut frame = Framebuffer::default();
        for (x, y) in [(-1, 0), (320, 0), (0, -1), (0, 240)] {
            frame.pixel(x, y, 0xffffff);
        }
        assert!(frame.pixels.iter().all(|p| *p == 0));
        frame.pixel(319, 239, 0xff123456);
        assert_eq!(frame.pixels[WIDTH * HEIGHT - 1], 0x123456);
    }

    #[test]
    fn text_has_transparent_background_and_fixed_cells() {
        let mut frame = Framebuffer::default();
        frame.pixels.fill(0x123456);
        frame.text(3, 2, "A A\nA", 0xffffff);
        // The font's A has a centered stem on the first row (bits 2 and 3).
        assert_eq!(frame.pixels[2 * WIDTH + 5], 0xffffff);
        assert_eq!(frame.pixels[2 * WIDTH + 3], 0x123456);
        for row in 2..10 {
            assert!(frame.pixels[row * WIDTH + 11..row * WIDTH + 19]
                .iter()
                .all(|p| *p == 0x123456));
        }
        for row in 0..8 {
            assert_eq!(
                &frame.pixels[(2 + row) * WIDTH + 3..(2 + row) * WIDTH + 11],
                &frame.pixels[(10 + row) * WIDTH + 3..(10 + row) * WIDTH + 11]
            );
        }
    }

    #[test]
    fn unsupported_characters_use_one_replacement_cell() {
        let mut actual = Framebuffer::default();
        let mut expected = Framebuffer::default();
        actual.text(-2, -2, "é\t", 1);
        expected.text(-2, -2, "??", 1);
        assert_eq!(actual.pixels, expected.pixels);
        actual.text(i32::MAX, i32::MAX, "overflow\ncheck", 1);
        assert_eq!(actual.pixels, expected.pixels);
    }
}
