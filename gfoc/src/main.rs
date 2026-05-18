#![no_std]
#![no_main]

pub mod board;
pub mod control;
pub mod foc_control;
pub mod foc_tests;
pub mod tasks;
pub mod trap_tests;
pub mod utils;

use crate::board::{OpAmpPins, SAMPLE_TOGGLE};
use as5600::asynch::As5600;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::rcc::{AHBPrescaler, APBPrescaler, Pll, PllMul, PllPreDiv, PllRDiv, PllSource};
use embassy_stm32::{rcc::Hsi48Config, time::Hertz};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<embassy_stm32::peripherals::I2C1>;
    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<embassy_stm32::peripherals::I2C1>;
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

pub enum DriverState {
    Align,
    Run,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(config());
    let shunt1 = OpAmpPins {
        opamp: p.OPAMP1,
        in_pin: p.PA1,
        bias_pin: p.PA3,
        out_pin: p.PA2,
        gain: embassy_stm32::opamp::OpAmpGain::Mul16,
    };
    let shunt2 = OpAmpPins {
        opamp: p.OPAMP2,
        in_pin: p.PA7,
        bias_pin: p.PA5,
        out_pin: p.PA6,
        gain: embassy_stm32::opamp::OpAmpGain::Mul16,
    };
    let shunt3 = OpAmpPins {
        opamp: p.OPAMP3,
        in_pin: p.PB0,
        bias_pin: p.PB2,
        out_pin: p.PB1,
        gain: embassy_stm32::opamp::OpAmpGain::Mul16,
    };
    spawner.spawn(
        // tasks::adc::adc_task(
        //     p.ADC1, p.ADC2, p.OPAMP1, p.OPAMP2, p.OPAMP3, p.PA1, p.PA2, p.PA3, p.PA5, p.PA6, p.PA7,
        //     p.PB0, p.PB1, p.PB2, p.PA0, p.PB14,
        // )
        tasks::adc::adc_task(p.ADC1, p.ADC2, shunt1, shunt2, shunt3, p.PA0, p.PB14).unwrap(),
    );
    defmt::info!("Adc Spawned");
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
    defmt::info!("Angle Spawned");
    let st = embassy_stm32::gpio::Output::new(
        p.PB6,
        embassy_stm32::gpio::Level::High,
        embassy_stm32::gpio::Speed::VeryHigh,
    );

    critical_section::with(|cs| {
        SAMPLE_TOGGLE.borrow(cs).replace(Some(st));
    });
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
    spawner.spawn(tasks::communication::communcation_task(uart).unwrap());
    defmt::info!("Communication Spawned");
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
        Hertz(20_000),
        embassy_stm32::timer::low_level::CountingMode::CenterAlignedUpInterrupts,
    );
    motor_driver.set_master_output_enable(false);
    // motor_driver.set_mms2(embassy_stm32::timer::complementary_pwm::Mms2::Update);
    // motor_driver.set_mms2(embassy_stm32::timer::complementary_pwm::Mms2::ComparePulse);
    // motor_driver.enable(embassy_stm32::timer::Channel::Ch4);
    motor_driver.set_mms2(embassy_stm32::timer::complementary_pwm::Mms2::CompareOc4);
    let max_duty = motor_driver.get_max_duty();
    motor_driver.set_duty(embassy_stm32::timer::Channel::Ch4, max_duty - 50);
    motor_driver.enable(embassy_stm32::timer::Channel::Ch4);
    let desired_dead_time = 0.02;
    let dead_time_counts = max_duty as f32 * desired_dead_time;
    motor_driver.set_dead_time(dead_time_counts as u16);
    motor_driver.set_master_output_enable(true);
    defmt::info!("Max Duty: {}", max_duty);
    embassy_time::Timer::after_millis(1000).await;
    // trap_tests::basic_trap(&mut motor_driver).await;
    foc_tests::basic_foc(&mut motor_driver).await;
}
