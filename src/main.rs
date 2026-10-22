use avian3d::prelude::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use simulator::{
    CameraSettings,
    hud::HudPlugin,
    radial_menu::{Icon, OptionConfig, RadialMenuAppExt, RadialMenuPlugin, RadialMenuState},
    state::{AppState, PauseState},
    style::{self, StylePlugin},
    surface::Surface,
    terrain::{TerrainPlugin, TerrainSlot, TerrainTileReady},
    vehicle::{
        CameraIntent, ControlProfile, Controlled, DriveInput, LiftMode, MouseMode, ThrottleMode,
        Vehicle, VehicleConfig, VehicleIntent, VehicleState, cube::Cube, heading,
        physics::VehiclePhysicsPlugin,
    },
    vehicle_picker::{CubeAssets, VehiclePickerPlugin, VehiclePickerState},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<AppState>()
        .init_state::<PauseState>()
        .add_plugins((
            StylePlugin,
            HudPlugin,
            RadialMenuPlugin,
            VehiclePickerPlugin,
            VehiclePhysicsPlugin,
            TerrainPlugin,
            Simulator,
        ))
        .run();
}

pub struct Simulator;

impl Plugin for Simulator {
    fn build(&self, app: &mut App) {
        app.add_radial_menu_option(
            OptionConfig::new(
                Icon::Image("icons/vehicle-options.png".into()),
                "Change vehicle",
            ),
            change_vehicle,
        )
        .add_systems(Startup, (setup, hold_physics_until_terrain))
        .add_systems(Update, release_physics_when_terrain_ready)
        .add_systems(OnEnter(PauseState::Paused), spawn_pause_menu)
        .add_systems(
            Update,
            (
                gather_other_input.run_if(in_state(VehiclePickerState::Closed)),
                (
                    (gather_vehicle_input, gather_camera_input)
                        .run_if(in_state(RadialMenuState::Closed))
                        .run_if(in_state(VehiclePickerState::Closed)),
                    (apply_drive_input, follow_camera),
                )
                    .chain()
                    .run_if(in_state(PauseState::Running)),
                handle_pause_menu.run_if(in_state(PauseState::Paused)),
            ),
        );
    }
}

/// Radial menu handler: opens the vehicle picker.
fn change_vehicle(mut next_state: ResMut<NextState<VehiclePickerState>>) {
    next_state.set(VehiclePickerState::Open);
}

#[derive(Component)]
enum PauseMenuButton {
    Resume,
    Exit,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cube: Res<CubeAssets>,
    asset_server: Res<AssetServer>,
) {
    // Spawned a little above its resting ride height, so it settles onto its suspension.
    let vehicle = Vehicle::new(Vec3::new(0., 1., 0.), 2., Quat::default());
    let vehicle_config = Cube::generate_vehicle_config();
    let camera_settings = CameraSettings::default();
    // Resources
    commands.insert_resource(MouseMode::CameraAndSteer);
    // camera
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Camera3d::default(),
        Transform::from_translation(camera_settings.get_position(vehicle.direction, vehicle.spawn))
            .looking_at(vehicle.spawn, Vec3::Y),
        camera_settings,
        CameraIntent::default(),
    ));
    // cube
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Mesh3d(cube.mesh.clone()),
        MeshMaterial3d(cube.material.clone()),
        Transform::from_translation(vehicle.spawn),
        vehicle,
        Cube {},
        Controlled {},
        Cube::generate_rigid_body(),
        Cube::generate_control_profile(),
        VehicleState::new(&vehicle_config),
        vehicle_config,
        VehicleIntent::default(),
        DriveInput::default(),
    ));
    // sky
    commands.spawn((
        Mesh3d(meshes.add(Sphere::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::linear_rgb(0.1, 0.6, 1.0),
            ..default()
        })),
        Transform::default().with_scale(Vec3::splat(-4000.0)),
    ));
    // cylinder
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 5.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(25, 255, 25))),
        Transform::from_xyz(2., 0., 0.),
        RigidBody::Static,
        Collider::cylinder(1.0, 5.0),
    ));
    // ramp, rising towards +Z, to test the suspension
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(4., 0.5, 8.))),
        MeshMaterial3d(materials.add(Color::srgb_u8(120, 120, 140))),
        Transform::from_xyz(0., 0., 20.).with_rotation(Quat::from_rotation_x(-0.2)),
        RigidBody::Static,
        Collider::cuboid(4., 0.5, 8.),
        Surface::CONCRETE,
    ));
    // ground: the only terrain tile we have, centred on the origin with its middle at y = 0
    commands.spawn((
        TerrainSlot(asset_server.load("terrain/CA_center.tif")),
        MeshMaterial3d(materials.add(Color::srgb_u8(110, 130, 80))),
        Surface::GRAVEL,
    ));
    // light
    commands.spawn((PointLight { ..default() }, Transform::from_xyz(0., 10., 0.)));
}

