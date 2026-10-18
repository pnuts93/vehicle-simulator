//! Vehicle picker overlay.
//!
//! Lists the built-in [`VehicleOption::Cube`] (the default) followed by one option per `.glb` file
//! found in `assets/models`. Each option shows a live preview: the model is spawned on its own
//! [`RenderLayers`] layer and rendered by a dedicated camera into an image on a white background.
//! Clicking an option swaps the visual model of the [`Controlled`] vehicle; Escape closes the
//! picker without changes.

use bevy::{
    app::{HierarchyPropagatePlugin, Propagate},
    asset::io::file::FileAssetReader,
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::TextureFormat,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{radial_menu::RadialMenuState, style, vehicle::Controlled};

/// Folder scanned for vehicle models, relative to the assets root.
const MODELS_DIR: &str = "models";
const PREVIEW_SIZE: u32 = 192;
const CARD_GAP: f32 = 16.;
const SELECTED_OUTLINE_COLOR: Color = Color::srgb(0.85, 0.55, 0.1);
/// Vertical offset of a model below the vehicle origin, which sits at the cube's center.
const MODEL_OFFSET_Y: f32 = -0.5;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VehiclePickerState {
    #[default]
    Closed,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VehicleOption {
    Cube,
    /// Asset path of a glTF model, e.g. `models/truck.glb`.
    Model(String),
}

impl VehicleOption {
    pub fn label(&self) -> String {
        match self {
            VehicleOption::Cube => "Cube".into(),
            VehicleOption::Model(path) => std::path::Path::new(path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone()),
        }
    }
}

/// Available vehicles; index 0 is always [`VehicleOption::Cube`].
#[derive(Resource)]
pub struct VehicleCatalog(pub Vec<VehicleOption>);

impl Default for VehicleCatalog {
    fn default() -> Self {
        Self(vec![VehicleOption::Cube])
    }
}

/// Index into [`VehicleCatalog`] of the vehicle currently in use.
#[derive(Resource, Default)]
pub struct SelectedVehicle(pub usize);

/// Shared mesh and material of the cube vehicle.
#[derive(Resource)]
pub struct CubeAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for CubeAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(1.0, 1.0, 1.0));
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(Color::srgb_u8(255, 25, 25));
        Self { mesh, material }
    }
}

pub struct VehiclePickerPlugin;

impl Plugin for VehiclePickerPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<VehiclePickerState>()
            .add_plugins(HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate))
            .insert_resource(discover_vehicles())
            .init_resource::<SelectedVehicle>()
            .init_resource::<CubeAssets>()
            .add_systems(
                OnEnter(VehiclePickerState::Open),
                (release_cursor, spawn_picker),
            )
            .add_systems(OnExit(VehiclePickerState::Open), grab_cursor)
            .add_systems(OnEnter(RadialMenuState::Open), close_picker)
            .add_systems(
                Update,
                (frame_previews, handle_picker_input).run_if(in_state(VehiclePickerState::Open)),
            );
    }
}

/// Builds the catalog from the `.glb` files in the models folder, sorted by file name.
fn discover_vehicles() -> VehicleCatalog {
    let dir = FileAssetReader::get_base_path()
        .join("assets")
        .join(MODELS_DIR);
    let mut models: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"))
            })
            .filter_map(|path| {
                path.file_name()
                    .map(|name| format!("{MODELS_DIR}/{}", name.to_string_lossy()))
            })
            .collect(),
        Err(err) => {
            warn!(
                "Could not read vehicle models from {}: {err}",
                dir.display()
            );
            Vec::new()
        }
    };
    models.sort();
    let mut catalog = VehicleCatalog::default();
    catalog
        .0
        .extend(models.into_iter().map(VehicleOption::Model));
    catalog
}

#[derive(Component)]
struct PickerCard(usize);

/// Root of the model shown in preview `.0`.
#[derive(Component)]
struct PreviewSubject(usize);

/// Camera rendering preview `.0`.
#[derive(Component)]
struct PreviewCamera(usize);

/// Marks a preview whose camera has been positioned around the loaded model.
#[derive(Component)]
struct Framed;

