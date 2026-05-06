#![no_std]
#![no_main]

pub mod control;
pub mod tasks;
pub mod utils;

use crate::control::angle_estimator::{self, AngleEstimator};
use crate::tasks::angle::{ANGLE_SIGNAL, AngleReading};
use as5600::asynch::As5600;
use control::{Step, apply_step};
use core::cmp::max;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::rcc::{AHBPrescaler, APBPrescaler, Pll, PllMul, PllPreDiv, PllRDiv, PllSource};
use embassy_stm32::{rcc::Hsi48Config, time::Hertz};
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
    // USART2 => embassy_stm32::usart::
    USART2 => embassy_stm32::usart::InterruptHandler<embassy_stm32::peripherals::USART2>;
    DMA1_CHANNEL3 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH3>;
    DMA1_CHANNEL4 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH4>;
    DMA1_CHANNEL5 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH5>;
    DMA1_CHANNEL6 => embassy_stm32::dma::InterruptHandler<embassy_stm32::peripherals::DMA1_CH6>;
    EXTI9_5 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI9_5>;
});

fn rcc_config() -> embassy_stm32::rcc::Config {
    let mut rcc_config = embassy_stm32::rcc::Config::default();
    rcc_config.hsi = false;
    rcc_config.hse = Some(embassy_stm32::rcc::Hse {
        freq: Hertz(8_000_000),
        mode: embassy_stm32::rcc::HseMode::Oscillator,
    });
    rcc_config.sys = embassy_stm32::rcc::Sysclk::Pll1R;
    rcc_config.hsi48 = Some(Hsi48Config::new());
    rcc_config.pll = Some(Pll {
        source: PllSource::Hse,
        prediv: PllPreDiv::Div2,
        mul: PllMul::Mul85,
        divp: None,
        divq: None,
        divr: Some(PllRDiv::Div2),
    });
    rcc_config.ahb_pre = AHBPrescaler::Div1;
    rcc_config.apb1_pre = APBPrescaler::Div1;
    rcc_config.apb2_pre = APBPrescaler::Div1;
    rcc_config.boost = true;
    rcc_config.mux.adc12sel = embassy_stm32::rcc::mux::Adcsel::Sys;
    rcc_config
}

fn config() -> embassy_stm32::Config {
    let mut config = embassy_stm32::Config::default();
    config.rcc = rcc_config();
    config
}

// pub struct HallSensors<'a> {
//     pub hall_a: embassy_stm32::exti::ExtiInput<'a, embassy_stm32::mode::Async>,
//     pub hall_b: embassy_stm32::exti::ExtiInput<'a, embassy_stm32::mode::Async>,
//     pub hall_c: embassy_stm32::exti::ExtiInput<'a, embassy_stm32::mode::Async>,
// }

pub enum DriverState {
    Align,
    Run,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(config());
    // GPIO BEMF is used as a way of enabling the voltage divider
    // let gpio_bemf =
    //     embassy_stm32::exti::ExtiInput::new(p.PB5, p.EXTI5, embassy_stm32::gpio::Pull::Down, Irqs);
    // let gpio_bemf1 = embassy_stm32::gpio::Input::new(p.PA4, embassy_stm32::gpio::Pull::None);
    // let gpio_bemf2 = embassy_stm32::gpio::Input::new(p.PC4, embassy_stm32::gpio::Pull::None);
    // let gpio_bemf3 = embassy_stm32::gpio::Input::new(p.PB11, embassy_stm32::gpio::Pull::None);
    // spawner.spawn(sensorless_task(gpio_bemf, gpio_bemf1, gpio_bemf2, gpio_bemf3).unwrap());

    // let _hall_1 = embassy_stm32::gpio::Input::new(p.PB6, embassy_stm32::gpio::Pull::None);
    // let _hall_2 = embassy_stm32::gpio::Input::new(p.PB7, embassy_stm32::gpio::Pull::None);
    // let _hall_3 = embassy_stm32::gpio::Input::new(p.PB8, embassy_stm32::gpio::Pull::None);

