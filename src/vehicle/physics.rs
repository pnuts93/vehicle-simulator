//! Raycast vehicle physics.
//!
//! The chassis is an Avian dynamic rigid body; wheels are not. Each wheel casts a ray from its
//! suspension mount along the chassis' down axis, pushes the chassis up with a spring-damper force,
//! and grips the ground with a tyre force computed from slip (Pacejka "magic formula"). Wheel spin
//! is a simulated state variable, driven by engine and brake torque and by the tyre's reaction.
//! All forces are applied to the chassis at the contact points.
//!
//! Local axes follow the rest of the vehicle code: +Z forward, +Y up, +X left.

use std::f32::consts::{FRAC_PI_2, TAU};

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    state::PauseState,
    surface::Surface,
    vehicle::{Controlled, DriveInput, VehicleConfig, VehicleState, heading},
};

pub const DEBUG_KEY: KeyCode = KeyCode::F3;
pub const RESET_KEY: KeyCode = KeyCode::KeyR;
const AIR_DENSITY: f32 = 1.225;
/// Floor for the speed that slip is measured against; below it the slip definitions blow up.
const LOW_SPEED: f32 = 1.0;
/// Cosine of the angle between chassis `up` and ground normal beyond which the suspension ignores
/// the normal (45°), and below which it follows it completely (30°).
const STEEP_NORMAL_COS: f32 = 0.707;
const GENTLE_NORMAL_COS: f32 = 0.866;
/// Length of a debug force arrow per newton.
const FORCE_GIZMO_SCALE: f32 = 1. / 1000.;

pub struct VehiclePhysicsPlugin;

impl Plugin for VehiclePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PhysicsPlugins::default(), PhysicsDebugPlugin))
            .init_resource::<PhysicsDebug>()
            .add_systems(OnEnter(PauseState::Paused), pause_physics)
            .add_systems(OnExit(PauseState::Paused), resume_physics)
            .add_systems(
                FixedUpdate,
                simulate_wheels.run_if(in_state(PauseState::Running)),
            )
            .add_systems(
                Update,
                (
                    (toggle_physics_debug, reset_vehicle).run_if(in_state(PauseState::Running)),
                    draw_wheel_gizmos.run_if(|debug: Res<PhysicsDebug>| debug.0),
                ),
            );
    }
}

/// Whether physics debug gizmos (colliders, wheels, forces) are drawn.
#[derive(Resource, Deref, DerefMut)]
pub struct PhysicsDebug(pub bool);

impl Default for PhysicsDebug {
    fn default() -> Self {
        Self(true)
    }
}

/// Where a wheel sits on the chassis and what it does.
#[derive(Clone, Copy)]
pub struct WheelMount {
    /// Top of the suspension in chassis-local space; the ray is cast down from here.
    pub position: Vec3,
    pub steered: bool,
    pub driven: bool,
}

#[derive(Clone, Copy)]
pub struct Suspension {
    /// Distance from the mount to the wheel hub when the spring is fully extended, in m.
    pub rest_length: f32,
    /// Spring rate, in N/m.
    pub stiffness: f32,
    /// Damper rate, in N·s/m.
    pub damping: f32,
}

impl Suspension {
    /// Force pushing the chassis away from the ground, in N. A spring-damper that can push but
    /// never pull the chassis down.
    pub fn load(&self, compression: f32, compression_speed: f32) -> f32 {
        (self.stiffness * compression + self.damping * compression_speed).max(0.)
    }

    /// Unit direction of the suspension force: the ground normal, as a contact force should be,
    /// so it stays orthogonal to the tyre force. Surfaces steeper than `STEEP_NORMAL` from the
    /// chassis' `up` (walls, edges) would shove the car sideways, so they blend back to `up`.
    pub fn force_direction(up: Vec3, normal: Vec3) -> Vec3 {
        let alignment = up.dot(normal);
        let t = ((alignment - STEEP_NORMAL_COS) / (GENTLE_NORMAL_COS - STEEP_NORMAL_COS))
            .clamp(0., 1.);
        let t = t * t * (3. - 2. * t);
        up.lerp(normal, t).normalize_or(up)
    }