/// Visual model attached to the controlled vehicle when it is not the cube.
#[derive(Component)]
struct VehicleModel;

/// Render layer of preview `index`; layer 0 is the main scene.
fn preview_layer(index: usize) -> usize {
    index + 1
}

fn close_picker(mut next_state: ResMut<NextState<VehiclePickerState>>) {
    next_state.set(VehiclePickerState::Closed);
}

fn release_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = true;
    cursor_options.grab_mode = CursorGrabMode::None;
}

fn grab_cursor(mut cursor_options: Single<&mut CursorOptions>) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Confined;
}

fn spawn_picker(
    mut commands: Commands,
    catalog: Res<VehicleCatalog>,
    selected: Res<SelectedVehicle>,
    cube: Res<CubeAssets>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let previews: Vec<Handle<Image>> = catalog
        .0
        .iter()
        .enumerate()
        .map(|(index, option)| {
            spawn_preview(
                &mut commands,
                index,
                option,
                &cube,
                &mut images,
                &asset_server,
            )
        })
        .collect();
    let all_layers: Vec<usize> = (0..catalog.0.len()).map(preview_layer).collect();
    commands.spawn((
        DespawnOnExit(VehiclePickerState::Open),
        DirectionalLight {
            illuminance: 8_000.,
            ..default()
        },
        Transform::from_xyz(1., 2., 1.5).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::from_layers(&all_layers),
    ));
    spawn_picker_ui(&mut commands, &catalog, selected.0, previews);
}

/// Spawns the model and camera of one preview; returns the image the camera renders into.
fn spawn_preview(
    commands: &mut Commands,
    index: usize,
    option: &VehicleOption,
    cube: &CubeAssets,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
) -> Handle<Image> {
    let layer = RenderLayers::layer(preview_layer(index));
    let mut subject = commands.spawn((
        DespawnOnExit(VehiclePickerState::Open),
        PreviewSubject(index),
        Propagate(layer.clone()),
        Transform::default(),
    ));
    match option {
        VehicleOption::Cube => subject.insert((
            Mesh3d(cube.mesh.clone()),
            MeshMaterial3d(cube.material.clone()),
        )),
        VehicleOption::Model(path) => subject.insert(WorldAssetRoot(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone())),
        )),
    };
    let image = images.add(Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        DespawnOnExit(VehiclePickerState::Open),
        PreviewCamera(index),
        Camera3d::default(),
        Camera {
            order: -1 - index as isize,
            clear_color: ClearColorConfig::Custom(Color::WHITE),
            ..default()
        },
        RenderTarget::from(image.clone()),
        // Keep the white background pure white.
        Tonemapping::None,
        Transform::from_xyz(2., 1.5, 3.).looking_at(Vec3::ZERO, Vec3::Y),
        layer,
    ));
    image
}

/// Points each preview camera at its model once the model's meshes have bounds.
fn frame_previews(
    mut commands: Commands,
    q_subjects: Query<(Entity, &PreviewSubject), Without<Framed>>,
    q_children: Query<&Children>,
    q_bounds: Query<(&GlobalTransform, &Aabb)>,
    mut q_cameras: Query<(&PreviewCamera, &mut Transform, &Projection)>,
) {
    for (subject, preview) in &q_subjects {
        let Some((min, max)) = std::iter::once(subject)
            .chain(q_children.iter_descendants(subject))
            .filter_map(|entity| q_bounds.get(entity).ok())
            .map(|(transform, aabb)| world_bounds(transform, aabb))
            .reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
        else {
            continue;
        };
        let center = (min + max) / 2.;
        let radius = ((max - min).length() / 2.).max(0.01);
        for (camera, mut transform, projection) in &mut q_cameras {
            if camera.0 != preview.0 {
                continue;
            }
            let fov = match projection {
                Projection::Perspective(perspective) => perspective.fov,
                _ => std::f32::consts::FRAC_PI_4,
            };
            let distance = radius / (fov / 2.).sin() * 1.05;
            let direction = Vec3::new(1., 0.6, 1.4).normalize();
            *transform = Transform::from_translation(center + direction * distance)
                .looking_at(center, Vec3::Y);
        }
        commands.entity(subject).insert(Framed);
    }
}

