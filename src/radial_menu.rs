//! Hold-to-open radial menu.
//!
//! While [`MENU_KEY`] is held the menu is shown and mouse motion moves a virtual pointer away from
//! the menu center. The option whose angular sector contains the pointer is selected, wherever the
//! pointer is (outside the ring included) as long as it is past a small deadzone. A left click runs
//! the selected option's handler and closes the menu; releasing the key closes it without action.
//!
//! Options are registered with [`RadialMenuAppExt::add_radial_menu_option`]: an [`OptionConfig`]
//! (icon + title) paired with any Bevy system, which is run as a one-shot system on click.

use std::f32::consts::TAU;

use bevy::{ecs::system::SystemId, input::mouse::AccumulatedMouseMotion, prelude::*};

use crate::{state::PauseState, style};

pub const MENU_KEY: KeyCode = KeyCode::Tab;
const MENU_RADIUS: f32 = 160.;
const OPTION_RING_RADIUS: f32 = 110.;
const OPTION_SIZE: f32 = 90.;
const POINTER_SIZE: f32 = 12.;
/// Pointer distance from the center below which nothing is selected.
const DEADZONE: f32 = 20.;

const MENU_BACKGROUND_COLOR: Color = Color::srgba(0., 0., 0., 0.5);
const MENU_OUTLINE_COLOR: Color = Color::srgba(1., 1., 1., 0.3);
const OPTION_SELECTED_COLOR: Color = Color::srgb(0.85, 0.55, 0.1);

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RadialMenuState {
    #[default]
    Closed,
    Open,
}

pub enum Icon {
    /// A short text glyph rendered with the default font.
    Glyph(String),
    /// An asset path to an image, loaded when the menu opens.
    Image(String),
}

pub struct OptionConfig {
    pub icon: Icon,
    pub title: String,
}

impl OptionConfig {
    pub fn new(icon: Icon, title: impl Into<String>) -> Self {
        Self {
            icon,
            title: title.into(),
        }
    }
}

/// Menu options in display order, clockwise starting from the top.
#[derive(Resource, Default)]
pub struct RadialMenuOptions(pub Vec<(OptionConfig, SystemId)>);

pub trait RadialMenuAppExt {
    /// Adds an option to the radial menu; `handler` runs once each time the option is clicked.
    fn add_radial_menu_option<M>(
        &mut self,
        config: OptionConfig,
        handler: impl IntoSystem<(), (), M> + 'static,
    ) -> &mut Self;
}

impl RadialMenuAppExt for App {
    fn add_radial_menu_option<M>(
        &mut self,
        config: OptionConfig,
        handler: impl IntoSystem<(), (), M> + 'static,
    ) -> &mut Self {
        let world = self.world_mut();
        let handler = world.register_system(handler);
        world
            .get_resource_or_init::<RadialMenuOptions>()
            .0
            .push((config, handler));
        self
    }
}

pub struct RadialMenuPlugin;

impl Plugin for RadialMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<RadialMenuState>()
            .init_resource::<RadialMenuOptions>()
            .init_resource::<RadialMenuCursor>()
            .add_systems(OnEnter(RadialMenuState::Open), spawn_radial_menu)
            .add_systems(OnEnter(PauseState::Paused), close_radial_menu)
            .add_systems(
                Update,
                (
                    toggle_radial_menu.run_if(in_state(PauseState::Running)),
                    (track_pointer, update_radial_menu, handle_click)
                        .chain()
                        .run_if(in_state(RadialMenuState::Open)),
                ),
            );
    }
}

#[derive(Resource, Default)]
struct RadialMenuCursor {
    /// Offset from the menu center in UI pixels (y points down).
    pointer: Vec2,
    hovered: Option<usize>,
}

#[derive(Component)]
struct RadialMenuOptionNode(usize);

#[derive(Component)]
struct RadialMenuPointer;

#[derive(Component)]
struct RadialMenuTitle;

fn toggle_radial_menu(
    key: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<RadialMenuState>>,
) {
    if key.just_pressed(MENU_KEY) {
        next_state.set(RadialMenuState::Open);
    } else if key.just_released(MENU_KEY) {
        next_state.set(RadialMenuState::Closed);
    }
}

fn close_radial_menu(mut next_state: ResMut<NextState<RadialMenuState>>) {
    next_state.set(RadialMenuState::Closed);
}

/// Unit direction of option `index` in UI space: the first option is at the top, then clockwise.
fn option_direction(index: usize, count: usize) -> Vec2 {
    let angle = index as f32 * TAU / count as f32;
    Vec2::new(angle.sin(), -angle.cos())
}

