//! Readouts of the controlled vehicle, shown bottom right: speed, steering and the surface
//! under the wheels.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{
    HUD_FONT_SIZE, HUD_LARGE_FONT_SIZE, HUD_TEXT_COLOR, HudCorner, HudSet, corner_entity, label,
    panel_bundle, text,
};
use crate::vehicle::{Controlled, DriveInput, VehicleConfig, VehicleState};

const STEER_BAR_WIDTH: f32 = 180.;
const STEER_BAR_HEIGHT: f32 = 8.;
const STEER_BAR_COLOR: Color = Color::srgba(1., 1., 1., 0.2);

pub struct VehicleHudPlugin;

impl Plugin for VehicleHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_vehicle_hud.after(HudSet::Spawn))
            .add_systems(Update, (update_speed, update_steering, update_surface));
    }
}

#[derive(Component)]
struct SpeedText;

#[derive(Component)]
struct SteeringText;

#[derive(Component)]
struct SurfaceText;

/// One half of the steering bar; it fills outwards from the centre.
#[derive(Component, Clone, Copy, PartialEq)]
enum SteeringFill {
    Left,
    Right,
}

fn spawn_vehicle_hud(mut commands: Commands, corners: Query<(Entity, &HudCorner)>) {
    let Some(corner) = corner_entity(&corners, HudCorner::BottomRight) else {
        return;
    };
    commands.entity(corner).with_child((
        panel_bundle(),
        children![
            (SpeedText, text("0 km/h", HUD_LARGE_FONT_SIZE)),
            steering_bar(),
            (SteeringText, text("0 deg", HUD_FONT_SIZE)),
            label("Surface"),
            (SurfaceText, text("-", HUD_FONT_SIZE)),
        ],
    ));
}

fn steering_bar() -> impl Bundle {
    (
        Node {
            width: Val::Px(STEER_BAR_WIDTH),
            height: Val::Px(STEER_BAR_HEIGHT),
            border_radius: BorderRadius::all(Val::Px(STEER_BAR_HEIGHT / 2.)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(STEER_BAR_COLOR),
        children![
            steering_half(SteeringFill::Left, JustifyContent::FlexEnd),
            steering_half(SteeringFill::Right, JustifyContent::FlexStart),
        ],
    )
}

fn steering_half(side: SteeringFill, justify_content: JustifyContent) -> impl Bundle {
    (
        Node {
            width: Val::Percent(50.),
            height: Val::Percent(100.),
            justify_content,
            ..default()
        },
        children![(
            side,
            Node {
                width: Val::Percent(0.),
                height: Val::Percent(100.),
                ..default()
            },
            BackgroundColor(HUD_TEXT_COLOR),
        )],
    )
}

fn update_speed(
    vehicle: Query<&LinearVelocity, With<Controlled>>,
    mut q: Query<&mut Text, With<SpeedText>>,
) {
    let (Ok(velocity), Ok(mut text)) = (vehicle.single(), q.single_mut()) else {
        return;
    };
    text.0 = format!("{:.0} km/h", velocity.length() * 3.6);
}

fn update_steering(
    vehicle: Query<(&DriveInput, &VehicleConfig), With<Controlled>>,
    mut text: Query<&mut Text, With<SteeringText>>,
    mut fills: Query<(&SteeringFill, &mut Node)>,
) {
    let Ok((input, config)) = vehicle.single() else {
        return;
    };
    // steer is -1 (full right) to 1 (full left).
    for (side, mut node) in &mut fills {
        let amount = match side {
            SteeringFill::Left => input.steer.max(0.),
            SteeringFill::Right => (-input.steer).max(0.),
        };
        node.width = Val::Percent(amount * 100.);
    }
    if let Ok(mut text) = text.single_mut() {
        let degrees = (input.steer * config.steering.max_angle).to_degrees();
        text.0 = match degrees {
            d if d >= 0.5 => format!("L {d:.0} deg"),
            d if d <= -0.5 => format!("R {:.0} deg", -d),
            _ => "0 deg".to_string(),
        };
    }
}

fn update_surface(
    vehicle: Query<&VehicleState, With<Controlled>>,
    mut q: Query<&mut Text, With<SurfaceText>>,
) {
    let (Ok(state), Ok(mut text)) = (vehicle.single(), q.single_mut()) else {
        return;
    };
    text.0 = state
        .wheels
        .iter()
        .find_map(|wheel| wheel.contact.map(|contact| contact.surface.name))
        .unwrap_or("Airborne")
        .to_string();
}
