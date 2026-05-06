use crate::utils;
use core::cell::RefCell;
use embassy_stm32::adc::AdcChannel;
use embassy_stm32::adc::{Adc, AdcConfig, Exten, InjectedAdc, InjectedAdcTrigger, SampleTime};
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::typelevel::ADC1_2;
use embassy_stm32::interrupt::typelevel::Interrupt;
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

static ADCS_HANDLE: CriticalSectionMutex<
    RefCell<Option<(InjectedAdc<AdcRegs>, InjectedAdc<AdcRegs>)>>,
> = CriticalSectionMutex::new(RefCell::new(None));

static OP1: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP1>> =
    StaticCell::new();
static OP2: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP2>> =
    StaticCell::new();
static OP3: StaticCell<embassy_stm32::opamp::OpAmp<'static, peripherals::OPAMP3>> =
    StaticCell::new();

static SHUNT1: StaticCell<OpAmpOutput<'static, peripherals::OPAMP1>> = StaticCell::new();
static SHUNT2: StaticCell<OpAmpOutput<'static, peripherals::OPAMP2>> = StaticCell::new();
static SHUNT3: StaticCell<OpAmpOutput<'static, peripherals::OPAMP3>> = StaticCell::new();
static VBUS: StaticCell<Peri<'static, peripherals::PA0>> = StaticCell::new();

static CURRENT_SIGNAL: Signal<CriticalSectionRawMutex, (u16, u16, u16, u16)> = Signal::new();

