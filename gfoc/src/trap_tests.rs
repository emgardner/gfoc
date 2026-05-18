use crate::DriverState;
use crate::board::CURRENT_SIGNAL;
use crate::control::angle_estimator::AngleEstimator;
use crate::control::{Step, apply_step};
use crate::tasks::angle::{ANGLE_SIGNAL, AngleReading};
use crate::tasks::communication::{COMMAND_CHANNEL, RESPONSE_CHANNEL};
use embassy_stm32::timer::AdvancedInstance4Channel;
use embassy_time::Duration;
use gfoc_proto::{Command, Response, Status};

pub async fn basic_trap<T: AdvancedInstance4Channel>(
    mut motor_driver: &mut embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'_, T>,
) {
    let steps = [Step::S1, Step::S2, Step::S3, Step::S4, Step::S5, Step::S6];
    let mut pwm = 10;
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
    let mut run = false;
    let max_pwm = 800;
    let mut ticker = embassy_time::Ticker::every(Duration::from_micros(50));
    let target_velocity = 25.0;
    defmt::info!("Loop Starting");
    defmt::info!("Max Duty: {}", max_duty);
    loop {
        if let Ok(cmd) = COMMAND_CHANNEL.try_receive() {
            match cmd {
                Command::Stop => {
                    run = false;
                    defmt::info!("stop");
                    motor_driver.set_master_output_enable(false);
                    pwm = 0;
                    RESPONSE_CHANNEL.send(Response::Ack).await;
                }
                Command::Start => {
                    defmt::info!("start");
                    run = true;
                    motor_driver.set_master_output_enable(true);
                    RESPONSE_CHANNEL.send(Response::Ack).await;
                }
                Command::Status => {
                    let currents = match CURRENT_SIGNAL.try_take() {
                        Some(currents) => currents,
                        None => (0.0, 0.0, 0.0, 0.0),
                    };
                    RESPONSE_CHANNEL
                        .send(Response::CyclicStatus(Status {
                            state: if run {
                                gfoc_proto::State::Running
                            } else {
                                gfoc_proto::State::Idle
                            },
                            angle: estimator.angle_now(embassy_time::Instant::now()),
                            velocity: estimator.velocity_rad_s(),
                            current_a: currents.0,
                            current_b: currents.1,
                            current_c: currents.2,
                            v_bus: currents.3,
                        }))
                        .await;
                }
                Command::SetCyclic(_) => {
                    RESPONSE_CHANNEL.send(Response::Ack).await;
                }
            };
        }
        if run {
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
                        // let next_sector = (sector + 1) % 6;
                        let next_sector = (sector + 1) % 6;

                        apply_step(&mut motor_driver, steps[next_sector], pwm, max_duty);
                    } else {
                        // Sensor is stale. Safer than commutating blindly.
                        apply_step(&mut motor_driver, Step::S1, 0, max_duty);
                    }

                    let velocity = estimator.velocity_rad_s();
                    if velocity < target_velocity - 0.1 {
                        if pwm < max_pwm {
                            pwm += 1;
                        }
                    } else if velocity > target_velocity + 0.1 {
                        if pwm > 1 {
                            pwm -= 1;
                        }
                    }
                    if updates >= 1000 {
                        let elapsed_s = duration_to_secs(embassy_time::Instant::now() - start_time);
                        let rps = (elapsed_angle / core::f32::consts::TAU) / elapsed_s;
                        let rpm = rps * 60.0;

                        //defmt::info!("{:?} {:?} {:?} {:?}", elapsed_s, rps, rpm, pwm);
                        defmt::info!("{:?}", elapsed_s);

                        start_time = embassy_time::Instant::now();
                        updates = 0;
                        elapsed_angle = 0.0;

                        // if pwm < max_pwm {
                        //     pwm += 20;
                        // }
                    }
                    ticker.next().await;
                    // Timer::after_micros(5).await;
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
        } else {
            embassy_time::Timer::after_micros(100).await;
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

fn sector_from_aligned_angle(current_mech_rad: f32, align_mech_rad: f32, pole_pairs: f32) -> usize {
    let electrical_angle = wrap_0_tau((current_mech_rad - align_mech_rad) * pole_pairs);

    (electrical_angle * (6.0 / core::f32::consts::TAU)) as usize
}
