pub fn convert_adc_value(adc_value: u16, v_ref: f32, gain: Option<f32>) -> f32 {
    let g: f32 = gain.unwrap_or(1.0);
    ((adc_value as f32 / g) / 4096 as f32) * v_ref
}

pub fn shunt_current(voltage: f32, resistance: f32) -> f32 {
    voltage / resistance
}

pub fn voltage_divider(vin: f32, r1: f32, r2: f32) -> f32 {
    vin * (r2 / (r1 + r2))
}

pub fn voltage_divider_vin(vout: f32, r1: f32, r2: f32) -> f32 {
    vout * ((r1 + r2) / r2)
}

pub fn vbus(voltage: f32) -> f32 {
    voltage_divider_vin(voltage, 169_000.0, 18_000.0)
}

pub fn calculate_temperature(volts: f32, vcc: f32) -> f32 {
    const R_FIXED: f32 = 10000.0; // Fixed resistor value (Ohms)
    const BETA: f32 = 3455.0; // Beta parameter (K)
    const T0: f32 = 298.15; // Reference temperature (Kelvin)
    const R0: f32 = 10000.0; // Resistance at T0 (Ohms)
    if volts <= 0.0 || volts >= vcc {
        return -273.15; // Return absolute zero if invalid
    }
    let r_ntc = R_FIXED * (volts / (vcc - volts));
    let temp_k = 1.0 / ((1.0 / T0) + (1.0 / BETA) * libm::logf(r_ntc / R0));
    let temp_c = temp_k - 273.15;
    temp_c
}

// pub fn clamp<T: PartialOrd>(value: T, min: T, max: T)