    let i2c = embassy_stm32::i2c::I2c::new(
        p.I2C1,
        p.PB8,
        p.PB7,
        p.DMA1_CH5,
        p.DMA1_CH6,
        Irqs,
        embassy_stm32::i2c::Config::default(),
    );
    let as5600 = As5600::new(i2c);
    spawner.spawn(tasks::angle::angle_task(as5600).unwrap());
    // spawner.spawn(tasks::angle::control_task().unwrap());
    // I2C1 SCL   PB8
    // I2C1 SDA   PB7/PB9
    // CAN_TX     PB9
    // CAN_RX     PA11
    // CAN_SHDC   PC11
    // CAN_TERM   PC14
    // STATUS     PC6
    // Temp Input PB14  NTC 10k with 4.7k
    // Vbus       PA0  169k 18k
    // Pot        PB12
    // Button     PC10
    // Pwm Input  PA15
    // USART2_RX  PB4
    // USART2_TX  PB3
    let uart = embassy_stm32::usart::Uart::new(
        p.USART2,
        p.PB4,
        p.PB3,
        p.DMA1_CH3,
        p.DMA1_CH4,
        Irqs,
        embassy_stm32::usart::Config::default(),
    )
    .unwrap();
    // let command_channel = COMMAND_CHANNEL.init(embassy_sync::channel::Channel::new());
    // let response_channel = RESPONSE_CHANNEL.init(embassy_sync::channel::Channel::new());
    // let command_sender = command_channel.sender();
    // let response_receiver = response_channel.receiver();
    spawner.spawn(
        // tasks::communication::communcation_task(uart, command_sender, response_receiver).unwrap(),
        tasks::communication::communcation_task(uart).unwrap(),
    );
    info!("Hello World!");
    let mut motor_driver = embassy_stm32::timer::complementary_pwm::ComplementaryPwm::new(
        p.TIM1,
        Some(embassy_stm32::timer::simple_pwm::PwmPin::new(
            p.PA8,
            embassy_stm32::gpio::OutputType::PushPull,
        )),
        Some(
            embassy_stm32::timer::complementary_pwm::ComplementaryPwmPin::new(
                p.PC13,
                embassy_stm32::gpio::OutputType::PushPull,
            ),
        ),
        Some(embassy_stm32::timer::simple_pwm::PwmPin::new(
            p.PA9,
            embassy_stm32::gpio::OutputType::PushPull,
        )),
        Some(
            embassy_stm32::timer::complementary_pwm::ComplementaryPwmPin::new(
                p.PA12,
                embassy_stm32::gpio::OutputType::PushPull,
            ),
        ),
        Some(embassy_stm32::timer::simple_pwm::PwmPin::new(
            p.PA10,
            embassy_stm32::gpio::OutputType::PushPull,
        )),
        Some(
            embassy_stm32::timer::complementary_pwm::ComplementaryPwmPin::new(
                p.PB15,
                embassy_stm32::gpio::OutputType::PushPull,
            ),
        ),
        None,
        None,
        Hertz(60_000),
        embassy_stm32::timer::low_level::CountingMode::CenterAlignedUpInterrupts,
    );
    motor_driver.set_master_output_enable(false);
    // motor_driver.set_dead_time(2);
    // motor_driver.set_duty(embassy_stm32::timer::Channel::Ch1, 100);
    // motor_driver.set_duty(embassy_stm32::timer::Channel::Ch2, 100);
    // motor_driver.set_duty(embassy_stm32::timer::Channel::Ch3, 100);
    // motor_driver.enable(embassy_stm32::timer::Channel::Ch1);
    // motor_driver.enable(embassy_stm32::timer::Channel::Ch2);
    // motor_driver.enable(embassy_stm32::timer::Channel::Ch3);
    // motor_driver.set_master_output_enable(true);
    // motor_driver.enable(embassy_stm32::timer::Channel::Ch4);
    motor_driver.set_mms2(embassy_stm32::timer::complementary_pwm::Mms2::Update);
    // motor_driver.set_mms2(embassy_stm32::timer::complementary_pwm::Mms2::CompareOc4);

    spawner.spawn(
        tasks::adc::adc_task(
            p.ADC1, p.ADC2, p.OPAMP1, p.OPAMP2, p.OPAMP3, p.PA1, p.PA2, p.PA3, p.PA5, p.PA6, p.PA7,
            p.PB0, p.PB1, p.PB2, p.PA0, p.PB14,
            // p.PB12,
        )
        .unwrap(),
    );