/// World-space axis-aligned bounds of a local `aabb`.
fn world_bounds(transform: &GlobalTransform, aabb: &Aabb) -> (Vec3, Vec3) {
    let (min, max) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
    (0..8)
        .map(|corner| {
            transform.transform_point(Vec3::new(
                if corner & 1 == 0 { min.x } else { max.x },
                if corner & 2 == 0 { min.y } else { max.y },
                if corner & 4 == 0 { min.z } else { max.z },
            ))
        })
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), point| {
            (lo.min(point), hi.max(point))
        })
}

fn spawn_picker_ui(
    commands: &mut Commands,
    catalog: &VehicleCatalog,
    selected: usize,
    previews: Vec<Handle<Image>>,
) {
    commands
        .spawn((
            DespawnOnExit(VehiclePickerState::Open),
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.6)),
        ))
        .with_children(|root| {
            root.spawn(style::button_text("Choose vehicle"));
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(CARD_GAP),
                row_gap: Val::Px(CARD_GAP),
                max_width: Val::Percent(90.),
                ..default()
            })
            .with_children(|grid| {
                for (index, (option, preview)) in catalog.0.iter().zip(previews).enumerate() {
                    let outline_color = if index == selected {
                        SELECTED_OUTLINE_COLOR
                    } else {
                        Color::NONE
                    };
                    grid.spawn((
                        PickerCard(index),
                        Button,
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(8.)),
                            row_gap: Val::Px(8.),
                            border_radius: BorderRadius::all(style::BUTTON_BORDER_RADIUS),
                            ..default()
                        },
                        BackgroundColor(style::BUTTON_COLOR),
                        Outline::new(Val::Px(3.), Val::ZERO, outline_color),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            ImageNode::new(preview),
                            Node {
                                width: Val::Px(PREVIEW_SIZE as f32),
                                height: Val::Px(PREVIEW_SIZE as f32),
                                border_radius: BorderRadius::all(Val::Px(6.)),
                                ..default()
                            },
                        ));
                        card.spawn(style::button_text(option.label()));
                    });
                }
            });
        });
}

/// Applies a clicked card, or closes the picker on Escape.
fn handle_picker_input(
    mut commands: Commands,
    key: Res<ButtonInput<KeyCode>>,
    q_cards: Query<(&Interaction, &PickerCard), Changed<Interaction>>,
    catalog: Res<VehicleCatalog>,
    cube: Res<CubeAssets>,
    asset_server: Res<AssetServer>,
    mut selected: ResMut<SelectedVehicle>,
    q_vehicle: Query<(Entity, Option<&Children>), With<Controlled>>,
    q_models: Query<(), With<VehicleModel>>,
    mut next_state: ResMut<NextState<VehiclePickerState>>,
) {
    if key.just_pressed(KeyCode::Escape) {
        next_state.set(VehiclePickerState::Closed);
        return;
    }
    let Some(index) = q_cards
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, card)| card.0)
    else {
        return;
    };
    next_state.set(VehiclePickerState::Closed);
    if index == selected.0 {
        return;
    }
    selected.0 = index;
    let Ok((vehicle, children)) = q_vehicle.single() else {
        return;
    };
    for child in children.into_iter().flatten() {
        if q_models.contains(*child) {
            commands.entity(*child).despawn();
        }
    }
    let mut vehicle = commands.entity(vehicle);
    match &catalog.0[index] {
        VehicleOption::Cube => {
            vehicle.insert((
                Mesh3d(cube.mesh.clone()),
                MeshMaterial3d(cube.material.clone()),
            ));
        }
        VehicleOption::Model(path) => {
            vehicle
                .remove::<(Mesh3d, MeshMaterial3d<StandardMaterial>)>()
                .with_child((
                    VehicleModel,
                    WorldAssetRoot(
                        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone())),
                    ),
                    Transform::from_xyz(0., MODEL_OFFSET_Y, 0.),
                ));
        }
    }
}
