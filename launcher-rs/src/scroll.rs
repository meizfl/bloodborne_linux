//! The accepted native Wayland scroll behavior, independent of GUI timing.
#[derive(Debug, Default, Clone)]
pub struct Spring {
    pub remaining: f64,
    pub velocity: f64,
    fraction: f64,
}
impl Spring {
    pub fn input(&mut self, pixels: f64) {
        self.remaining += pixels;
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
    pub fn active(&self) -> bool {
        self.remaining.abs() > 0.05 || self.velocity.abs() > 1.0
    }
    pub fn step(&mut self, elapsed: f64) -> i32 {
        let elapsed = elapsed.clamp(0.001, 0.05);
        let omega = 18.0;
        let coefficient = self.velocity - omega * self.remaining;
        let decay = (-omega * elapsed).exp();
        let next = (self.remaining - coefficient * elapsed) * decay;
        self.velocity = (self.velocity - omega * coefficient * elapsed) * decay;
        self.fraction += self.remaining - next;
        self.remaining = next;
        if !self.active() {
            self.fraction += self.remaining;
            self.remaining = 0.0;
            self.velocity = 0.0;
        }
        let pixels = self.fraction as i32;
        self.fraction -= f64::from(pixels);
        pixels
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starts_gently_and_accumulates_without_velocity_jumps() {
        let mut spring = Spring::default();
        spring.input(64.0);
        assert_eq!(spring.step(0.008), 0);
        let velocity = spring.velocity;
        spring.input(64.0);
        assert_eq!(spring.velocity, velocity);
        let mut pixels = 0;
        for _ in 0..200 {
            pixels += spring.step(0.008);
        }
        assert!(!spring.active());
        assert!((f64::from(pixels) + spring.fraction - 128.0).abs() < 1e-8);
    }
    #[test]
    fn stalled_frame_does_not_teleport_to_the_target() {
        let mut spring = Spring::default();
        spring.input(64.0);
        assert!(spring.step(2.0) < 32);
        spring.cancel();
        assert_eq!(spring.step(0.016), 0);
    }
}
