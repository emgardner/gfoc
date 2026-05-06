use crate::control::Sector;
use as5600::asynch::As5600;
use defmt::Format;
use embassy_stm32::i2c::Master;
use embassy_stm32::mode::Async;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

#[derive(Debug, Clone, Copy, Format)]
pub struct AngleReading {
    pub angle: f32,
    pub time: embassy_time::Instant,
}

pub static ANGLE_SIGNAL: Signal<CriticalSectionRawMutex, AngleReading> = Signal::new();

#[embassy_executor::task]
pub async fn angle_task(mut as5600: As5600<embassy_stm32::i2c::I2c<'static, Async, Master>>) {
    defmt::info!("Running");
    loop {
        let angle = as5600.angle().await;
        match angle {
            Ok(value) => {
                let rads = (value as f32 / 4096.0) * 2.0 * core::f32::consts::PI;
                ANGLE_SIGNAL.signal(AngleReading {
                    angle: rads,
                    time: embassy_time::Instant::now(),
                });
            }
            Err(e) => match e {
                as5600::error::Error::Communication(e) => {
                    defmt::info!("{:?}", e);
                }
                _ => {}
            },
        }
    }
}

pub static SECTOR_SIGNAL: Signal<CriticalSectionRawMutex, Option<Sector>> = Signal::new();

#[embassy_executor::task]
pub async fn sensorless_task(
    mut bemf: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::mode::Async>,
    bemf1: embassy_stm32::gpio::Input<'static>,
    bemf2: embassy_stm32::gpio::Input<'static>,
    bemf3: embassy_stm32::gpio::Input<'static>,
) {
    defmt::info!("Running");
    // loop {
    //     defmt::info!(
    //         "common={} b1={} b2={} b3={}",
    //         bemf.is_high(),
    //         bemf1.is_high(),
    //         bemf2.is_high(),
    //         bemf3.is_high()
    //     );
    //     Timer::after_millis(100).await;
    // }
    loop {
        bemf.wait_for_any_edge().await;
        let (b1, b2, b3) = (bemf1.is_high(), bemf2.is_high(), bemf3.is_high());
        SECTOR_SIGNAL.signal(Sector::from_bools(b1, b2, b3));
        // defmt::info!(
        //     "{} {} {} {} {}",
        //     b0,
        //     b1,
        //     b2,
        //     b3,
        //     Sector::from_bools(b1, b2, b3)
        // );
    }
}

#[embassy_executor::task]
pub async fn control_task() {
    let mut last_angle: Option<AngleReading> = None;
    loop {
        let new_angle = ANGLE_SIGNAL.wait().await;
        if let Some(ref mut la) = last_angle {
            let dt = new_angle.time - la.time;
            let da = new_angle.angle - la.angle;
            last_angle = Some(new_angle);
            defmt::info!("{:?} {:?}", dt.as_micros(), da);
        } else {
            last_angle = Some(new_angle);
        }
    }
}
