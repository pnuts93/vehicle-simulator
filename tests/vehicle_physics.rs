//! Headless checks that the cube settles on its suspension, drives, steers, and grips according
//! to the surface it is on.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use simulator::{
    state::PauseState,
    surface::Surface,
    vehicle::{
        Controlled, DriveInput, VehicleConfig, VehicleState,
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

/// Holds a steering key (1 left, -1 right) for `seconds`, through the vehicle's keyboard steering
/// model like the game does. Returns the largest body slip angle seen, in rad.
fn hold_steering(app: &mut App, vehicle: Entity, direction: f32, seconds: f32) -> f32 {
    let steering = app.world().get::<VehicleConfig>(vehicle).unwrap().steering;
    let mut max_body_slip: f32 = 0.;
    for _ in 0..(seconds / STEP.as_secs_f32()) as usize {
        let velocity = speed(app, vehicle);
        let forward = transform(app, vehicle).rotation * Vec3::Z;
        let input = &mut *app.world_mut().get_mut::<DriveInput>(vehicle).unwrap();
        input.steer = steering.update(
            input.steer,
            direction,
            velocity.dot(forward),
            STEP.as_secs_f32(),
        );
        app.update();
        let body_slip = forward.cross(velocity).y.atan2(forward.dot(velocity));
        max_body_slip = max_body_slip.max(body_slip.abs());
    }
    max_body_slip
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

    hold_steering(&mut app, vehicle, 1., 1.5);
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

#[test]
fn lateral_tyre_curve_peaks_near_8_degrees() {
    let curve = Cube::generate_vehicle_config().tyre.lateral;
    let peak = curve.peak_slip();
    println!("peak slip angle {:.2}°", peak.to_degrees());
    assert!((peak.to_degrees() - 8.27).abs() < 0.1);
    assert!(curve.eval(peak) > 0.9999);
    assert!(curve.eval(peak - 0.01) < curve.eval(peak) && curve.eval(peak + 0.01) < curve.eval(peak));
}

#[test]
fn keyboard_steering_holds_drifts_and_returns() {
    let steering = Cube::generate_vehicle_config().steering;
    let dt = 1. / 60.;
    let hold = |mut steer: f32, direction: f32, speed: f32, seconds: f32| {
        for _ in 0..(seconds / dt).round() as usize {
            steer = steering.update(steer, direction, speed, dt);
        }
        steer
    };
    // Half a second of A at standstill: half lock.
    let steer = hold(0., 1., 0., 0.5);
    assert!((steer - 0.5).abs() < 0.01, "{steer}");
    // Released for a second: mostly held, drifting slowly back.
    let released = hold(steer, 0., 0., 1.);
    assert!((released - 0.2).abs() < 0.01, "{released}");
    // D brings it back to centre quickly, then steers right.
    let returned = hold(released, -1., 0., 4. * dt);
    assert!(returned.abs() < 0.01, "{returned}");
    assert!(hold(returned, -1., 0., 0.2) < -0.15);
    // At 10 m/s the same half second only gives a quarter lock.
    let fast = hold(0., 1., 10., 0.5);
    assert!((fast - 0.25).abs() < 0.01, "{fast}");
}

/// Heading change, front wheel angle and largest body slip angle after holding left for 2 s,
/// coasting from ~15 m/s.
fn turn_at_speed(grip_assist: bool) -> (f32, f32, f32) {
    let (mut app, vehicle) = app(Surface::ASPHALT);
    app.world_mut()
        .get_mut::<VehicleConfig>(vehicle)
        .unwrap()
        .steering
        .grip_assist = grip_assist;
    run(&mut app, 2.);
    app.world_mut().get_mut::<DriveInput>(vehicle).unwrap().throttle = 1.;
    run(&mut app, 3.);
    app.world_mut().get_mut::<DriveInput>(vehicle).unwrap().throttle = 0.;
    let max_body_slip = hold_steering(&mut app, vehicle, 1., 2.);
    let forward = transform(&app, vehicle).rotation * Vec3::Z;
    let wheel_angle = app.world().get::<VehicleState>(vehicle).unwrap().wheels[0].steer;
    (forward.x.atan2(forward.z), wheel_angle, max_body_slip)
}

#[test]
fn grip_assist_turns_tighter_at_speed() {
    let report = |label, (heading, angle, slip): (f32, f32, f32)| {
        println!(
            "{label}: turned {:.1}°, wheel at {:.1}°, body slip up to {:.1}°",
            heading.to_degrees(),
            angle.to_degrees(),
            slip.to_degrees()
        );
    };
    let raw = turn_at_speed(false);
    let assisted = turn_at_speed(true);
    report("raw     ", raw);
    report("assisted", assisted);
    let peak = Cube::generate_vehicle_config().tyre.lateral.peak_slip();
    assert!(assisted.1 < raw.1 * 0.6, "assist did not reduce lock");
    assert!(assisted.1 < peak * 1.5, "wheel far past the tyre's peak");
    assert!(assisted.0 > raw.0, "assist did not turn tighter");
    assert!(assisted.2 < 10_f32.to_radians(), "car slid sideways");
}
