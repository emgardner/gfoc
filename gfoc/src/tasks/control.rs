use crate::tasks::angle::{ANGLE_SIGNAL, AngleReading};

// fn angle_delta(current_angle: AngleReading, last_angle: AngleReading) -> f32 {
//     if current_angle > last_angle {}
// }

// fn get_computed_angle(last_speed: f32, last_angle: AngleReading) -> f32 {}

#[embassy_executor::task]
pub async fn control_task() {
    let mut last_angle: Option<AngleReading> = None;
    let mut ticker = embassy_time::Ticker::every(embassy_time::Duration::from_micros(250));
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
        ticker.next().await;
    }
}
