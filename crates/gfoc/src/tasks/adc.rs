use crate::board::{
    ADCS_HANDLE, OP1, OP2, OP3, R_SHUNT, SAMPLE_TOGGLE, SHUNT1, SHUNT2, SHUNT3, Shunt1, Shunt2,
    Shunt3, V_REF, VBUS, init_biased_opamp_ext,
};
use crate::utils;
use embassy_stm32::Peri;
use embassy_stm32::adc::AdcChannel;
use embassy_stm32::adc::{Adc, AdcConfig, Exten, InjectedAdcTrigger, SampleTime};
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::typelevel::ADC1_2;
use embassy_stm32::interrupt::typelevel::Interrupt;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

static RAW_CURRENT_SIGNAL: Signal<CriticalSectionRawMutex, (u16, u16, u16, u16)> = Signal::new();
pub static CURRENT_SIGNAL: Signal<CriticalSectionRawMutex, (f32, f32, f32, f32)> = Signal::new();

#[embassy_executor::task]
pub async fn adc_task(
    adc_1: Peri<'static, embassy_stm32::peripherals::ADC1>,
    adc_2: Peri<'static, embassy_stm32::peripherals::ADC2>,
    shunt1: Shunt1,
    shunt2: Shunt2,
    shunt3: Shunt3,
    pa_0: Peri<'static, embassy_stm32::peripherals::PA0>,
    _pb_14: Peri<'static, embassy_stm32::peripherals::PB14>,
    // _pb_12: Peri<'static, embassy_stm32::peripherals::PB12>,
) {
    // Temp Input PB14  NTC 10k with 4.7k ADC1
    // Vbus       PA0  169k 18k  ADC1
    // Pot        PB12
    let adc1 = Adc::new(adc_1, AdcConfig::default());
    let adc2 = Adc::new(adc_2, AdcConfig::default());
    let shunt1 = init_biased_opamp_ext(&OP1, &SHUNT1, shunt1);
    let shunt2 = init_biased_opamp_ext(&OP2, &SHUNT2, shunt2);
    let shunt3 = init_biased_opamp_ext(&OP3, &SHUNT3, shunt3);
    let shunt1_ch = shunt1.degrade_adc();
    let shunt2_ch = shunt2.degrade_adc();
    let shunt3_ch = shunt3.degrade_adc();
    let vbus_ch = VBUS.init(pa_0).degrade_adc();

    let injected_adc_1 = adc1.setup_injected_conversions(
        [
            (shunt1_ch, SampleTime::Cycles25),
            (shunt3_ch, SampleTime::Cycles25),
            (vbus_ch, SampleTime::Cycles25),
        ],
        InjectedAdcTrigger::from(embassy_stm32::triggers::TIM1_TRGO2, Exten::RisingEdge),
        true, // enable JEOS interrupt
    );
    let injected_adc_2 = adc2.setup_injected_conversions(
        [(shunt2_ch, SampleTime::Cycles25)],
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
    loop {
        let (phase_a, phase_b, phase_c, v_bus) = RAW_CURRENT_SIGNAL.wait().await;
        let (voltage_a, voltage_b, voltage_c, v_bus) = (
            utils::convert_adc_value(phase_a, V_REF, Some(16.0)),
            utils::convert_adc_value(phase_b, V_REF, Some(16.0)),
            utils::convert_adc_value(phase_c, V_REF, Some(16.0)),
            utils::convert_adc_value(v_bus, V_REF, Some(1.0)),
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
            let (current_a, current_b, current_c, v_bus) = (
                utils::shunt_current(va, R_SHUNT),
                utils::shunt_current(vb, R_SHUNT),
                utils::shunt_current(vc, R_SHUNT),
                utils::vbus(v_bus),
            );
            CURRENT_SIGNAL.signal((current_a, current_b, current_c, v_bus));
        }
    }
}

#[interrupt]
#[allow(non_snake_case)]
unsafe fn ADC1_2() {
    critical_section::with(|cs| {
        if let Some(sp) = SAMPLE_TOGGLE.borrow(cs).borrow_mut().as_mut() {
            sp.toggle();
        }
        if let Some((injected_adc_1, injected_adc_2)) = ADCS_HANDLE.borrow(cs).borrow_mut().as_mut()
        {
            let mut injected_data_1 = [0u16; 3];
            let mut injected_data_2 = [0u16; 1];
            injected_adc_1.read_injected_samples(&mut injected_data_1);
            injected_adc_2.read_injected_samples(&mut injected_data_2);
            RAW_CURRENT_SIGNAL.signal((
                injected_data_1[0],
                injected_data_2[0],
                injected_data_1[1],
                injected_data_1[2],
            ));
        }
        if let Some(sp) = SAMPLE_TOGGLE.borrow(cs).borrow_mut().as_mut() {
            sp.toggle();
        }
    });
}
