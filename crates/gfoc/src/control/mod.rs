pub mod angle_estimator;
pub mod driver;
use defmt::Format;
use embassy_stm32::timer::AdvancedInstance4Channel;

pub struct MotorCharacterization {
    pub phase_resistance: f32,
    pub pole_pairs: u32,
    pub kv: Option<u32>,
    pub lq: Option<f32>,
    pub ld: Option<f32>,
}

#[derive(Clone, Copy)]
pub enum Step {
    S1, // U+ V-
    S2, // U+ W-
    S3, // V+ W-
    S4, // V+ U-
    S5, // W+ U-
    S6, // W+ V-
}

#[derive(Debug, Format, Clone, Copy, Eq, PartialEq)]
pub enum Sector {
    Sector0,
    Sector1,
    Sector2,
    Sector3,
    Sector4,
    Sector5,
}

const SECTOR_TABLE: [Option<Sector>; 8] = [
    None,
    None,
    Some(Sector::Sector2),
    Some(Sector::Sector1),
    Some(Sector::Sector4),
    Some(Sector::Sector5),
    Some(Sector::Sector3),
    None,
];

impl Sector {
    pub fn from_bools(hall_a: bool, hall_b: bool, hall_c: bool) -> Option<Sector> {
        let mut base = hall_a as usize;
        base |= (hall_b as usize) << 1;
        base |= (hall_c as usize) << 2;
        SECTOR_TABLE.into_iter().nth(base).unwrap_or(None)
    }

    pub fn from_usize(state: usize) -> Option<Sector> {
        SECTOR_TABLE.into_iter().nth(state).unwrap_or(None)
    }
}

pub fn get_vlimit_pwm(v_limit: f32, v_supply: f32) -> f32 {
    (v_limit / v_supply).clamp(0.0, 1.0)
}

pub fn get_climit_pwm(c_limit: f32, v_supply: f32, phase_resistance: f32) -> f32 {
    let max_voltage = c_limit * phase_resistance;
    get_vlimit_pwm(max_voltage, v_supply)
}

pub fn get_max_pwm(c_limit: f32, v_limit: f32, v_supply: f32, phase_resistance: f32) -> f32 {
    let vlimit_pwm = get_vlimit_pwm(v_limit, v_supply);
    let climit_pwm = get_climit_pwm(c_limit, v_supply, phase_resistance);
    if vlimit_pwm > climit_pwm {
        vlimit_pwm
    } else {
        climit_pwm
    }
}

pub fn get_pwms(sector: Sector) -> () {
    match sector {
        Sector::Sector0 => {}
        Sector::Sector1 => {}
        Sector::Sector2 => {}
        Sector::Sector3 => {}
        Sector::Sector4 => {}
        Sector::Sector5 => {}
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PhaseCommand {
    Float,
    HighPwm(u32),
    LowOn,
    HighOn,
}

fn apply_phase<T: AdvancedInstance4Channel>(
    driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
    ch: embassy_stm32::timer::Channel,
    cmd: PhaseCommand,
    max_duty: u32,
) {
    match cmd {
        PhaseCommand::Float => {
            driver.disable(ch);
        }

        // CHx = PWM, CHxN = opposite PWM
        // With CHx -> HIN and CHxN -> LIN:
        // high side is PWM'd, low side is complementary.
        PhaseCommand::HighPwm(duty) => {
            let duty = duty.min(max_duty);
            driver.set_duty(ch, duty);
            driver.enable(ch);
        }

        // CHx = 0, CHxN = 1
        // high side off, low side on continuously
        PhaseCommand::LowOn => {
            driver.set_duty(ch, 0);
            driver.enable(ch);
        }

        // CHx = 1, CHxN = 0
        // high side on continuously, low side off
        PhaseCommand::HighOn => {
            driver.set_duty(ch, max_duty);
            driver.enable(ch);
        }
    }
}

pub fn apply_step<T: AdvancedInstance4Channel>(
    driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
    step: Step,
    pwm: u32,
    max_duty: u32,
) {
    let (u, v, w) = match step {
        // U = PWM, V = low side on, W = float
        Step::S1 => (
            PhaseCommand::HighPwm(pwm),
            PhaseCommand::LowOn,
            PhaseCommand::Float,
        ),

        // U = PWM, W = low side on, V = float
        Step::S2 => (
            PhaseCommand::HighPwm(pwm),
            PhaseCommand::Float,
            PhaseCommand::LowOn,
        ),

        // V = PWM, W = low side on, U = float
        Step::S3 => (
            PhaseCommand::Float,
            PhaseCommand::HighPwm(pwm),
            PhaseCommand::LowOn,
        ),

        // V = PWM, U = low side on, W = float
        Step::S4 => (
            PhaseCommand::LowOn,
            PhaseCommand::HighPwm(pwm),
            PhaseCommand::Float,
        ),

        // W = PWM, U = low side on, V = float
        Step::S5 => (
            PhaseCommand::LowOn,
            PhaseCommand::Float,
            PhaseCommand::HighPwm(pwm),
        ),

        // W = PWM, V = low side on, U = float
        Step::S6 => (
            PhaseCommand::Float,
            PhaseCommand::LowOn,
            PhaseCommand::HighPwm(pwm),
        ),
    };

    // Program all three phases
    apply_phase(driver, embassy_stm32::timer::Channel::Ch1, u, max_duty);
    apply_phase(driver, embassy_stm32::timer::Channel::Ch2, v, max_duty);
    apply_phase(driver, embassy_stm32::timer::Channel::Ch3, w, max_duty);
}

fn apply_channel<T: AdvancedInstance4Channel>(
    driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
    channel: embassy_stm32::timer::Channel,
    pwm: u32,
) {
    // if pwm == 0 {
    //     driver.disable(channel);
    // } else {
    driver.set_duty(channel, pwm);
    driver.enable(channel);
    // }
}

pub fn apply_pwms<T: AdvancedInstance4Channel>(
    driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
    u: u32,
    v: u32,
    w: u32,
) {
    apply_channel(driver, embassy_stm32::timer::Channel::Ch1, u);
    apply_channel(driver, embassy_stm32::timer::Channel::Ch2, v);
    apply_channel(driver, embassy_stm32::timer::Channel::Ch3, w);
}