    /// Factor turning a length measured along the (chassis-aligned) ray into one measured along
    /// the force direction: an oblique ray is longer than the distance to the surface.
    pub fn oblique_scale(up: Vec3, normal: Vec3) -> f32 {
        up.dot(normal).clamp(0.5, 1.)
    }
}

#[derive(Clone, Copy)]
pub struct Tyre {
    pub radius: f32,
    /// Rotational inertia of wheel + tyre about the axle, in kg·m².
    pub inertia: f32,
    /// Peak friction coefficient on dry asphalt; on a [`Surface`] the force is capped at
    /// `friction * surface.grip * load` (friction circle).
    pub friction: f32,
    /// Rolling resistance coefficient on dry asphalt: torque
    /// `rolling_resistance * surface.rolling_resistance * load * radius` slows the wheel.
    pub rolling_resistance: f32,
    /// Normalised force against slip ratio.
    pub longitudinal: MagicFormula,
    /// Normalised force against slip angle (radians).
    pub lateral: MagicFormula,
}

impl Tyre {
    /// How the tyre slides over the ground at wheel `spin`.
    pub fn slip(&self, patch: &ContactPatch, spin: f32) -> Slip {
        let speed = spin * self.radius - patch.forward_speed;
        let reference_speed = patch.forward_speed.abs().max(LOW_SPEED);
        Slip {
            speed,
            ratio: speed / reference_speed,
            angle: (patch.side_speed / reference_speed).atan(),
        }
    }

    /// Grip force of the tyre on the ground, in N, in the patch frame (x forward, y left).
    pub fn force(
        &self,
        patch: &ContactPatch,
        slip: &Slip,
        surface: &Surface,
        load: f32,
        dt: f32,
        g: f32,
    ) -> Vec2 {
        let grip = self.friction * surface.grip * load;
        let force = Vec2::new(
            grip * self.longitudinal.eval(slip.ratio),
            -grip * self.lateral.eval(slip.angle),
        )
        .clamp_length_max(grip);

        // The curves are stiff enough to overshoot at low speed: never push harder than it
        // takes to cancel the slip within this step, given the mass this wheel carries.
        let wheel_mass = load / g;
        let max = Vec2::new(slip.speed.abs(), patch.side_speed.abs()) * wheel_mass / dt;
        force.clamp(-max, max)
    }

    /// Wheel spin after the ground pushes back on the tread with `longitudinal_force`, which
    /// pulls the spin towards free rolling, and after rolling resistance slows it.
    pub fn react(
        &self,
        spin: f32,
        patch: &ContactPatch,
        surface: &Surface,
        longitudinal_force: f32,
        load: f32,
        dt: f32,
    ) -> f32 {
        let rolling_spin = patch.forward_speed / self.radius;
        let reaction = longitudinal_force.abs() * self.radius / self.inertia * dt;
        let spin = approach(spin, rolling_spin, reaction);
        let rolling_resistance = self.rolling_resistance * surface.rolling_resistance;
        let resistance = rolling_resistance * load * self.radius / self.inertia * dt;
        approach(spin, 0., resistance)
    }
}

/// How a tyre slides over the ground; the tyre force is a function of this.
#[derive(Clone, Copy, Debug, Default)]
pub struct Slip {
    /// Tread speed minus ground speed along the wheel, in m/s.
    pub speed: f32,
    /// `speed` relative to the ground speed: 0 when rolling freely, positive when spinning,
    /// -1 when locked.
    pub ratio: f32,
    /// Angle between where the wheel points and where its contact patch moves, in rad;
    /// positive when the patch slides to the left.
    pub angle: f32,
}

/// Simplified Pacejka curve `sin(C·atan(B·s − E·(B·s − atan(B·s))))`, peaking at 1.
///
/// `b` sets the initial slope (stiffness), `c` the shape after the peak, `e` the curvature near it.
#[derive(Clone, Copy)]
pub struct MagicFormula {
    pub b: f32,
    pub c: f32,
    pub e: f32,
}

impl MagicFormula {
    pub fn eval(&self, slip: f32) -> f32 {
        let bs = self.b * slip;
        (self.c * (bs - self.e * (bs - bs.atan())).atan()).sin()
    }