#[embassy_executor::task]
pub async fn adc_task(
    adc_1: Peri<'static, embassy_stm32::peripherals::ADC1>,
    adc_2: Peri<'static, embassy_stm32::peripherals::ADC2>,
    opamp_1: Peri<'static, embassy_stm32::peripherals::OPAMP1>,
    opamp_2: Peri<'static, embassy_stm32::peripherals::OPAMP2>,
    opamp_3: Peri<'static, embassy_stm32::peripherals::OPAMP3>,
    pa_1: Peri<'static, embassy_stm32::peripherals::PA1>,
    pa_2: Peri<'static, embassy_stm32::peripherals::PA2>,
    pa_3: Peri<'static, embassy_stm32::peripherals::PA3>,
    pa_5: Peri<'static, embassy_stm32::peripherals::PA5>,
    pa_6: Peri<'static, embassy_stm32::peripherals::PA6>,
    pa_7: Peri<'static, embassy_stm32::peripherals::PA7>,
    pb_0: Peri<'static, embassy_stm32::peripherals::PB0>,
    pb_1: Peri<'static, embassy_stm32::peripherals::PB1>,
    pb_2: Peri<'static, embassy_stm32::peripherals::PB2>,
    pa_0: Peri<'static, embassy_stm32::peripherals::PA0>,
    _pb_14: Peri<'static, embassy_stm32::peripherals::PB14>,
    // _pb_12: Peri<'static, embassy_stm32::peripherals::PB12>,
) {
    // Temp Input PB14  NTC 10k with 4.7k ADC1
    // Vbus       PA0  169k 18k  ADC1
    // Pot        PB12
    let adc1 = Adc::new(adc_1, AdcConfig::default());
    let adc2 = Adc::new(adc_2, AdcConfig::default());
    let op1 = OP1.init(OpAmp::new(opamp_1, OpAmpSpeed::HighSpeed));
    let op2 = OP2.init(OpAmp::new(opamp_2, OpAmpSpeed::HighSpeed));
    let op3 = OP3.init(OpAmp::new(opamp_3, OpAmpSpeed::HighSpeed));

    let shunt1 = SHUNT1.init(op1.pga_biased_ext(pa_1, pa_3, pa_2, OpAmpGain::Mul16));
    let shunt2 = SHUNT2.init(op2.pga_biased_ext(pa_7, pa_5, pa_6, OpAmpGain::Mul16));
    let shunt3 = SHUNT3.init(op3.pga_biased_ext(pb_0, pb_2, pb_1, OpAmpGain::Mul16));

    let shunt1_ch = shunt1.degrade_adc();
    let shunt2_ch = shunt2.degrade_adc();
    let shunt3_ch = shunt3.degrade_adc();
    let vbus_ch = VBUS.init(pa_0).degrade_adc();

    let injected_adc_1 = adc1.setup_injected_conversions(
        [
            (shunt1_ch, SampleTime::Cycles125),
            (shunt3_ch, SampleTime::Cycles125),
            (vbus_ch, SampleTime::Cycles125),
        ],
        InjectedAdcTrigger::from(embassy_stm32::triggers::TIM1_TRGO2, Exten::RisingEdge),
        true, // enable JEOS interrupt
    );
    // let injected_adc_1 = adc1.into_ring_buffered_and_injected(
    //     [
    //         (shunt1_ch, SampleTime::Cycles125),
    //         (shunt3_ch, SampleTime::Cycles125),
    //     ],
    //     InjectedAdcTrigger::from(embassy_stm32::triggers::TIM1_TRGO2, Exten::RisingEdge),
    //     true, // enable JEOS interrupt
    // );

    let injected_adc_2 = adc2.setup_injected_conversions(
        [(shunt2_ch, SampleTime::Cycles125)],
        InjectedAdcTrigger::from(embassy_stm32::triggers::TIM1_TRGO2, Exten::RisingEdge),
        true, // enable JEOS interrupt
    );

    critical_section::with(|cs| {
        ADCS_HANDLE
            .borrow(cs)
            .replace(Some((injected_adc_1, injected_adc_2)));
    });

    unsafe { ADC1_2::enable() };
    let mut avg_count = 0;
    let mut pha_sum = 0.0;
    let mut phb_sum = 0.0;
    let mut phc_sum = 0.0;
    let mut pha_avg = 0.0;
    let mut phb_avg = 0.0;
    let mut phc_avg = 0.0;
    // let mut sample_time = embassy_time::Instant::now();
    loop {
        let (phase_a, phase_b, phase_c, v_bus) = CURRENT_SIGNAL.wait().await;
        let (voltage_a, voltage_b, voltage_c, v_bus) = (
            utils::convert_adc_value(phase_a, V_REF, Some(16.0)),
            utils::convert_adc_value(phase_b, V_REF, Some(16.0)),
            utils::convert_adc_value(phase_c, V_REF, Some(16.0)),
            utils::convert_adc_value(v_bus, V_REF, Some(16.0)),
        );
        if avg_count < 10 {
            pha_sum += voltage_a;
            phb_sum += voltage_b;
            phc_sum += voltage_c;
            avg_count += 1;
            if avg_count == 10 {
                pha_avg = pha_sum / 10.0;
                phb_avg = phb_sum / 10.0;
                phc_avg = phc_sum / 10.0;
            }
        } else {
            let (va, vb, vc) = (
                voltage_a - pha_avg,
                voltage_b - phb_avg,
                voltage_c - phc_avg,
            );
            let (_current_a, _current_b, _current_c, _v_bus) = (
                utils::shunt_current(va, R_SHUNT),
                utils::shunt_current(vb, R_SHUNT),
                utils::shunt_current(vc, R_SHUNT),
                utils::voltage_divider_vin(v_bus, 169_000.0, 18_000.0),
            );
            // let now = embassy_time::Instant::now();
            // if (now - sample_time).as_millis() > 100 {
            //     sample_time = now;
            //     defmt::info!("{} {} {}", _current_a, _current_b, _current_c);
            // }
            // defmt::info!("{} {} {}", voltage_a, voltage_b, voltage_c);
        }

        // Timer::after_micros(300).await;
    }
}

#[interrupt]
#[allow(non_snake_case)]
unsafe fn ADC1_2() {
    critical_section::with(|cs| {
        if let Some((injected_adc_1, injected_adc_2)) = ADCS_HANDLE.borrow(cs).borrow_mut().as_mut()
        {
            let mut injected_data_1 = [0u16; 3];
            let mut injected_data_2 = [0u16; 1];
            injected_adc_1.read_injected_samples(&mut injected_data_1);
            injected_adc_2.read_injected_samples(&mut injected_data_2);
            CURRENT_SIGNAL.signal((
                injected_data_1[0],
                injected_data_2[0],
                injected_data_1[1],
                injected_data_1[2],
            ));
        }
    });
}
