//! Ground surface materials.
//!
//! A [`Surface`] on a collider tells wheels touching it how much grip they get. Values are relative
//! to dry asphalt, the reference surface tyres are described on (see
//! [`Tyre`](crate::vehicle::physics::Tyre)): a tyre with peak friction 1.0 reaches μ = 1.0 on
//! asphalt and μ = 0.1 on ice. Colliders without a `Surface` behave as [`Surface::ASPHALT`].

use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// Name shown to the player.
    pub name: &'static str,
    /// Multiplier on the tyre's peak friction.
    pub grip: f32,
    /// Multiplier on the tyre's rolling resistance; soft ground makes the tyre sink in and drag.
    pub rolling_resistance: f32,
}

impl Surface {
    /// Dry asphalt: high grip.
    pub const ASPHALT: Self = Self::new("Asphalt", 1.0, 1.0);
    pub const WET_ASPHALT: Self = Self::new("Wet asphalt", 0.7, 1.0);
    pub const CONCRETE: Self = Self::new("Concrete", 0.95, 1.0);
    pub const GRAVEL: Self = Self::new("Gravel", 0.6, 2.0);
    pub const GRASS: Self = Self::new("Grass", 0.45, 4.0);
    pub const SNOW: Self = Self::new("Snow", 0.3, 3.0);
    pub const ICE: Self = Self::new("Ice", 0.1, 0.8);

    pub const fn new(name: &'static str, grip: f32, rolling_resistance: f32) -> Self {
        Self {
            name,
            grip,
            rolling_resistance,
        }
    }
}

impl Default for Surface {
    fn default() -> Self {
        Self::ASPHALT
    }
}