    let steps = [Step::S1, Step::S2, Step::S3, Step::S4, Step::S5, Step::S6];
    let mut pwm = 10;
    let max_duty = motor_driver.get_max_duty();
    let desired_dead_time = 0.02;
    let dead_time_counts = max_duty as f32 * desired_dead_time;
    motor_driver.set_dead_time(dead_time_counts as u16);
    motor_driver.set_master_output_enable(true);
    defmt::info!("Max Duty: {:?}", max_duty);
    // apply_step(&mut motor_driver, Step::S1, pwm, 0);
    // Timer::after_millis(200).await;
    let delay = 100;
    embassy_time::Timer::after_millis(1000).await;
    let mut driver_state = DriverState::Align;
    let mut start_angle = AngleReading {
        angle: 0.0,
        time: embassy_time::Instant::now(),
    };
    let mut last_angle = AngleReading {
        angle: 0.0,
        time: embassy_time::Instant::now(),
    };
    let mut updates = 0;
    let mut start_time = embassy_time::Instant::now();
    let mut elapsed_angle = 0.0;
    let mut velocity = 0.0;
    let mut estimator = AngleEstimator::new();
    let mut last_applied_sector: Option<usize> = None;
    loop {
        match driver_state {
            DriverState::Align => {
                apply_step(&mut motor_driver, Step::S1, 300, max_duty);
                embassy_time::Timer::after_millis(400).await;
                start_angle = ANGLE_SIGNAL.wait().await;
                apply_step(&mut motor_driver, Step::S2, 300, max_duty);
                embassy_time::Timer::after_millis(400).await;
                let commutated_angle = ANGLE_SIGNAL.wait().await;
                last_angle = commutated_angle;
                let angle_delta = compute_actual_angle(start_angle, commutated_angle);
                estimator.update(start_angle);
                estimator.update(commutated_angle);
                last_angle = commutated_angle;
                defmt::info!("{:?} {:?} {:?}", start_angle, commutated_angle, angle_delta);
                driver_state = DriverState::Run;
            }
            // DriverState::Run => {
            //     let current_angle = ANGLE_SIGNAL.wait().await;
            //     // let angle = compute_actual_angle(start_angle, current_angle);
            //     // defmt::info!("{}", angle);
            //     let sector = sector_from_aligned_angle(current_angle.angle, start_angle.angle, 7.0);
            //     let next_sector = (sector + 1) % 6;
            //     // let next_sector = sector;
            //     apply_step(&mut motor_driver, steps[next_sector], pwm, max_duty);
            //     updates += 1;
            //     elapsed_angle += compute_actual_angle(last_angle, current_angle);
            //     last_angle = current_angle;
            //     if updates == 1000 {
            //         let elapsed_avg =
            //             (embassy_time::Instant::now() - start_time).as_millis() as f32 / 1000.0;
            //         let rps = (1.0 / (elapsed_avg / 1000.0))
            //             * ((elapsed_angle / 1000.0) / core::f32::consts::TAU);
            //         let rpm = rps * 60.0;
            //         defmt::info!("{:?} {:?} {:?} {:?}", elapsed_avg, rps, rpm, pwm);
            //         start_time = embassy_time::Instant::now();
            //         updates = 0;
            //         elapsed_angle = 0.0;
            //         pwm += 20;
            //     }
            // }
            DriverState::Run => {
                if let Some(current_angle) = ANGLE_SIGNAL.try_take() {
                    let delta = compute_actual_angle(last_angle, current_angle);

                    elapsed_angle += delta;
                    updates += 1;

                    estimator.update(current_angle);
                    last_angle = current_angle;
                }

                if let Some(mech_angle) = estimator.angle_now(embassy_time::Instant::now()) {
                    let sector = sector_from_aligned_angle(mech_angle, start_angle.angle, 7.0);
                    let next_sector = (sector + 1) % 6;

                    apply_step(&mut motor_driver, steps[next_sector], pwm, max_duty);
                } else {
                    // Sensor is stale. Safer than commutating blindly.
                    apply_step(&mut motor_driver, Step::S1, 0, max_duty);
                }
                if updates >= 1000 {
                    let elapsed_s = duration_to_secs(embassy_time::Instant::now() - start_time);
                    let rps = (elapsed_angle / core::f32::consts::TAU) / elapsed_s;
                    let rpm = rps * 60.0;

                    defmt::info!("{:?} {:?} {:?} {:?}", elapsed_s, rps, rpm, pwm);

                    start_time = embassy_time::Instant::now();
                    updates = 0;
                    elapsed_angle = 0.0;

                    if pwm < 300 {
                        pwm += 20;
                    }
                }

                Timer::after_micros(5).await;
            } // DriverState::Run => {
              //     while let Some(current_angle) = ANGLE_SIGNAL.try_take() {
              //         estimator.update(current_angle);
              //         last_angle = current_angle;
              //     }

              //     let now = embassy_time::Instant::now();

              //     if let Some(mech_angle) = estimator.angle_now(now) {
              //         let sector = sector_from_aligned_angle(mech_angle, start_angle.angle, 7.0);
              //         let next_sector = (sector + 1) % 6;

              //         if last_applied_sector != Some(next_sector) {
              //             apply_step(&mut motor_driver, steps[next_sector], pwm, max_duty);
              //             last_applied_sector = Some(next_sector);
              //         }
              //     }
              //     if pwm < 800 {
              //         pwm += 10;
              //     }
              //     Timer::after_micros(100).await;
              // }
        }

        // if let Some(sector) = SECTOR_SIGNAL.try_take() {
        //     defmt::info!("{:?}", sector);
        // }
        // for &step in &steps {
        //     start_angle = ANGLE_SIGNAL.wait().await;
        //     apply_step(&mut motor_driver, step, pwm, max_duty);
        //     // let (bemf, bemf1, bemf2, bemf3) = (
        //     //     gpio_bemf.is_high(),
        //     //     gpio_bemf1.is_high(),
        //     //     gpio_bemf2.is_high(),
        //     //     gpio_bemf3.is_high(),
        //     // );
        //     // defmt::info!("{} {} {} {}", bemf, bemf1, bemf2, bemf3);
        //     // defmt::info!("{} {} {}", bemf1, bemf2, bemf3);
        //     Timer::after_millis(5).await;
        //     let commutated_angle = ANGLE_SIGNAL.wait().await;
        //     let angle_delta = compute_actual_angle(start_angle, commutated_angle);
        //     defmt::info!("{:?}", angle_delta);
        // }
        // defmt::info!("done");
    }
}

