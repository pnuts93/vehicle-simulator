use avian3d::prelude::*;
use bevy::prelude::*;

use crate::vehicle::{
    ControlProfile, LiftMode, Vehicle, VehicleConfig, VehicleType,
    physics::{Engine, MagicFormula, Steering, Suspension, Tyre, WheelMount},
};

/// Mass of the cube, in kg: roughly a go-kart with its driver.
const MASS: f32 = 300.;

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

    /// Rigid body of the 1 m cube, with the center of mass low to resist rolling over.
    pub fn generate_rigid_body() -> impl Bundle {
        (
            RigidBody::Dynamic,
            Collider::cuboid(1., 1., 1.),
            Mass(MASS),
            CenterOfMass::new(0., -0.4, 0.),
            TransformInterpolation,
        )
    }

    /// Four wheels on the cube's lower corners: front steered, rear driven.
    pub fn generate_vehicle_config() -> VehicleConfig {
        let wheel = |x: f32, z: f32| WheelMount {
            position: Vec3::new(x, -0.2, z),
            steered: z > 0.,
            driven: z < 0.,
        };
        VehicleConfig {
            wheels: vec![
                wheel(0.5, 0.45),
                wheel(-0.5, 0.45),
                wheel(0.5, -0.45),
                wheel(-0.5, -0.45),
            ],
            // Static load is about 740 N per wheel, so ~0.05 m of sag at rest; damping is
            // about half of critical.
            suspension: Suspension {
                rest_length: 0.3,
                stiffness: 15_000.,
                damping: 1_100.,
            },
            tyre: Tyre {
                radius: 0.3,
                inertia: 0.6,
                friction: 1.,
                rolling_resistance: 0.015,
                // Peaks at ~13% slip, ~0.8 of peak when locked or spinning.
                longitudinal: MagicFormula {
                    b: 14.,
                    c: 1.5,
                    e: 0.2,
                },
                // Peaks at ~8°, ~0.8 of peak when sliding sideways.
                lateral: MagicFormula {
                    b: 12.,
                    c: 1.5,
                    e: 0.,
                },
            },
            // Top speed from drag is about 39 m/s (140 km/h).
            engine: Engine {
                max_torque: 400.,
                max_power: 30_000.,
                brake_torque: 250.,
                // About 1.5 m/s² of drag with all four wheels gripping.
                idle_brake: 35.,
            },
            // A tap moves the steering a little; at 10 m/s full lock takes 2 s instead of 1 s.
            steering: Steering {
                max_angle: 0.5,
                attack_rate: 1.,
                attack_speed_reference: 10.,
                return_rate: 3.,
                drift_rate: 0.3,
                grip_assist: true,
            },
            drag_area: 0.8,
        }
    }

    pub fn generate_control_profile() -> ControlProfile {
        ControlProfile {
            lift_mode: LiftMode::NoLift,
        }
    }
}
