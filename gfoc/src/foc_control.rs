use crate::control::driver::{
    Direction, DriverControlMode, MotorController, MotorDriver, MotorParams,
};
use embassy_stm32::timer::AdvancedInstance4Channel;
// use foc::Foc;
// use foc::pwm::SpaceVector;

pub async fn basic_control<T: AdvancedInstance4Channel>(
    mut motor_driver: embassy_stm32::timer::complementary_pwm::ComplementaryPwm<'static, T>,
) {
    let max_duty = motor_driver.get_max_duty();
    let desired_dead_time = 0.02;
    let dead_time_counts = max_duty as f32 * desired_dead_time;
    motor_driver.set_dead_time(dead_time_counts as u16);
    let md = MotorDriver::new(motor_driver);
    let mut controller = MotorController::new(
        md,
        DriverControlMode::Torque(0.5, Direction::Cw),
        MotorParams {
            phase_resistance: 2.3,
            pole_pairs: 7,
            kv: Some(220),
            lq: None,
            ld: None,
        },
    );
    controller.enable().await;

    // let mut foc: Foc<SpaceVector, 3400> = Foc::new(
    //     PIController::new(
    //         fixed::FixedI32::from_num(0.0),
    //         fixed::FixedI32::from_num(0.0),
    //     ),
    //     PIController::new(
    //         fixed::FixedI32::from_num(0.0),
    //         fixed::FixedI32::from_num(0.0),
    //     ),
    // );
    defmt::info!("Loop Starting");
    loop {}
}