    /// Slip at which the curve peaks. The sine peaks where its argument reaches π/2, i.e. where
    /// `x − E·(x − atan x)` (with `x = B·slip`) reaches `tan(π / 2C)`; solved by bisection.
    /// Curves with `C ≤ 1` never peak and report π/2.
    pub fn peak_slip(&self) -> f32 {
        if self.c <= 1. {
            return FRAC_PI_2;
        }
        let target = (FRAC_PI_2 / self.c).tan();
        let (mut low, mut high) = (0_f32, 1000_f32);
        for _ in 0..50 {
            let x = (low + high) / 2.;
            if x - self.e * (x - x.atan()) < target {
                low = x;
            } else {
                high = x;
            }
        }
        low / self.b
    }
}

/// Constant-power engine without a gearbox: wheel torque is capped by `max_torque` at low wheel
/// speed and by `max_power / spin` above it, which gives a realistic falling acceleration curve.
#[derive(Clone, Copy)]
pub struct Engine {
    /// Total torque at the driven wheels, in N·m.
    pub max_torque: f32,
    /// Total power at the driven wheels, in W.
    pub max_power: f32,
    /// Brake torque per wheel at full brake, in N·m.
    pub brake_torque: f32,
    /// Drag braking per wheel while neither throttle nor brake is pressed, in N·m. Spread over
    /// all wheels rather than the driven ones, so lifting off in a corner does not unsettle the rear.
    pub idle_brake: f32,
}

impl Engine {
    /// Torque shared by all driven wheels at `throttle` (-1 to 1) and wheel `spin`, in N·m.
    pub fn drive_torque(&self, throttle: f32, spin: f32) -> f32 {
        throttle * (self.max_power / spin.abs().max(f32::EPSILON)).min(self.max_torque)
    }
}

/// How keyboard steering moves the driver's steering position, and how far the wheels turn.
///
/// Keys move the steering gradually and it stays roughly where it was left: releasing a key lets
/// it drift slowly back to centre, the opposite key brings it back quickly.
#[derive(Clone, Copy)]
pub struct Steering {
    /// Wheel angle at full lock, in rad.
    pub max_angle: f32,
    /// Speed of the steering position while a key is held, at standstill, in full locks per s.
    pub attack_rate: f32,
    /// Speed at which the attack rate has halved, in m/s; it keeps falling as `1 / (1 + v / this)`.
    pub attack_speed_reference: f32,
    /// Speed of the return to centre while the opposite key is held, in full locks per s.
    pub return_rate: f32,
    /// Speed of the drift back to centre with no key held, in full locks per s.
    pub drift_rate: f32,
    /// Take lock away whenever more would push the tyre past its peak slip angle.
    pub grip_assist: bool,
}

impl Steering {
    /// Steering position (-1 right to 1 left) after `dt` with `direction` held (-1, 0 or 1).
    pub fn update(&self, steer: f32, direction: f32, speed: f32, dt: f32) -> f32 {
        if direction == 0. {
            approach(steer, 0., self.drift_rate * dt)
        } else if steer * direction < 0. {
            approach(steer, 0., self.return_rate * dt)
        } else {
            let rate = self.attack_rate / (1. + speed.abs() / self.attack_speed_reference);
            approach(steer, direction, rate * dt)
        }
    }

    /// Reduces the wheel angle `steer` so the tyre's slip angle stays within `peak_slip_angle`,
    /// given the wheel's `velocity` in chassis space. Past the peak more lock only scrubs speed.
    /// It only ever takes lock away, never steers on its own.
    pub fn limit_to_grip(steer: f32, velocity: Vec3, peak_slip_angle: f32) -> f32 {
        let ground = Vec2::new(velocity.x, velocity.z);
        // Slow enough and the slip stays small whatever the angle (see `Tyre::slip`).
        let max_deviation = (peak_slip_angle.tan() * LOW_SPEED / ground.length())
            .min(1.)
            .asin()
            .max(peak_slip_angle);
        // Where the wheel is going, as a steering angle; when reversing it lines up with its
        // backwards motion.
        let ground = if ground.y < 0. { -ground } else { ground };
        let travel = ground.x.atan2(ground.y);
        let limited = steer.clamp(travel - max_deviation, travel + max_deviation);
        if steer >= 0. {
            limited.clamp(0., steer)
        } else {
            limited.clamp(steer, 0.)
        }
    }
}

