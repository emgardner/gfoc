use crate::tasks::angle::AngleReading;

pub struct AngleEstimator {
    last_mech: f32,
    last_unwrapped: f32,
    last_time: embassy_time::Instant,
    velocity_rad_s: f32,
    initialized: bool,
}

impl AngleEstimator {
    pub const fn new() -> Self {
        Self {
            last_mech: 0.0,
            last_unwrapped: 0.0,
            last_time: embassy_time::Instant::from_ticks(0),
            velocity_rad_s: 0.0,
            initialized: false,
        }
    }

    pub fn update(&mut self, reading: AngleReading) {
        if !self.initialized {
            self.last_mech = reading.angle;
            self.last_unwrapped = reading.angle;
            self.last_time = reading.time;
            self.velocity_rad_s = 0.0;
            self.initialized = true;
            return;
        }

        let dt = duration_to_secs(reading.time - self.last_time);
        if dt <= 0.0 {
            return;
        }

        let delta = wrap_pm_pi(reading.angle - self.last_mech);
        let inst_velocity = delta / dt;

        // Tune this. Lower = smoother, higher = more responsive.
        const ALPHA: f32 = 0.20;

        self.velocity_rad_s = self.velocity_rad_s + ALPHA * (inst_velocity - self.velocity_rad_s);

        self.last_unwrapped += delta;
        self.last_mech = reading.angle;
        self.last_time = reading.time;
    }

    pub fn angle_now(&self, now: embassy_time::Instant) -> Option<f32> {
        if !self.initialized {
            return None;
        }

        let dt = duration_to_secs(now - self.last_time);

        // Do not extrapolate forever if the sensor stalls.
        // Tune based on your AS5600 update period.
        // if dt > 0.050 {
        if dt > 0.050 {
            return None;
        }
        Some(wrap_0_tau(self.last_mech + self.velocity_rad_s * dt))
    }

    pub fn velocity_rad_s(&self) -> f32 {
        self.velocity_rad_s
    }
}

pub fn duration_to_secs(duration: embassy_time::Duration) -> f32 {
    duration.as_micros() as f32 / 1_000_000.0
}

pub fn wrap_pm_pi(mut x: f32) -> f32 {
    use core::f32::consts::{PI, TAU};

    while x > PI {
        x -= TAU;
    }

    while x < -PI {
        x += TAU;
    }

    x
}

pub fn wrap_0_tau(mut x: f32) -> f32 {
    use core::f32::consts::TAU;

    while x >= TAU {
        x -= TAU;
    }

    while x < 0.0 {
        x += TAU;
    }

    x
}