/// Index of the option whose angular sector contains `pointer`, if past the deadzone.
fn hovered_option(pointer: Vec2, count: usize) -> Option<usize> {
    if count == 0 || pointer.length() < DEADZONE {
        return None;
    }
    let angle = pointer.x.atan2(-pointer.y).rem_euclid(TAU);
    let sector = TAU / count as f32;
    Some(((angle + sector / 2.) / sector) as usize % count)
}

fn spawn_radial_menu(
    mut commands: Commands,
    options: Res<RadialMenuOptions>,
    mut cursor: ResMut<RadialMenuCursor>,
    asset_server: Res<AssetServer>,
) {
    *cursor = RadialMenuCursor::default();
    let count = options.0.len();
    commands
        .spawn((
            DespawnOnExit(RadialMenuState::Open),
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(2. * MENU_RADIUS),
                    height: Val::Px(2. * MENU_RADIUS),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Percent(50.)),
                    ..default()
                },
                BackgroundColor(MENU_BACKGROUND_COLOR),
                Outline::new(Val::Px(3.), Val::ZERO, MENU_OUTLINE_COLOR),
            ))
            .with_children(|ring| {
                ring.spawn((RadialMenuTitle, style::button_text("")));
                for (index, (config, _)) in options.0.iter().enumerate() {
                    let center = Vec2::splat(MENU_RADIUS)
                        + option_direction(index, count) * OPTION_RING_RADIUS;
                    ring.spawn((
                        RadialMenuOptionNode(index),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(center.x - OPTION_SIZE / 2.),
                            top: Val::Px(center.y - OPTION_SIZE / 2.),
                            width: Val::Px(OPTION_SIZE),
                            height: Val::Px(OPTION_SIZE),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Percent(50.)),
                            ..default()
                        },
                        BackgroundColor(style::BUTTON_COLOR),
                    ))
                    .with_children(|option| {
                        match &config.icon {
                            Icon::Glyph(glyph) => option.spawn(style::button_text(glyph.clone())),
                            Icon::Image(path) => option.spawn((
                                ImageNode::new(asset_server.load(path.clone())),
                                Node {
                                    width: Val::Px(OPTION_SIZE * 0.6),
                                    height: Val::Px(OPTION_SIZE * 0.6),
                                    ..default()
                                },
                            )),
                        };
                    });
                }
                ring.spawn((
                    RadialMenuPointer,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(MENU_RADIUS - POINTER_SIZE / 2.),
                        top: Val::Px(MENU_RADIUS - POINTER_SIZE / 2.),
                        width: Val::Px(POINTER_SIZE),
                        height: Val::Px(POINTER_SIZE),
                        border_radius: BorderRadius::all(Val::Percent(50.)),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                ));
            });
        });
}

fn track_pointer(
    motion: Res<AccumulatedMouseMotion>,
    options: Res<RadialMenuOptions>,
    mut cursor: ResMut<RadialMenuCursor>,
) {
    cursor.pointer = (cursor.pointer + motion.delta).clamp_length_max(MENU_RADIUS);
    cursor.hovered = hovered_option(cursor.pointer, options.0.len());
}

fn update_radial_menu(
    cursor: Res<RadialMenuCursor>,
    options: Res<RadialMenuOptions>,
    mut q_options: Query<(&RadialMenuOptionNode, &mut BackgroundColor)>,
    mut pointer: Single<&mut Node, With<RadialMenuPointer>>,
    mut title: Single<&mut Text, With<RadialMenuTitle>>,
) {
    for (option, mut background) in &mut q_options {
        background.0 = if cursor.hovered == Some(option.0) {
            OPTION_SELECTED_COLOR
        } else {
            style::BUTTON_COLOR
        };
    }
    pointer.left = Val::Px(MENU_RADIUS + cursor.pointer.x - POINTER_SIZE / 2.);
    pointer.top = Val::Px(MENU_RADIUS + cursor.pointer.y - POINTER_SIZE / 2.);
    title.0 = cursor
        .hovered
        .map(|index| options.0[index].0.title.clone())
        .unwrap_or_default();
}

fn handle_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<RadialMenuCursor>,
    options: Res<RadialMenuOptions>,
    mut next_state: ResMut<NextState<RadialMenuState>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if let Some(index) = cursor.hovered {
        commands.run_system(options.0[index].1);
        next_state.set(RadialMenuState::Closed);
    }
}
