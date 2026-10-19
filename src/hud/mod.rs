//! Heads-up display.
//!
//! The screen has four [`HudCorner`] containers, spawned at startup. A widget is a module with a
//! plugin that spawns its UI into a corner (after [`HudSet::Spawn`]) and a system that keeps it up
//! to date; adding one (altitude, weather, a minimap...) means writing that module and adding its
//! plugin to [`HudPlugin`]. Widgets sharing a corner stack in the order they are spawned.

pub mod clock;
pub mod vehicle;

use bevy::prelude::*;

use crate::state::AppState;

pub const HUD_MARGIN: f32 = 16.;
pub const HUD_GAP: f32 = 8.;
pub const HUD_PANEL_COLOR: Color = Color::srgba(0., 0., 0., 0.45);
pub const HUD_TEXT_COLOR: Color = Color::WHITE;
pub const HUD_LABEL_COLOR: Color = Color::srgba(1., 1., 1., 0.6);
pub const HUD_FONT_SIZE: f32 = 24.;
pub const HUD_LARGE_FONT_SIZE: f32 = 48.;
pub const HUD_LABEL_FONT_SIZE: f32 = 16.;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud.in_set(HudSet::Spawn))
            .add_plugins((clock::ClockPlugin, vehicle::VehicleHudPlugin));
    }
}

/// Widgets spawn into the corners after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum HudSet {
    Spawn,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl HudCorner {
    const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    fn node(self) -> Node {
        let (top, bottom, left, right) = match self {
            Self::TopLeft => (
                Val::Px(HUD_MARGIN),
                Val::Auto,
                Val::Px(HUD_MARGIN),
                Val::Auto,
            ),
            Self::TopRight => (
                Val::Px(HUD_MARGIN),
                Val::Auto,
                Val::Auto,
                Val::Px(HUD_MARGIN),
            ),
            Self::BottomLeft => (
                Val::Auto,
                Val::Px(HUD_MARGIN),
                Val::Px(HUD_MARGIN),
                Val::Auto,
            ),
            Self::BottomRight => (
                Val::Auto,
                Val::Px(HUD_MARGIN),
                Val::Auto,
                Val::Px(HUD_MARGIN),
            ),
        };
        let align_items = match self {
            Self::TopLeft | Self::BottomLeft => AlignItems::FlexStart,
            Self::TopRight | Self::BottomRight => AlignItems::FlexEnd,
        };
        Node {
            position_type: PositionType::Absolute,
            top,
            bottom,
            left,
            right,
            flex_direction: FlexDirection::Column,
            align_items,
            row_gap: Val::Px(HUD_GAP),
            ..default()
        }
    }
}

fn spawn_hud(mut commands: Commands) {
    for corner in HudCorner::ALL {
        commands.spawn((DespawnOnExit(AppState::InGame), corner, corner.node()));
    }
}

/// The container of `corner`, once the HUD has been spawned.
pub fn corner_entity(corners: &Query<(Entity, &HudCorner)>, corner: HudCorner) -> Option<Entity> {
    corners
        .iter()
        .find_map(|(entity, c)| (*c == corner).then_some(entity))
}

/// A translucent box for a widget.
pub fn panel() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::FlexEnd,
        row_gap: Val::Px(4.),
        padding: UiRect::all(Val::Px(10.)),
        border_radius: BorderRadius::all(Val::Px(10.)),
        ..default()
    }
}

pub fn panel_bundle() -> impl Bundle {
    (panel(), BackgroundColor(HUD_PANEL_COLOR))
}

/// Text in the HUD's main style; add a marker next to it to update it.
pub fn text(content: impl Into<String>, font_size: f32) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: font_size.into(),
            ..default()
        },
        TextColor(HUD_TEXT_COLOR),
    )
}

/// Small dimmed caption.
pub fn label(content: impl Into<String>) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: HUD_LABEL_FONT_SIZE.into(),
            ..default()
        },
        TextColor(HUD_LABEL_COLOR),
    )
}
