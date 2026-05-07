use bevy::prelude::*;
use stagelx_render::StageLxRenderPlugin;
use stagelx_ui::StageLxUiPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "stageLX — Phase 1".into(),
                resolution: (1600.0, 900.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(StageLxRenderPlugin)
        .add_plugins(StageLxUiPlugin)
        .add_systems(Startup, print_controls)
        .run();
}

fn print_controls() {
    info!("stageLX controls:");
    info!("  Arrow keys  — Pan / Tilt");
    info!("  + / -       — Dimmer up / down");
    info!("  W           — White");
    info!("  X           — Red");
    info!("  C           — Cyan/blue");
    info!("  R/G/B       — Nudge red/green/blue channel up");
}
