use core::cell::RefCell;
use embassy_stm32::adc::InjectedAdc;
use embassy_stm32::opamp::OpAmpOutput;
use embassy_stm32::opamp::{OpAmp, OpAmpGain, OpAmpSpeed};
use embassy_stm32::pac::adc::Adc as AdcRegs;
use embassy_stm32::{Peri, peripherals};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

pub const V_REF: f32 = 3.3;
pub const R_SHUNT: f32 = 0.003;

pub static ADCS_HANDLE: CriticalSectionMutex<
    RefCell<Option<(InjectedAdc<AdcRegs>, InjectedAdc<AdcRegs>)>>,
> = CriticalSectionMutex::new(RefCell::new(None));

pub static OP1: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP1>> =
    StaticCell::new();
pub static OP2: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP2>> =
    StaticCell::new();
pub static OP3: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP3>> =
    StaticCell::new();

pub static SHUNT1: StaticCell<OpAmpOutput<'static, peripherals::OPAMP1>> = StaticCell::new();
pub static SHUNT2: StaticCell<OpAmpOutput<'static, peripherals::OPAMP2>> = StaticCell::new();
pub static SHUNT3: StaticCell<OpAmpOutput<'static, peripherals::OPAMP3>> = StaticCell::new();
pub static VBUS: StaticCell<Peri<'static, peripherals::PA0>> = StaticCell::new();

pub static RAW_CURRENT_SIGNAL: Signal<CriticalSectionRawMutex, (u16, u16, u16, u16)> =
    Signal::new();
pub static CURRENT_SIGNAL: Signal<CriticalSectionRawMutex, (f32, f32, f32, f32)> = Signal::new();
pub static SAMPLE_TOGGLE: CriticalSectionMutex<
    RefCell<Option<embassy_stm32::gpio::Output<'static>>>,
> = CriticalSectionMutex::new(RefCell::new(None));

pub struct OpAmpPins<'d, T, IN, BIAS, OUT>
where
    T: embassy_stm32::opamp::Instance,
    IN: embassy_stm32::opamp::NonInvertingPin<T>,
    OUT: embassy_stm32::opamp::OutputPin<T>,
    BIAS: embassy_stm32::opamp::BiasPin<T>,
{
    pub opamp: Peri<'d, T>,
    pub in_pin: Peri<'d, IN>,
    pub bias_pin: Peri<'d, BIAS>,
    pub out_pin: Peri<'d, OUT>,
    pub gain: OpAmpGain,
}

pub type Shunt1 = OpAmpPins<
    'static,
    embassy_stm32::peripherals::OPAMP1,
    embassy_stm32::peripherals::PA1,
    embassy_stm32::peripherals::PA3,
    embassy_stm32::peripherals::PA2,
>;

pub type Shunt2 = OpAmpPins<
    'static,
    embassy_stm32::peripherals::OPAMP2,
    embassy_stm32::peripherals::PA7,
    embassy_stm32::peripherals::PA5,
    embassy_stm32::peripherals::PA6,
>;

pub type Shunt3 = OpAmpPins<
    'static,
    embassy_stm32::peripherals::OPAMP3,
    embassy_stm32::peripherals::PB0,
    embassy_stm32::peripherals::PB2,
    embassy_stm32::peripherals::PB1,
>;

pub fn init_biased_opamp_ext<'d, T, IN, BIAS, OUT>(
    cell: &'static StaticCell<OpAmp<'static, T>>,

    output_cell: &'static StaticCell<OpAmpOutput<'static, T>>,

    pins: OpAmpPins<'static, T, IN, BIAS, OUT>,
) -> &'static mut OpAmpOutput<'static, T>
where
    T: embassy_stm32::opamp::Instance,
    IN: embassy_stm32::opamp::NonInvertingPin<T> + embassy_stm32::gpio::Pin,
    BIAS: embassy_stm32::opamp::BiasPin<T> + embassy_stm32::gpio::Pin,
    OUT: embassy_stm32::opamp::OutputPin<T> + embassy_stm32::gpio::Pin,
{
    let opamp = cell.init(OpAmp::new(pins.opamp, OpAmpSpeed::HighSpeed));
    // opamp.calibrate();
    output_cell.init(opamp.pga_biased_ext(pins.in_pin, pins.bias_pin, pins.out_pin, pins.gain))
}
