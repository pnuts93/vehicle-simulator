//! In-world clock, shown top right.

use bevy::prelude::*;

use super::{HudCorner, HudSet, corner_entity, panel_bundle, text};
use crate::state::PauseState;

const SECONDS_PER_DAY: f32 = 24. * 3600.;

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClock>()
            .add_systems(Startup, spawn_clock.after(HudSet::Spawn))
            .add_systems(
                Update,
                (
                    advance_clock.run_if(in_state(PauseState::Running)),
                    update_clock_text,
                )
                    .chain(),
            );
    }
}

/// Time of day in the simulated world.
#[derive(Resource)]
pub struct WorldClock {
    /// Seconds since midnight.
    pub seconds: f32,
    /// World seconds per real second.
    pub speed: f32,
}

impl Default for WorldClock {
    fn default() -> Self {
        Self {
            seconds: 12. * 3600.,
            speed: 1.,
        }
    }
}

impl WorldClock {
    pub fn hours(&self) -> u32 {
        (self.seconds / 3600.) as u32
    }

    pub fn minutes(&self) -> u32 {
        (self.seconds / 60.) as u32 % 60
    }
}

#[derive(Component)]
struct ClockText;

fn spawn_clock(mut commands: Commands, corners: Query<(Entity, &HudCorner)>) {
    let Some(corner) = corner_entity(&corners, HudCorner::TopRight) else {
        return;
    };
    commands.entity(corner).with_child((
        panel_bundle(),
        children![(ClockText, text("--:--", super::HUD_FONT_SIZE))],
    ));
}

fn advance_clock(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.seconds = (clock.seconds + time.delta_secs() * clock.speed).rem_euclid(SECONDS_PER_DAY);
}

fn update_clock_text(clock: Res<WorldClock>, mut q: Query<&mut Text, With<ClockText>>) {
    for mut text in &mut q {
        text.0 = format!("{:02}:{:02}", clock.hours(), clock.minutes());
    }
}
