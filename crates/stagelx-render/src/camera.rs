use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

/// Orbit / pan / zoom controller for the FOH perspective camera.
#[derive(Component, Debug, Clone, Copy)]
pub struct FohCameraController {
    /// Point the camera looks at.
    pub target: Vec3,
    /// Distance from target.
    pub distance: f32,
    /// Horizontal rotation around Y, in radians (0 = looking down -Z).
    pub yaw: f32,
    /// Elevation above the horizontal plane, in radians.
    pub pitch: f32,
    /// Mouse sensitivity for orbit (radians per pixel).
    pub orbit_speed: f32,
    /// Mouse sensitivity for pan (world units per pixel, scaled by distance).
    pub pan_speed: f32,
    /// Scroll-wheel sensitivity (fraction of current distance per line).
    pub zoom_speed: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
}

impl Default for FohCameraController {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 2.0, 0.0),
            distance: 22.8035,
            yaw: 0.0,
            pitch: 0.264,
            orbit_speed: 0.005,
            pan_speed: 0.003,
            zoom_speed: 0.08,
            min_distance: 1.5,
            max_distance: 80.0,
            min_pitch: -1.3,
            max_pitch: 1.3,
        }
    }
}

impl FohCameraController {
    /// Direction vector from target to camera.
    fn direction(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        )
    }
}

// ─── Input ────────────────────────────────────────────────────────────────────

/// Reads mouse input and updates the controller state.
pub fn foh_camera_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut scroll: MessageReader<MouseWheel>,
    mut cursor_opts: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut cameras: Query<&mut FohCameraController>,
) {
    let Ok(mut ctrl) = cameras.single_mut() else { return };
    let motion_delta = mouse_motion.delta;

    // ── Orbit: right mouse drag ──
    if mouse_buttons.pressed(MouseButton::Right) && motion_delta != Vec2::ZERO {
        ctrl.yaw -= motion_delta.x * ctrl.orbit_speed;
        ctrl.pitch += motion_delta.y * ctrl.orbit_speed;
        ctrl.pitch = ctrl.pitch.clamp(ctrl.min_pitch, ctrl.max_pitch);

        // Lock cursor while orbiting for continuous drag across screen edges.
        if let Ok(mut opts) = cursor_opts.single_mut() {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
    }
    // ── Pan: middle mouse drag ──
    else if mouse_buttons.pressed(MouseButton::Middle) && motion_delta != Vec2::ZERO {
        let dir = ctrl.direction();
        let right = dir.cross(Vec3::Y).normalize();
        let up = right.cross(dir).normalize();

        let pan = right * (-motion_delta.x * ctrl.pan_speed * ctrl.distance)
            + up * (motion_delta.y * ctrl.pan_speed * ctrl.distance);
        ctrl.target += pan;
    }
    // Release cursor grab when right button is released.
    else if mouse_buttons.just_released(MouseButton::Right) {
        if let Ok(mut opts) = cursor_opts.single_mut() {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }
    }

    // ── Zoom: scroll wheel ──
    for ev in scroll.read() {
        let delta = match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y * 0.01,
        };
        ctrl.distance -= delta * ctrl.zoom_speed * ctrl.distance;
        ctrl.distance = ctrl.distance.clamp(ctrl.min_distance, ctrl.max_distance);
    }
}

// ─── Apply ────────────────────────────────────────────────────────────────────

/// Writes the controller state into the camera [`Transform`].
pub fn foh_camera_update(mut cameras: Query<(&FohCameraController, &mut Transform)>) {
    for (ctrl, mut transform) in &mut cameras {
        transform.translation = ctrl.target + ctrl.direction() * ctrl.distance;
        transform.look_at(ctrl.target, Vec3::Y);
    }
}
