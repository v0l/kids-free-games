#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    pub fn scaled(self, k: f32) -> Insets {
        Insets {
            left: self.left * k,
            top: self.top * k,
            right: self.right * k,
            bottom: self.bottom * k,
        }
    }
}

pub struct SafeArea {
    cached: Insets,
    size: (f32, f32),
    frames: u32,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            cached: Insets::default(),
            size: (0.0, 0.0),
            frames: u32::MAX,
        }
    }
}

impl SafeArea {
    pub fn get(&mut self, screen_px: (f32, f32), read: impl FnOnce() -> Insets) -> Insets {
        if screen_px != self.size {
            self.size = screen_px;
            self.frames = 0;
        }
        if self.frames < 90 && self.frames.is_multiple_of(15) {
            self.cached = read();
        }
        self.frames = self.frames.saturating_add(1);
        self.cached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_triggers_a_fresh_read() {
        let mut s = SafeArea::default();
        let mut reads = 0;
        s.get((1000.0, 2000.0), || {
            reads += 1;
            Insets::default()
        });
        assert_eq!(s.frames, 1);
        for _ in 0..200 {
            s.get((1000.0, 2000.0), || {
                reads += 1;
                Insets::default()
            });
        }
        let settled = s.frames;
        assert_eq!(reads, 6);
        s.get((2000.0, 1000.0), || Insets {
            top: 5.0,
            ..Default::default()
        });
        assert_eq!(s.frames, 1);
        assert_eq!(s.cached.top, 5.0);
        assert!(settled > 90);
    }
}
