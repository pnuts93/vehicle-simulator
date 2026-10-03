use bevy::prelude::*;

pub mod state;
pub mod vehicle;

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

    pub fn get_position(&self, direction: Quat, spawn: Vec3) -> Vec3 {
        direction * self.offset.offset() + spawn
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
