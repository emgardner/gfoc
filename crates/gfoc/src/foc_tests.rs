use crate::DriverState;
use crate::control::angle_estimator::AngleEstimator;
use crate::control::{Step, apply_pwms, apply_step};
use crate::tasks::adc::CURRENT_SIGNAL;
use crate::tasks::angle::{ANGLE_SIGNAL, AngleReading};
use embassy_stm32::timer::AdvancedInstance4Channel;
use embassy_time::Duration;
use fixed::FixedI32;
use foc::Foc;
use foc::pid::PIController;
use foc::pwm::SpaceVector;

pub async fn basic_foc<T: AdvancedInstance4Channel>(
    mut motor_driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
) {
    let mut foc: Foc<SpaceVector, 3400> = Foc::new(
        PIController::new(
            fixed::FixedI32::from_num(0.0),
            fixed::FixedI32::from_num(0.0),
        ),
        PIController::new(
            fixed::FixedI32::from_num(0.0),
            fixed::FixedI32::from_num(0.0),
        ),
    );
    let max_duty = motor_driver.get_max_duty();
    let desired_dead_time = 0.02;
    let dead_time_counts = max_duty as f32 * desired_dead_time;
    motor_driver.set_dead_time(dead_time_counts as u16);
    motor_driver.set_master_output_enable(true);
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
    let mut estimator = AngleEstimator::new();
    let mut ticker = embassy_time::Ticker::every(Duration::from_micros(50));
    defmt::info!("Loop Starting");
    let mut last_update_time: Option<embassy_time::Instant> = None;
    loop {
        match driver_state {
            DriverState::Align => {
                apply_step(&mut motor_driver, Step::S1, 300, max_duty);
                embassy_time::Timer::after_millis(400).await;
                start_angle = ANGLE_SIGNAL.wait().await;
                apply_step(&mut motor_driver, Step::S2, 300, max_duty);
                embassy_time::Timer::after_millis(400).await;
                let commutated_angle = ANGLE_SIGNAL.wait().await;
                estimator.update(start_angle);
                estimator.update(commutated_angle);
                last_angle = commutated_angle;
                driver_state = DriverState::Run;
                ticker.reset();
            }
            DriverState::Run => {
                if let Some(current_angle) = ANGLE_SIGNAL.try_take() {
                    let delta = compute_actual_angle(last_angle, current_angle);

                    elapsed_angle += delta;
                    updates += 1;

                    estimator.update(current_angle);
                    last_angle = current_angle;
                }
                if let (Some(current), Some(angle)) = (
                    CURRENT_SIGNAL.try_take(),
                    estimator.angle_now(embassy_time::Instant::now()),
                ) {
                    if let Some(last_update) = last_update_time {
                        let now = embassy_time::Instant::now();
                        let dt = now - last_update;
                        last_update_time = Some(now);
                        let ea = electrical_angle(angle);
                        let values = foc.update(
                            [FixedI32::from_num(current.0), FixedI32::from_num(current.1)],
                            FixedI32::from_num(ea),
                            FixedI32::from_num(0.25),
                            FixedI32::from_num(duration_to_secs(dt)),
                        );
                        apply_pwms(
                            motor_driver,
                            values[0].into(),
                            values[1].into(),
                            values[2].into(),
                        );
                        updates += 1;
                    } else {
                        last_update_time = Some(embassy_time::Instant::now());
                    }
                };

                if updates >= 1000 {
                    let elapsed_s = duration_to_secs(embassy_time::Instant::now() - start_time);
                    let rps = (elapsed_angle / core::f32::consts::TAU) / elapsed_s;
                    defmt::info!("{:?} {:?}", elapsed_s, rps);
                    start_time = embassy_time::Instant::now();
                    updates = 0;
                    elapsed_angle = 0.0;
                }
                ticker.next().await;
            }
        }
    }
}

fn duration_to_secs(duration: embassy_time::Duration) -> f32 {
    (duration.as_micros() as f32) / 1_000_000.0
}

fn compute_actual_angle(start_angle: AngleReading, current_angle: AngleReading) -> f32 {
    use core::f32::consts::{PI, TAU};
    let mut delta = current_angle.angle - start_angle.angle;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    delta
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

fn electrical_angle(mechanical_angle: f32) -> f32 {
    let angle = mechanical_angle * 7.0;
    wrap_0_tau(angle)
}
