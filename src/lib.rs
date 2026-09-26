use bevy::prelude::*;

pub struct MovableConfig {
    pub speed: f32,
    pub spawn: Vec3,
    pub direction: Quat,
}

pub trait Movable {
    fn new(config: &MovableConfig) -> Self;
    fn get_speed(&self) -> f32;
    fn get_spawn(&self) -> Vec3;
}

#[derive(Component)]
pub struct Vehicle {
    spawn: Vec3,
    speed: f32,
    direction: Quat,
}

impl Movable for Vehicle {
    fn new(config: &MovableConfig) -> Self {
        Self {
            spawn: config.spawn,
            speed: config.speed,
            direction: config.direction,
        }
    }

    fn get_speed(&self) -> f32 {
        self.speed
    }

    fn get_spawn(&self) -> Vec3 {
        self.spawn
    }
}

impl Vehicle {
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

#[derive(Debug, Component, Deref, DerefMut)]
pub struct CameraSensitivity(Vec2);

impl Default for CameraSensitivity {
    fn default() -> Self {
        Self(Vec2::new(0.3, 0.2))
    }
}

#[derive(Component)]
pub struct CameraSettings {
    pub sensitivity: CameraSensitivity,
    pub offset: ChaseCameraPreset,
}

impl CameraSettings {
    pub fn new(sensitivity: CameraSensitivity, offset: ChaseCameraPreset) -> Self {
        Self {
            sensitivity,
            offset,
        }
    }

    pub fn get_position(&self, vehicle: &Vehicle) -> Vec3 {
        vehicle.direction * self.offset.offset() + vehicle.get_spawn()
    }
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self::new(CameraSensitivity::default(), ChaseCameraPreset::Standard)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum ChaseCameraPreset {
    Close = 0,
    Standard = 1,
    Wide = 2,
    Overhead = 3,
}

impl ChaseCameraPreset {
    pub fn offset(self) -> Vec3 {
        match self {
            ChaseCameraPreset::Close => Vec3::new(0.0, 1.5, -4.0),
            ChaseCameraPreset::Standard => Vec3::new(0.0, 3.0, -8.0),
            ChaseCameraPreset::Wide => Vec3::new(0.0, 5.0, -14.0),
            ChaseCameraPreset::Overhead => Vec3::new(0.0, 12.0, -20.0),
        }
    }

    pub fn increase(&mut self) {
        if let Ok(new_offset) = ChaseCameraPreset::try_from(*self as u8 + 1) {
            *self = new_offset;
        }
    }

    pub fn decrease(&mut self) {
        if matches!(*self, ChaseCameraPreset::Close) {
            return;
        };
        if let Ok(new_offset) = ChaseCameraPreset::try_from(*self as u8 - 1) {
            *self = new_offset;
        }
    }
}

impl TryFrom<u8> for ChaseCameraPreset {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChaseCameraPreset::Close),
            1 => Ok(ChaseCameraPreset::Standard),
            2 => Ok(ChaseCameraPreset::Wide),
            3 => Ok(ChaseCameraPreset::Overhead),
            _ => Err(()),
        }
    }
}