/// Terrain loads asynchronously; until its collider exists the vehicle would fall through the void.
fn hold_physics_until_terrain(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn release_physics_when_terrain_ready(
    mut ready: MessageReader<TerrainTileReady>,
    mut time: ResMut<Time<Physics>>,
    pause_state: Res<State<PauseState>>,
) {
    // A tile becoming ready while paused is covered by the pause menu's own resume.
    if ready.read().count() > 0 && *pause_state.get() == PauseState::Running {
        time.unpause();
    }
}

fn gather_other_input(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
    pause_state: Res<State<PauseState>>,
    mut next_pause_state: ResMut<NextState<PauseState>>,
) {
    match pause_state.get() {
        PauseState::Running => {
            if mouse.just_pressed(MouseButton::Left) {
                grab_cursor(&mut cursor_options);
            }
            if key.just_pressed(KeyCode::Escape) {
                release_cursor(&mut cursor_options);
                next_pause_state.set(PauseState::Paused);
            }
        }
        PauseState::Paused => {
            if key.just_pressed(KeyCode::Escape) {
                grab_cursor(&mut cursor_options);
                next_pause_state.set(PauseState::Running);
            }
        }
    }
}

fn grab_cursor(cursor_options: &mut CursorOptions) {
    cursor_options.visible = false;
    cursor_options.grab_mode = CursorGrabMode::Confined;
}

fn release_cursor(cursor_options: &mut CursorOptions) {
    cursor_options.visible = true;
    cursor_options.grab_mode = CursorGrabMode::Confined;
}

fn spawn_pause_menu(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(PauseState::Paused),
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.6)),
        ))
        .with_children(|parent| {
            for (button, label) in [
                (PauseMenuButton::Resume, "Resume"),
                (PauseMenuButton::Exit, "Exit"),
            ] {
                parent.spawn((button, style::button(label)));
            }
        });
}

fn handle_pause_menu(
    mut cursor_options: Single<&mut CursorOptions>,
    q: Query<(&Interaction, &PauseMenuButton), Changed<Interaction>>,
    mut next_pause_state: ResMut<NextState<PauseState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button) in &q {
        if *interaction == Interaction::Pressed {
            match button {
                PauseMenuButton::Resume => {
                    grab_cursor(&mut cursor_options);
                    next_pause_state.set(PauseState::Running);
                }
                PauseMenuButton::Exit => {
                    exit.write(AppExit::Success);
                }
            }
        }
    }
}

fn gather_vehicle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<AccumulatedMouseMotion>,
    mode: Res<MouseMode>,
    mut q: Query<(&mut VehicleIntent, &ControlProfile), With<Controlled>>,
) {
    if let Ok((mut vehicle_intent, profile)) = q.single_mut() {
        if let MouseMode::CameraAndSteer = *mode {
            vehicle_intent.yaw -= mouse.delta.x;
            vehicle_intent.pitch += mouse.delta.y;
        }
        if let LiftMode::Lift = profile.lift_mode {
            if keys.pressed(KeyCode::Space) {
                vehicle_intent.lift += 1;
            } else if keys.any_pressed(vec![KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
                vehicle_intent.lift -= 1;
            }
        }
        if keys.pressed(KeyCode::KeyW) {
            vehicle_intent.throttle = ThrottleMode::Forth;
        } else if keys.pressed(KeyCode::KeyS) {
            vehicle_intent.throttle = ThrottleMode::Back;
        }
        if keys.pressed(KeyCode::KeyA) {
            vehicle_intent.lateral += 1;
        } else if keys.pressed(KeyCode::KeyD) {
            vehicle_intent.lateral -= 1;
        }
    }
}

fn gather_camera_input(
    scroll: Res<AccumulatedMouseScroll>,
    mouse: Res<AccumulatedMouseMotion>,
    mode: Res<MouseMode>,
    mut q: Query<&mut CameraIntent, With<Camera3d>>,
) {
    if let Ok(mut camera_intent) = q.single_mut() {
        if let MouseMode::OnlyCamera = *mode {
            camera_intent.yaw += mouse.delta.x;
            camera_intent.pitch += mouse.delta.y;
        }
        if scroll.delta.y < 0. {
            camera_intent.zoom -= 1;
        } else if scroll.delta.y > 0. {
            camera_intent.zoom += 1;
        }
    }
}

/// Turns this frame's [`VehicleIntent`] into the held [`DriveInput`] the physics steps read.
fn apply_drive_input(
    time: Res<Time>,
    mut q: Query<
        (
            &mut VehicleIntent,
            &mut DriveInput,
            &VehicleConfig,
            &Transform,
            &LinearVelocity,
        ),
        With<Controlled>,
    >,
) {
    let Ok((mut intent, mut input, config, transform, velocity)) = q.single_mut() else {
        return;
    };
    let direction = match intent.throttle {
        ThrottleMode::Forth => 1.,
        ThrottleMode::Back => -1.,
        ThrottleMode::None => 0.,
    };
    let forward_speed = velocity.dot(transform.rotation * Vec3::Z);
    // Pushing against the direction of travel brakes; once nearly stopped it drives that way.
    if direction * forward_speed < 0. && forward_speed.abs() > 0.5 {
        input.throttle = 0.;
        input.brake = 1.;
    } else {
        input.throttle = direction;
        input.brake = 0.;
    }
    input.steer = config.steering.update(
        input.steer,
        (intent.lateral as f32).clamp(-1., 1.),
        forward_speed,
        time.delta_secs(),
    );
    intent.reset();
}

fn follow_camera(
    q_vehicle: Query<&Transform, (With<Controlled>, Without<Camera3d>)>,
    mut q_camera: Query<
        (&mut Transform, &mut CameraSettings, &mut CameraIntent),
        (With<Camera3d>, Without<Controlled>),
    >,
) {
    let (Ok(vehicle), Ok((mut transform, mut camera_settings, mut camera_intent))) =
        (q_vehicle.single(), q_camera.single_mut())
    else {
        return;
    };
    if camera_intent.zoom > 0 {
        camera_settings.offset.increase();
    } else if camera_intent.zoom < 0 {
        camera_settings.offset.decrease();
    }
    camera_intent.reset();
    // Follow the heading only, so the camera does not pitch and roll with the chassis.
    transform.translation =
        camera_settings.get_position(heading(vehicle.rotation), vehicle.translation);
    transform.look_at(vehicle.translation, Vec3::Y);
}
