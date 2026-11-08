use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

impl Geometry {
    /// Logical canvas hit testing, independent of the visual compositor clip.
    pub fn contains_canvas_point(self, x: f64, y: f64) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let width = self.width.clamp(1.0, 700.0);
        let height = self.height.clamp(1.0, 540.0);
        let radius = self.radius.clamp(0.0, width.min(height) / 2.0);
        let left = (super::CANVAS_WIDTH - width) / 2.0;
        let top = super::TOP_INSET;
        if x < left || x >= left + width || y < top || y >= top + height {
            return false;
        }
        let dx = ((x - (left + width / 2.0)).abs() - (width / 2.0 - radius)).max(0.0);
        let dy = ((y - (top + height / 2.0)).abs() - (height / 2.0 - radius)).max(0.0);
        dx * dx + dy * dy <= radius * radius
    }
    pub const COMPACT: Self = Self {
        width: 180.0,
        height: 38.0,
        radius: 19.0,
    };

    pub fn validate(self) -> Result<Self, String> {
        if !self.width.is_finite() || !self.height.is_finite() || !self.radius.is_finite() {
            return Err("Island geometry must be finite".into());
        }
        if !(120.0..=680.0).contains(&self.width) || !(28.0..=520.0).contains(&self.height) {
            return Err("Island geometry is outside the native canvas".into());
        }
        if !(8.0..=80.0).contains(&self.radius) {
            return Err("Island radius must be between 8 and 80".into());
        }
        Ok(self)
    }

    fn values(self) -> [f64; 3] {
        [self.width, self.height, self.radius]
    }
    fn from_values(values: [f64; 3]) -> Self {
        Self {
            width: values[0],
            height: values[1],
            radius: values[2],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Spring {
    initial: [f64; 3],
    velocity: [f64; 3],
    target: [f64; 3],
    stiffness: f64,
    damping: f64,
    pub started: Instant,
    pub animate: bool,
}

impl Default for Spring {
    fn default() -> Self {
        let values = Geometry::COMPACT.values();
        Self {
            initial: values,
            velocity: [0.0; 3],
            target: values,
            stiffness: 260.0,
            damping: 28.0,
            started: Instant::now(),
            animate: false,
        }
    }
}

impl Spring {
    pub fn retarget(&mut self, target: Geometry, animate: bool, intensity: &str) {
        self.retarget_at(target, animate, intensity, Instant::now());
    }

    fn retarget_at(&mut self, target: Geometry, animate: bool, intensity: &str, now: Instant) {
        let (current, velocity) = self.sample(now);
        let (stiffness, damping) = match intensity {
            "subtle" => (320.0, 32.0),
            "expressive" => (230.0, 24.0),
            _ => (260.0, 28.0),
        };
        *self = Self {
            initial: current.values(),
            velocity,
            target: target.values(),
            stiffness,
            damping,
            started: now,
            animate,
        };
    }

    pub fn sample(&self, now: Instant) -> (Geometry, [f64; 3]) {
        if !self.animate {
            return (Geometry::from_values(self.target), [0.0; 3]);
        }
        let time = now.duration_since(self.started).as_secs_f64();
        let mut values = [0.0; 3];
        let mut velocities = [0.0; 3];
        for index in 0..3 {
            let (displacement, velocity) = spring_sample(
                self.initial[index] - self.target[index],
                self.velocity[index],
                time,
                self.stiffness,
                self.damping,
            );
            values[index] = self.target[index] + displacement;
            velocities[index] = velocity;
        }
        (Geometry::from_values(values), velocities)
    }

    pub fn is_settled(&self, now: Instant) -> bool {
        if !self.animate || now.duration_since(self.started).as_secs_f64() > 1.5 {
            return true;
        }
        let (value, velocity) = self.sample(now);
        value
            .values()
            .iter()
            .zip(self.target)
            .all(|(value, target)| (value - target).abs() < 0.1)
            && velocity.iter().all(|velocity| velocity.abs() < 0.1)
    }

    pub fn settle(&mut self) {
        self.animate = false;
    }
}

fn spring_sample(
    position: f64,
    velocity: f64,
    time: f64,
    stiffness: f64,
    damping: f64,
) -> (f64, f64) {
    let mass = 1.0;
    let alpha = damping / (2.0 * mass);
    let omega_squared = stiffness / mass;
    let discriminant = alpha * alpha - omega_squared;
    if discriminant < -1e-6 {
        let omega = (-discriminant).sqrt();
        let b = (velocity + alpha * position) / omega;
        let cosine = (omega * time).cos();
        let sine = (omega * time).sin();
        let envelope = (-alpha * time).exp();
        let displacement = envelope * (position * cosine + b * sine);
        let speed = envelope
            * ((-alpha * position + b * omega) * cosine + (-alpha * b - position * omega) * sine);
        (displacement, speed)
    } else if discriminant > 1e-6 {
        let root = discriminant.sqrt();
        let first = -alpha + root;
        let second = -alpha - root;
        let a = (velocity - second * position) / (first - second);
        let b = position - a;
        (
            a * (first * time).exp() + b * (second * time).exp(),
            a * first * (first * time).exp() + b * second * (second * time).exp(),
        )
    } else {
        let b = velocity + alpha * position;
        let envelope = (-alpha * time).exp();
        (
            envelope * (position + b * time),
            envelope * (b - alpha * (position + b * time)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn springs_converge_for_all_presets() {
        for intensity in ["subtle", "balanced", "expressive"] {
            let mut spring = Spring::default();
            spring.retarget(
                Geometry {
                    width: 560.0,
                    height: 354.0,
                    radius: 28.0,
                },
                true,
                intensity,
            );
            let (end, velocity) = spring.sample(spring.started + Duration::from_secs(2));
            assert!((end.width - 560.0).abs() < 0.001);
            assert!(velocity[0].abs() < 0.001);
        }
    }

    #[test]
    fn rejects_invalid_geometry() {
        assert!(Geometry {
            width: f64::INFINITY,
            height: 38.0,
            radius: 19.0
        }
        .validate()
        .is_err());
        assert!(Geometry {
            width: 720.0,
            height: 38.0,
            radius: 19.0
        }
        .validate()
        .is_err());
    }

    #[test]
    fn transparent_pill_corners_and_canvas_exterior_do_not_capture_mouse() {
        let pill = Geometry::COMPACT;
        assert!(pill.contains_canvas_point(360.0, 31.0));
        assert!(!pill.contains_canvas_point(270.0, 12.0));
        assert!(!pill.contains_canvas_point(269.0, 31.0));
        assert!(!pill.contains_canvas_point(450.0, 31.0));
        assert!(!pill.contains_canvas_point(360.0, 51.0));
        assert!(!pill.contains_canvas_point(f64::NAN, 31.0));
    }

    #[test]
    fn expanded_hit_test_retains_controls_but_releases_transparent_corners() {
        let panel = Geometry {
            width: 560.0,
            height: 354.0,
            radius: 28.0,
        };
        assert!(panel.contains_canvas_point(360.0, 200.0));
        assert!(panel.contains_canvas_point(600.0, 42.0));
        assert!(!panel.contains_canvas_point(80.0, 12.0));
        assert!(!panel.contains_canvas_point(639.0, 365.0));
        assert!(!panel.contains_canvas_point(360.0, 400.0));
    }

    #[test]
    fn rapid_reversal_preserves_position_and_velocity() {
        let mut spring = Spring::default();
        spring.retarget(
            Geometry {
                width: 560.0,
                height: 354.0,
                radius: 28.0,
            },
            true,
            "balanced",
        );
        let interrupted_at = spring.started + Duration::from_millis(80);
        let (before, velocity_before) = spring.sample(interrupted_at);
        spring.retarget_at(Geometry::COMPACT, true, "balanced", interrupted_at);
        let (after, velocity_after) = spring.sample(interrupted_at);
        assert!((before.width - after.width).abs() < 1e-9);
        assert!((before.height - after.height).abs() < 1e-9);
        for index in 0..3 {
            assert!((velocity_before[index] - velocity_after[index]).abs() < 1e-9);
        }
    }

    #[test]
    fn disabling_animation_applies_target_immediately() {
        let mut spring = Spring::default();
        spring.retarget(
            Geometry {
                width: 560.0,
                height: 354.0,
                radius: 28.0,
            },
            false,
            "balanced",
        );
        let (geometry, velocity) = spring.sample(spring.started);
        assert_eq!(geometry.width, 560.0);
        assert_eq!(geometry.height, 354.0);
        assert_eq!(velocity, [0.0; 3]);
        assert!(spring.is_settled(spring.started));
    }
}
