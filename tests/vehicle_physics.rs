//! Headless checks that the cube settles on its suspension, drives, steers, and grips according
//! to the surface it is on.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use simulator::{
    state::PauseState,
    surface::Surface,
    vehicle::{
        Controlled, DriveInput, VehicleState,
        cube::Cube,
        physics::{PhysicsDebug, VehiclePhysicsPlugin},
    },
};

const STEP: Duration = Duration::from_micros(15_625);

fn app(surface: Surface) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        bevy::input::InputPlugin,
        TransformPlugin,
        AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::gizmos::GizmoPlugin,
        VehiclePhysicsPlugin,
    ))
    .init_state::<PauseState>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
    .insert_resource(PhysicsDebug(false));
    // `app.update()` alone skips the plugins' `finish` hooks, which Avian relies on.
    app.finish();
    app.cleanup();
    app.world_mut()
        .spawn((RigidBody::Static, Collider::half_space(Vec3::Y), surface));
    let config = Cube::generate_vehicle_config();
    let vehicle = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0., 1., 0.),
            Cube::generate_rigid_body(),
            VehicleState::new(&config),
            config,
            DriveInput::default(),
            Controlled,
        ))
        .id();
    (app, vehicle)
}

fn run(app: &mut App, seconds: f32) {
    for _ in 0..(seconds / STEP.as_secs_f32()) as usize {
        app.update();
    }
}

fn transform(app: &App, vehicle: Entity) -> Transform {
    *app.world().get::<Transform>(vehicle).unwrap()
}

fn speed(app: &App, vehicle: Entity) -> Vec3 {
    app.world().get::<LinearVelocity>(vehicle).unwrap().0
}

#[test]
fn settles_drives_and_steers() {
    let (mut app, vehicle) = app(Surface::ASPHALT);

    run(&mut app, 3.);
    let rest = transform(&app, vehicle);
    println!("rest: {:?}, v = {:?}", rest.translation, speed(&app, vehicle));
    // Mount at -0.2, rest length 0.3, radius 0.3, ~0.05 m sag: centre ~0.75 m above ground.
    assert!((rest.translation.y - 0.75).abs() < 0.05, "ride height {}", rest.translation.y);
    assert!(speed(&app, vehicle).length() < 0.05, "still moving at rest");

    app.world_mut().get_mut::<DriveInput>(vehicle).unwrap().throttle = 1.;
    run(&mut app, 3.);
    let v = speed(&app, vehicle);
    println!("after 3 s throttle: {v:?} ({:.1} m/s)", v.length());
    assert!(v.z > 5. && v.z < 20., "forward speed {}", v.z);
    assert!(v.x.abs() < 0.5, "drifting sideways {}", v.x);

    app.world_mut().get_mut::<DriveInput>(vehicle).unwrap().steer = 1.;
    run(&mut app, 1.5);
    let turned = transform(&app, vehicle);
    let forward = turned.rotation * Vec3::Z;
    println!("after steering left: forward {forward:?}, up {:?}", turned.rotation * Vec3::Y);
    assert!(forward.x > 0.3, "did not turn left: {forward:?}");
    assert!((turned.rotation * Vec3::Y).y > 0.9, "rolled over");

    let input = &mut *app.world_mut().get_mut::<DriveInput>(vehicle).unwrap();
    (input.throttle, input.brake, input.steer) = (0., 1., 0.);
    run(&mut app, 4.);
    println!("after braking: {:?}", speed(&app, vehicle));
    assert!(speed(&app, vehicle).length() < 0.5, "did not stop");
}

/// Average forward speed of the driven (rear) wheels' treads minus the ground speed, in m/s.
fn rear_slip_speed(app: &App, vehicle: Entity) -> f32 {
    let state = app.world().get::<VehicleState>(vehicle).unwrap();
    let tread_speed = (state.wheels[2].spin + state.wheels[3].spin) / 2. * 0.3;
    tread_speed - speed(app, vehicle).z
}

#[test]
fn grip_depends_on_surface() {
    let launch = |surface| {
        let (mut app, vehicle) = app(surface);
        run(&mut app, 3.);
        app.world_mut().get_mut::<DriveInput>(vehicle).unwrap().throttle = 1.;
        run(&mut app, 1.);
        (speed(&app, vehicle).z, rear_slip_speed(&app, vehicle))
    };
    let (asphalt_speed, asphalt_slip) = launch(Surface::ASPHALT);
    let (ice_speed, ice_slip) = launch(Surface::ICE);
    println!("1 s full throttle: asphalt {asphalt_speed:.2} m/s (slip {asphalt_slip:.2} m/s)");
    println!("                   ice     {ice_speed:.2} m/s (slip {ice_slip:.2} m/s)");
    // On ice the tyres can't transmit the engine's torque: the car is slower and the wheels spin.
    assert!(ice_speed < asphalt_speed * 0.4, "ice not slower");
    assert!(ice_slip > asphalt_slip * 5. && ice_slip > 2., "wheels not spinning on ice");
}
