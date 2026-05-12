
Fs - Phase Resistance 0.052 ohms
Fs - Kv 140

2804 Motor Specifications
Model: 2804 Operating Voltage: 7.4 – 16 VDC
Rated Voltage: 12 V (Instantaneous max voltage 20 V) Slot/Poles: 12N14P
Stator Size: 28 mm (12 slots, 14 poles, FOC control, 7 pole pairs) Inner Rotor Shaft Diameter: 8 mm
Bearing Center Hole: 5.4 – 6.5 mm Motor Dimensions: 34.5 × 15 mm
Weight: 37 g Winding Resistance: 5.1 Ω
Phase Resistance Rs: 2.3 Ω Rated Current: 0.5 A (Temperature rise normal under 1 A)
Max Current: 2 A KV Rating: 220
Speed: 2600 RPM / 12 V Winding Inductance: 2.8 mH
Phase Inductance Ls: 0.86 mH Magnetic Flux: 0.0035 Wb
Torque: 300 g·cm (≈0.03 Nm)

pub struct MotorCharacterization {
    phase_resistance: f32,
    pole_pairs: u32,
    kv: Option<u32>,
    lq: Option<f32>,
    ld: Option<f32>,
}

White Ou1
Red Out2
Black Out3



I2C1 SCL   PB8
I2C1 SDA   PB7/PB9
CAN_TX     PB9
CAN_RX     PA11
CAN_SHDC   PC11
CAN_TERM   PC14
STATUS     PC6
Temp Input PB14  NTC 10k with 4.7k
Vbus       PA0  169k 18k
Pot        PB12
Button     PC10
Pwm Input  PA15
USART2_RX  PB4
USART2_TX  PB3