pub fn duration_to_secs(duration: embassy_time::Duration) -> f32 {
    (duration.as_micros() as f32) / 1_000_000.0
}

pub fn compute_actual_angle(start_angle: AngleReading, current_angle: AngleReading) -> f32 {
    use core::f32::consts::{PI, TAU};
    let mut delta = current_angle.angle - start_angle.angle;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    delta
}

pub fn sector_from_angle(mech_rad: f32, phase_offset: f32, pole_pairs: f32) -> usize {
    use core::f32::consts::TAU;
    let mut elec = mech_rad * pole_pairs + phase_offset;
    if elec >= TAU {
        elec -= TAU;
    } else if elec < 0.0 {
        elec += TAU;
    }
    (elec * (6.0 / TAU)) as usize
}
fn wrap_0_tau(mut x: f32) -> f32 {
    use core::f32::consts::TAU;
    while x >= TAU {
        x -= TAU;
    }
    while x < 0.0 {
        x += TAU;
    }
    x
}

fn sector_from_aligned_angle(current_mech_rad: f32, align_mech_rad: f32, pole_pairs: f32) -> usize {
    let electrical_angle = wrap_0_tau((current_mech_rad - align_mech_rad) * pole_pairs);

    (electrical_angle * (6.0 / core::f32::consts::TAU)) as usize
}

// Fs - Phase Resistance 0.052 ohms
// Fs - Kv 140
//
// 2804 Motor Specifications
// Model: 2804 Operating Voltage: 7.4 – 16 VDC
// Rated Voltage: 12 V (Instantaneous max voltage 20 V) Slot/Poles: 12N14P
// Stator Size: 28 mm (12 slots, 14 poles, FOC control, 7 pole pairs) Inner Rotor Shaft Diameter: 8 mm
// Bearing Center Hole: 5.4 – 6.5 mm Motor Dimensions: 34.5 × 15 mm
// Weight: 37 g Winding Resistance: 5.1 Ω
// Phase Resistance Rs: 2.3 Ω Rated Current: 0.5 A (Temperature rise normal under 1 A)
// Max Current: 2 A KV Rating: 220
// Speed: 2600 RPM / 12 V Winding Inductance: 2.8 mH
// Phase Inductance Ls: 0.86 mH Magnetic Flux: 0.0035 Wb
// Torque: 300 g·cm (≈0.03 Nm)
//
// pub struct MotorCharacterization {
//     phase_resistance: f32,
//     pole_pairs: u32,
//     kv: Option<u32>,
//     lq: Option<f32>,
//     ld: Option<f32>,
// }
//
// White Ou1
// Red Out2
// Black Out3
