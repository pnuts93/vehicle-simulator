pub mod cube;

use bevy::prelude::*;

#[derive(Debug, Default)]
pub enum VehicleType {
    #[default]
    Cube,
}

#[derive(Component)]
pub struct Vehicle {
    pub spawn: Vec3,
    pub speed: f32,
    pub throttle: f32,
    pub direction: Quat,
    pub vtype: VehicleType,
}

impl Vehicle {
    pub fn new(spawn: Vec3, speed: f32, direction: Quat) -> Vehicle {
        Self {
            spawn,
            speed,
            throttle: 0.0,
            direction,
            vtype: VehicleType::default(),
        }
    }

    pub fn move_x(&mut self, units: f32) {
        self.spawn.x -= units;
    }

    pub fn move_z(&mut self, units: f32) {
        self.spawn.z -= units;
    }

    pub fn rotate(&mut self, delta_yaw: f32, _delta_pitch: f32, _delta_roll: f32) {
        self.direction =
            (self.direction * Quat::from_euler(EulerRot::YXZ, delta_yaw, 0., 0.)).normalize();
    }
}

#[derive(Component)]
pub struct VehicleConfig {
    pub gears: Vec<GearConfig>, // index 0 = 1st gear, etc.
    pub n: f32,                 // curve exponent (shared across gears for now)
    pub reverse: GearConfig,    // reverse has its own (weaker) accel/max
    pub coast_decel: f32,       // drag toward 0 when no input
    pub brake_decel: f32,       // toward 0 when braking
    pub shift_up_ratio: f32,    // e.g. shift up at 0.9 * gear's max_velocity
    pub shift_down_ratio: f32,  // e.g. shift down below 0.4 * lower gear's max
}

#[derive(Clone, Copy)]
pub struct GearConfig {
    pub max_acceleration: f32,
    pub max_velocity: f32,
}

#[derive(Component, Default)]
pub struct VehicleState {
    pub current_gear: usize, // index into config.gears (ignored if velocity < 0 → reverse)
    pub velocity: f32,       // signed: + forward, - reverse; single source of truth for direction
    pub throttle: f32,       // ramped input value, 0..1 (from the earlier ramping design)
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ThrottleMode {
    #[default]
    None,
    Back,
    Forth,
}

#[derive(Component, Default, PartialEq)]
pub struct VehicleIntent {
    pub throttle: ThrottleMode,
    pub lateral: i32,
    pub yaw: f32,
    pub pitch: f32,
    pub lift: i32,
}

impl VehicleIntent {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Component, Default, PartialEq)]
pub struct CameraIntent {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: i32,
}

impl CameraIntent {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Component)]
pub struct Controlled;

#[derive(Resource)]
pub enum MouseMode {
    CameraAndSteer,
    OnlyCamera,
    Locked,
}

#[derive(Default)]
pub enum LiftMode {
    #[default]
    NoLift,
    Lift,
}

#[derive(Component, Default)]
pub struct ControlProfile {
    pub lift_mode: LiftMode,
}
