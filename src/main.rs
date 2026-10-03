use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use simulator::{
    CameraSettings,
    state::AppState,
    vehicle::{
        CameraIntent, ControlProfile, Controlled, LiftMode, MouseMode, ThrottleMode, Vehicle,
        VehicleConfig, VehicleIntent, VehicleState, cube::Cube,
    },
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<AppState>()
        .add_plugins(Simulator)
        .run();
}

pub struct Simulator;

impl Plugin for Simulator {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                grab_mouse,
                gather_vehicle_input,
                gather_camera_input,
                drive_cube,
            ),
        );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let vehicle = Vehicle::new(Vec3::ZERO + Vec3::new(0., 0.5, 0.), 2., Quat::default());
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
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 25, 25))),
        Transform::from_translation(vehicle.spawn),
        vehicle,
        Cube {},
        Controlled {},
        Cube::generate_control_profile(),
        Cube::generate_vehicle_config(),
        VehicleIntent::default(),
        VehicleState::default(),
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
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(50.)))),
        MeshMaterial3d(materials.add(Color::WHITE)),
    ));
    // light
    commands.spawn((PointLight { ..default() }, Transform::from_xyz(0., 10., 0.)));
}

fn grab_mouse(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Confined;
    }

    if key.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
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

fn drive_cube(
    time: Res<Time>,
    mut q_vehicle: Query<
        (
            &mut Transform,
            &mut VehicleState,
            &mut VehicleIntent,
            &VehicleConfig,
        ),
        (With<Cube>, Without<Camera3d>),
    >,
    mut q_camera: Query<
        (&mut Transform, &mut CameraSettings, &mut CameraIntent),
        (With<Camera3d>, Without<Cube>),
    >,
) {
    let vehicle_spawn;
    let vehicle_direction;
    let vehicle_change;
    if let Ok((mut transform, mut vehicle_state, mut vehicle_intent, vehicle_config)) =
        q_vehicle.single_mut()
    {
        throttle(
            vehicle_config,
            &mut vehicle_state,
            &vehicle_intent.throttle,
            time.delta_secs(),
        );
        transform.rotate_local(Quat::from_euler(
            EulerRot::XYZ,
            0.,
            vehicle_intent.yaw * 0.05,
            0.,
        ));
        let rotation = transform.rotation;
        transform.translation += rotation.normalize()
            * Vec3 {
                x: vehicle_intent.lateral as f32 * 0.05,
                y: vehicle_intent.lift as f32,
                z: vehicle_state.velocity * time.delta_secs(),
            };
        vehicle_spawn = transform.translation;
        vehicle_direction = transform.rotation;
        vehicle_intent.reset();
        vehicle_change = true;
    } else {
        return;
    }
    if let Ok((mut transform, mut camera_settings, mut camera_intent)) = q_camera.single_mut()
        && (!camera_intent.is_default() || vehicle_change)
    {
        transform.translation = camera_settings.get_position(vehicle_direction, vehicle_spawn);
        transform.rotation = transform.looking_at(vehicle_spawn, Vec3::Y).rotation;
        if camera_intent.zoom > 0 {
            camera_settings.offset.increase();
        } else if camera_intent.zoom < 0 {
            camera_settings.offset.decrease();
        }
        camera_intent.reset();
    }
}

fn throttle(
    vehicle_config: &VehicleConfig,
    vehicle_state: &mut VehicleState,
    throttle: &ThrottleMode,
    dt: f32,
) {
    let accel_intent = matches!(throttle, ThrottleMode::Forth);
    let reverse_intent = matches!(throttle, ThrottleMode::Back);

    let a = if reverse_intent && vehicle_state.velocity > 0.01 {
        // reverse pressed but still moving forward: brake first
        -vehicle_config.brake_decel
    } else if accel_intent && vehicle_state.velocity >= 0.0 {
        let gear = &vehicle_config.gears[vehicle_state.current_gear];
        gear.max_acceleration
            * (1.0 - (vehicle_state.velocity / gear.max_velocity).powf(vehicle_config.n))
    } else if reverse_intent {
        // at/near rest, or already reversing
        let g = &vehicle_config.reverse;
        -g.max_acceleration
            * (1.0 - (vehicle_state.velocity.abs() / g.max_velocity).powf(vehicle_config.n))
    } else {
        // coasting: drag toward 0
        -vehicle_state.velocity.signum() * vehicle_config.coast_decel
    };

    vehicle_state.velocity += a * dt;

    // gear shifting (simple threshold version)
    if vehicle_state.velocity > 0.0 {
        let gear = &vehicle_config.gears[vehicle_state.current_gear];
        if vehicle_state.velocity > gear.max_velocity * vehicle_config.shift_up_ratio
            && vehicle_state.current_gear + 1 < vehicle_config.gears.len()
        {
            vehicle_state.current_gear += 1;
        } else if vehicle_state.current_gear > 0 {
            let lower = &vehicle_config.gears[vehicle_state.current_gear - 1];
            if vehicle_state.velocity < lower.max_velocity * vehicle_config.shift_down_ratio {
                vehicle_state.current_gear -= 1;
            }
        }
    }
}
