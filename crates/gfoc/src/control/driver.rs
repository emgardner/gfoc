use defmt::*;
use embassy_stm32::timer::AdvancedInstance4Channel;
use embassy_stm32::timer::complementary_pwm::ComplementaryPwm;

#[derive(Debug, Format)]
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

    pub fn duty(&self) -> f32 {
        self.0
    }
}

pub struct DutyCycles {
    pub ua: DutyCycle,
    pub ub: DutyCycle,
    pub uv: DutyCycle,
}

// pub trait BldcDriver {
//     fn duty_cycles(&self) -> DutyCycles;
//     fn set_pwms(&mut self, ua: DutyCycle, ub: DutyCycle, uv: DutyCycle);
//     fn enable(&mut self);
//     fn disable(&mut self);
//     fn init(&mut self);
// }

pub struct MotorDriver<T: AdvancedInstance4Channel> {
    pwm: ComplementaryPwm<'static, T>,
    max_duty: u32,
}

impl<T: AdvancedInstance4Channel> MotorDriver<T> {
    pub fn new(pwm: ComplementaryPwm<'static, T>) -> Self {
        let max_duty = pwm.get_max_duty();
        Self { pwm: pwm, max_duty }
    }

    pub fn enable(&mut self) {
        self.pwm.set_master_output_enable(true);
    }

    pub fn disable(&mut self) {
        self.pwm.set_master_output_enable(false);
    }

    fn set_channel_pwm(&mut self, channel: embassy_stm32::timer::Channel, duty: DutyCycle) {
        let ch_duty = (self.max_duty as f32 * duty.duty()) as u32;
        self.pwm.set_duty(channel, ch_duty);
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
