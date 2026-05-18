use defmt::*;
use embassy_stm32::timer::AdvancedInstance4Channel;
use embassy_stm32::timer::complementary_pwm::ComplementaryPwm;

use crate::control::angle_estimator::{AngleEstimator, wrap_0_tau};
use crate::tasks::angle::AngleReading;

#[derive(Debug, Format, Clone, Copy)]
pub struct DutyCycle(f32);

impl DutyCycle {
    pub fn new(value: f32) -> Self {
        Self(if value > 1.0 {
            1.0
        } else if value < 0.0 {
            0.0
        } else {
            value
        })
    }

    pub fn zero() -> Self {
        Self::new(0.0)
    }

    pub fn duty(&self) -> f32 {
        self.0
    }
}

pub struct DutyCycles {
    pub ua: DutyCycle,
    pub ub: DutyCycle,
    pub uv: DutyCycle,
}

pub struct MotorDriver<T: AdvancedInstance4Channel> {
    pwm: ComplementaryPwm<'static, T>,
    max_duty: u32,
    pwm_limit: Option<DutyCycle>,
    pwm_max: Option<u32>,
}

impl<T: AdvancedInstance4Channel> MotorDriver<T> {
    pub fn new(pwm: ComplementaryPwm<'static, T>) -> Self {
        let max_duty = pwm.get_max_duty();
        Self {
            pwm: pwm,
            max_duty,
            pwm_limit: None,
            pwm_max: None,
        }
    }

    pub fn set_pwm_limit(&mut self, pwm_limit: Option<DutyCycle>) {
        if let Some(duty) = pwm_limit {
            let pwm_max = (self.max_duty as f32 * duty.duty()) as u32;
            self.pwm_limit = pwm_limit;
            self.pwm_max = Some(pwm_max);
        } else {
            self.pwm_limit = pwm_limit;
            self.pwm_max = None;
        }
    }

    pub fn enable(&mut self) {
        self.pwm.set_master_output_enable(true);
    }

    pub fn disable(&mut self) {
        self.pwm.set_master_output_enable(false);
    }

    fn set_channel_pwm(&mut self, channel: embassy_stm32::timer::Channel, duty: DutyCycle) {
        if let Some(max_pwm) = self.pwm_max {
            let ch_duty = ((self.max_duty as f32 * duty.duty()) as u32).clamp(0, max_pwm);
            self.pwm.set_duty(channel, ch_duty);
        } else {
            let ch_duty = (self.max_duty as f32 * duty.duty()) as u32;
            self.pwm.set_duty(channel, ch_duty);
        }
    }

    pub fn enable_channel(&mut self, channel: embassy_stm32::timer::Channel) {
        self.pwm.enable(channel);
    }

    pub fn disable_channel(&mut self, channel: embassy_stm32::timer::Channel) {
        self.pwm.disable(channel);
    }

    pub fn set_pwms(&mut self, ch1: DutyCycle, ch2: DutyCycle, ch3: DutyCycle) {
        self.set_channel_pwm(embassy_stm32::timer::Channel::Ch1, ch1);
        self.set_channel_pwm(embassy_stm32::timer::Channel::Ch2, ch2);
        self.set_channel_pwm(embassy_stm32::timer::Channel::Ch3, ch3);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Cw,
    Ccw,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriverControlMode {
    Angle(f32),
    Torque(f32, Direction),
    Velocity(f32, Direction),
}

pub struct MotorParams {
    pub phase_resistance: f32,
    pub pole_pairs: u32,
    pub kv: Option<u32>,
    pub lq: Option<f32>,
    pub ld: Option<f32>,
}

pub struct MotorController<T: AdvancedInstance4Channel> {
    driver: MotorDriver<T>,
    control_mode: DriverControlMode,
    angle_estimator: AngleEstimator,
    run: bool,
    motor_params: MotorParams,
    vbus: Option<f32>,
}

impl<T: AdvancedInstance4Channel> MotorController<T> {
    pub fn new(
        driver: MotorDriver<T>,
        control_mode: DriverControlMode,
        motor_params: MotorParams,
    ) -> MotorController<T> {
        Self {
            driver,
            control_mode,
            angle_estimator: AngleEstimator::new(),
            run: false,
            vbus: None,
            motor_params,
        }
    }

    pub fn update_vbus(&mut self, vbus: Option<f32>) {
        self.vbus = vbus;
    }

    pub fn set_run(&mut self, run: bool) {
        self.run = run;
    }

    pub fn update_angle(&mut self, angle: AngleReading) {
        self.angle_estimator.update(angle);
    }

    pub fn get_mechanical_angle(&self) -> Option<f32> {
        self.angle_estimator.angle_now(embassy_time::Instant::now())
    }

    pub fn get_electrical_angle(&self) -> Option<f32> {
        self.angle_estimator
            .angle_now(embassy_time::Instant::now())
            .map(|x| wrap_0_tau(x * self.motor_params.pole_pairs as f32))
    }

    pub fn get_speed(&self) -> f32 {
        self.angle_estimator.velocity_rad_s()
    }

    pub async fn enable(&mut self) {
        self.driver.enable();
    }

    pub async fn disable(&mut self) {
        self.driver.disable();
    }

    pub async fn align(&mut self, delay: embassy_time::Duration) {
        self.enable().await;
        self.driver
            .set_pwms(DutyCycle::new(0.5), DutyCycle::zero(), DutyCycle::zero());
        embassy_time::Timer::after(delay).await;
        self.disable().await;
    }

    pub fn reset_angle(&mut self) {
        self.angle_estimator = AngleEstimator::new();
    }

    // pub fn update_current(&mut self, current: &[f32; 2]) {
    // }
}

// pub trait AngleSensor {
//     pub fn initialize(&mut self)
//     pub fn update(&mut self, angle: AngleReading)
//     pub fn update(&mut self, angle: AngleReading)
// }
