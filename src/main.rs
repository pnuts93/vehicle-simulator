use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use simulator::{CameraSettings, Movable, MovableConfig, Vehicle};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(Simulator)
        .run();
}

pub struct Simulator;

impl Plugin for Simulator {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (update, camera_follow, grab_mouse));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Main "character"
    let config = MovableConfig {
        spawn: Vec3::ZERO + Vec3::new(0., 0.5, 0.),
        speed: 2.,
        direction: Quat::default(),
    };
    let vehicle = Vehicle::new(&config);
    let camera_settings = CameraSettings::default();
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(camera_settings.get_position(&vehicle))
            .looking_at(vehicle.get_spawn(), Vec3::Y),
        camera_settings,
    ));
    // cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 25, 25))),
        Transform::from_translation(config.spawn),
        vehicle,
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
    commands.spawn((
        PointLight { ..default() },
        Transform::from_xyz(4.0, 0.5, 0.),
    ));
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

fn update(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut vehicle_q: Query<(&mut Transform, &mut Vehicle), Without<CameraSettings>>,
    timer: Res<Time>,
) {
    let Ok((mut vehicle_tf, mut vehicle)) = vehicle_q.single_mut() else {
        return;
    };
    let movement = vehicle.get_speed() * timer.delta_secs();
    if keyboard_input.pressed(KeyCode::KeyW) {
        vehicle.move_z(-movement);
    } else if keyboard_input.pressed(KeyCode::KeyS) {
        vehicle.move_z(movement);
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        vehicle.move_x(-movement);
    } else if keyboard_input.pressed(KeyCode::KeyD) {
        vehicle.move_x(movement);
    }

    vehicle_tf.translation = vehicle.get_spawn();
}

fn camera_follow(
    accumulated_mouse_scroll: Res<AccumulatedMouseScroll>,
    mut vehicle_q: Query<(&Transform, &Vehicle), Without<CameraSettings>>,
    mut cam_q: Query<(&mut Transform, &mut CameraSettings), Without<Vehicle>>,
    timer: Res<Time>,
) {
    let Ok((vehicle_tf, vehicle)) = vehicle_q.single() else {
        return;
    };
    let Ok((mut cam_tf, mut cam)) = cam_q.single_mut() else {
        return;
    };
    if accumulated_mouse_scroll.delta.y < 0. {
        info!("decreasing camera offset, currently {:?}", cam.offset);
        cam.offset.increase();
        info!("decreased camera offset, currently {:?}", cam.offset);
    } else if accumulated_mouse_scroll.delta.y > 0. {
        info!("increasing camera offset");
        cam.offset.decrease();
    }
    cam_tf.translation = cam.get_position(vehicle);
    cam_tf.rotation = cam_tf.looking_at(vehicle.get_spawn(), Vec3::Y).rotation;
}