/// The ground-plane frame of a wheel's contact patch, and how fast the chassis moves in it.
pub struct ContactPatch {
    pub forward: Vec3,
    pub side: Vec3,
    pub forward_speed: f32,
    pub side_speed: f32,
}

impl ContactPatch {
    pub fn new(axle: Vec3, normal: Vec3, velocity: Vec3) -> Self {
        let forward = axle.cross(normal).normalize_or_zero();
        let side = normal.cross(forward);
        Self {
            forward,
            side,
            forward_speed: velocity.dot(forward),
            side_speed: velocity.dot(side),
        }
    }

    /// `force` in the patch frame (x forward, y left) as a world-space vector.
    pub fn to_world(&self, force: Vec2) -> Vec3 {
        self.forward * force.x + self.side * force.y
    }
}

/// Per-wheel simulation state, rewritten every physics step.
#[derive(Clone, Copy, Default)]
pub struct WheelState {
    /// Angular velocity about the axle, in rad/s; positive rolls forward.
    pub spin: f32,
    /// Accumulated rotation about the axle, for visuals.
    pub angle: f32,
    /// Steering angle, in rad; positive turns left.
    pub steer: f32,
    /// Suspension compression from full extension, in m.
    pub compression: f32,
    pub contact: Option<WheelContact>,
}

#[derive(Clone, Copy)]
pub struct WheelContact {
    pub point: Vec3,
    /// Suspension force, in N.
    pub load: f32,
    /// Tyre force in the ground plane, in N.
    pub tyre_force: Vec3,
    pub slip: Slip,
    pub surface: Surface,
}

/// Moves `value` towards `target` by at most `max_delta`, without overshooting.
fn approach(value: f32, target: f32, max_delta: f32) -> f32 {
    value + (target - value).clamp(-max_delta, max_delta)
}

