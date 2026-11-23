use minifb::Window;

/// Window dimensions are logical points; the presentation buffer uses physical pixels.
#[derive(Clone, Copy)]
pub struct Viewport {
    pub width: usize,
    pub height: usize,
    pub scale: f64,
}

impl Viewport {
    pub fn new(width: usize, height: usize, scale: f64) -> Self {
        let scale = if scale.is_finite() && scale >= 1.0 {
            scale
        } else {
            1.0
        };
        Self {
            width: (width as f64 * scale).round() as usize,
            height: (height as f64 * scale).round() as usize,
            scale,
        }
    }

    pub fn pixels(self, points: usize) -> usize {
        (points as f64 * self.scale).round() as usize
    }
}

pub fn viewport(window: &Window) -> Viewport {
    let (width, height) = window.get_size();
    Viewport::new(width, height, backing_scale(window))
}

#[cfg(target_os = "macos")]
fn backing_scale(window: &Window) -> f64 {
    use objc2::{msg_send, runtime::AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
            // The borrowed handle guarantees a live NSView. All calls happen on
            // the same main thread that owns minifb's window. No objects escape.
            unsafe {
                let view = handle.ns_view.as_ptr().cast::<AnyObject>();
                let native_window: *mut AnyObject = msg_send![view, window];
                if !native_window.is_null() {
                    return msg_send![native_window, backingScaleFactor];
                }
            }
        }
    }
    1.0
}

#[cfg(not(target_os = "macos"))]
fn backing_scale(_window: &Window) -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_points_to_backing_pixels_without_changing_logical_size() {
        let standard = Viewport::new(960, 720, 1.0);
        let retina = Viewport::new(960, 720, 2.0);
        assert_eq!((standard.width, standard.height), (960, 720));
        assert_eq!((retina.width, retina.height), (1920, 1440));
        assert_eq!(retina.pixels(30), 60);
        assert_eq!(Viewport::new(960, 720, f64::NAN).width, 960);
    }
}
