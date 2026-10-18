pub mod cube;
pub mod physics;

use bevy::prelude::*;

use physics::{Engine, Suspension, Tyre, WheelMount, WheelState};

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

/// Physical description of a wheeled vehicle; see [`physics`].
#[derive(Component)]
pub struct VehicleConfig {
    pub wheels: Vec<WheelMount>,
    pub suspension: Suspension,
    pub tyre: Tyre,
    pub engine: Engine,
    /// Steering angle of the steered wheels at full lock, in rad.
    pub max_steer_angle: f32,
    /// How fast the steering moves to full lock and back, in units of [`DriveInput::steer`] per s.
    pub steer_rate: f32,
    /// Drag coefficient times frontal area, in m².
    pub drag_area: f32,
}

/// Simulation state of each wheel, in the same order as [`VehicleConfig::wheels`].
#[derive(Component)]
pub struct VehicleState {
    pub wheels: Vec<WheelState>,
}

impl VehicleState {
    pub fn new(config: &VehicleConfig) -> Self {
        Self {
            wheels: vec![WheelState::default(); config.wheels.len()],
        }
    }
}

/// Driver controls as continuous values. Unlike [`VehicleIntent`] they are held between frames,
/// so every fixed physics step sees them.
#[derive(Component, Default)]
pub struct DriveInput {
    /// -1 (full reverse) to 1 (full forward).
    pub throttle: f32,
    /// 0 to 1.
    pub brake: f32,
    /// -1 (full right) to 1 (full left).
    pub steer: f32,
}

/// `rotation` reduced to its rotation about the world Y axis.
pub fn heading(rotation: Quat) -> Quat {
    let forward = rotation * Vec3::Z;
    Quat::from_rotation_y(forward.x.atan2(forward.z))
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