fn simulate_wheels(
    time: Res<Time>,
    gravity: Res<Gravity>,
    spatial_query: SpatialQuery,
    surfaces: Query<&Surface>,
    mut q: Query<(
        Entity,
        Forces,
        &VehicleConfig,
        &DriveInput,
        &mut VehicleState,
    )>,
) {
    let dt = time.delta_secs();
    if dt <= 0. {
        return;
    }
    let g = gravity.0.length();
    for (entity, mut forces, config, input, mut state) in &mut q {
        let velocity = forces.linear_velocity();
        forces.apply_force(-0.5 * AIR_DENSITY * config.drag_area * velocity.length() * velocity);

        let position = forces.position().0;
        let rotation = forces.rotation().0;
        let up = rotation * Vec3::Y;
        let down = rotation * Dir3::NEG_Y;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let tyre = config.tyre;
        let reach = config.suspension.rest_length + tyre.radius;
        let driven_wheels = config.wheels.iter().filter(|mount| mount.driven).count() as f32;
        let peak_slip_angle = tyre.lateral.peak_slip();

        for (mount, wheel) in config.wheels.iter().zip(&mut state.wheels) {
            let origin = position + rotation * mount.position;
            wheel.steer = if mount.steered {
                let steer = input.steer * config.steering.max_angle;
                if config.steering.grip_assist {
                    let velocity = rotation.inverse() * forces.velocity_at_point(origin);
                    Steering::limit_to_grip(steer, velocity, peak_slip_angle)
                } else {
                    steer
                }
            } else {
                0.
            };
            let mut spin = wheel.spin;
            if mount.driven {
                let torque = config.engine.drive_torque(input.throttle, spin) / driven_wheels;
                spin += torque / tyre.inertia * dt;
            }

            if let Some(hit) = spatial_query.cast_ray(origin, down, reach, true, &filter) {
                let point = origin + down * hit.distance;
                // The damper reacts to how fast the suspension length changes. (The chassis
                // velocity along its up axis is not that: once the body pitches or rolls, part
                // of the forward speed shows up in it.)
                let compression = reach - hit.distance;
                let scale = Suspension::oblique_scale(up, hit.normal);
                let compression_speed = (compression - wheel.compression) / dt;
                let load = config
                    .suspension
                    .load(compression * scale, compression_speed * scale);
                let load_direction = Suspension::force_direction(up, hit.normal);

                let axle = rotation * Quat::from_rotation_y(wheel.steer) * Vec3::X;
                let patch = ContactPatch::new(axle, hit.normal, forces.velocity_at_point(point));
                let surface = surfaces.get(hit.entity).copied().unwrap_or_default();
                let slip = tyre.slip(&patch, spin);
                let force = tyre.force(&patch, &slip, &surface, load, dt, g);
                let tyre_force = patch.to_world(force);
                forces.apply_force_at_point(load_direction * load + tyre_force, point);
                spin = tyre.react(spin, &patch, &surface, force.x, load, dt);

                wheel.compression = compression;
                wheel.contact = Some(WheelContact {
                    point,
                    load,
                    tyre_force,
                    slip,
                    surface,
                });
            } else {
                // Airborne: the wheel only spins up from the engine and slows from the brakes.
                wheel.compression = 0.;
                wheel.contact = None;
            }

            let idle = if input.throttle == 0. && input.brake == 0. {
                config.engine.idle_brake
            } else {
                0.
            };
            let braking =
                (input.brake * config.engine.brake_torque + idle) / tyre.inertia * dt;
            wheel.spin = approach(spin, 0., braking);
            wheel.angle = (wheel.angle + wheel.spin * dt).rem_euclid(TAU);
        }
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}

fn toggle_physics_debug(
    key: Res<ButtonInput<KeyCode>>,
    mut debug: ResMut<PhysicsDebug>,
    mut gizmo_store: ResMut<GizmoConfigStore>,
) {
    if key.just_pressed(DEBUG_KEY) {
        **debug = !**debug;
    }
    if debug.is_changed() {
        gizmo_store.config_mut::<PhysicsGizmos>().0.enabled = **debug;
    }
}

/// Puts the controlled vehicle back on its wheels, keeping its heading.
fn reset_vehicle(
    key: Res<ButtonInput<KeyCode>>,
    mut q: Query<
        (&mut Transform, &mut LinearVelocity, &mut AngularVelocity),
        With<Controlled>,
    >,
) {
    if !key.just_pressed(RESET_KEY) {
        return;
    }
    if let Ok((mut transform, mut linear, mut angular)) = q.single_mut() {
        transform.translation.y += 1.;
        transform.rotation = heading(transform.rotation);
        *linear = LinearVelocity::ZERO;
        *angular = AngularVelocity::ZERO;
    }
}

/// Draws each wheel at its suspension position with a spoke showing its spin, plus the
/// suspension (green) and tyre (orange) forces at the contact point.
fn draw_wheel_gizmos(
    mut gizmos: Gizmos,
    q: Query<(&Transform, &VehicleConfig, &VehicleState)>,
) {
    for (transform, config, state) in &q {
        for (mount, wheel) in config.wheels.iter().zip(&state.wheels) {
            let hub = transform.transform_point(
                mount.position - Vec3::Y * (config.suspension.rest_length - wheel.compression),
            );
            let wheel_rotation = transform.rotation * Quat::from_rotation_y(wheel.steer);
            let color = if wheel.contact.is_some() {
                Color::WHITE
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            };
            // Circles are drawn in the XY plane; turn it to face along the axle.
            gizmos.circle(
                Isometry3d::new(hub, wheel_rotation * Quat::from_rotation_y(FRAC_PI_2)),
                config.tyre.radius,
                color,
            );
            let spoke = wheel_rotation * Quat::from_rotation_x(wheel.angle) * Vec3::Z;
            gizmos.line(hub, hub + spoke * config.tyre.radius, color);
            if let Some(contact) = wheel.contact {
                let up = transform.rotation * Vec3::Y;
                gizmos.arrow(
                    contact.point,
                    contact.point + up * contact.load * FORCE_GIZMO_SCALE,
                    Color::srgb(0.2, 0.9, 0.2),
                );
                gizmos.arrow(
                    contact.point,
                    contact.point + contact.tyre_force * FORCE_GIZMO_SCALE,
                    Color::srgb(1.0, 0.55, 0.1),
                );
            }
        }
    }
}
