use std::cell::Cell;
use std::time::Duration;

/// Controls the speed/easing of the glide in the exponential-decay formula.
const DECAY_RATE: f64 = 21.0;
/// Distance from the target below which the animation snaps to it.
const SETTLE_EPSILON: f64 = 0.01;

#[derive(Debug)]
pub struct ScrollAnimator {
    current: Cell<f64>,
    target: Cell<f64>,
}

impl ScrollAnimator {
    pub fn new() -> Self {
        Self {
            current: Cell::new(0.0),
            target: Cell::new(0.0),
        }
    }

    pub fn current(&self) -> f64 {
        self.current.get()
    }

    pub fn target(&self) -> f64 {
        self.target.get()
    }

    pub fn set_target(&self, target: f64) {
        self.target.set(target);
    }

    pub fn snap_to(&self, value: f64) {
        self.current.set(value);
        self.target.set(value);
    }

    pub fn tick(&self, dt: Duration) {
        let current = self.current.get();
        let target = self.target.get();
        if (current - target).abs() < SETTLE_EPSILON {
            self.current.set(target);
        } else {
            // Exponential decay formula: current = current + (target - current) * (1.0 - exp(-decay * dt))
            let dt_secs = dt.as_secs_f64();
            let factor = 1.0 - (-DECAY_RATE * dt_secs).exp();
            let new_current = current + (target - current) * factor;
            if (new_current - target).abs() < SETTLE_EPSILON {
                self.current.set(target);
            } else {
                self.current.set(new_current);
            }
        }
    }

    pub fn is_animating(&self) -> bool {
        self.current.get() != self.target.get()
    }
}

impl Default for ScrollAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ScrollAnimator {
    fn clone(&self) -> Self {
        Self {
            current: Cell::new(self.current.get()),
            target: Cell::new(self.target.get()),
        }
    }
}

impl PartialEq for ScrollAnimator {
    fn eq(&self, other: &Self) -> bool {
        self.current.get() == other.current.get() && self.target.get() == other.target.get()
    }
}

impl Eq for ScrollAnimator {}
