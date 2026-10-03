use bevy::prelude::*;

use crate::vehicle::{ControlProfile, GearConfig, LiftMode, Vehicle, VehicleConfig, VehicleType};

#[derive(Component)]
pub struct Cube;

impl Cube {
    pub fn generate_vehicle(spawn: Vec3) -> Vehicle {
        Vehicle {
            spawn,
            speed: 2.,
            throttle: 0.,
            direction: Quat::default(),
            vtype: VehicleType::Cube,
        }
    }

    pub fn generate_vehicle_config() -> VehicleConfig {
        VehicleConfig {
            gears: vec![GearConfig {
                max_acceleration: 20.,
                max_velocity: 20.,
            }],
            n: 2.,
            reverse: GearConfig {
                max_acceleration: 2.,
                max_velocity: 2.,
            },
            coast_decel: 2.,
            brake_decel: 20.,
            shift_up_ratio: 0.8,
            shift_down_ratio: 0.3,
        }
    }

    pub fn generate_control_profile() -> ControlProfile {
        ControlProfile {
            lift_mode: LiftMode::NoLift,
        }
    }
}
