use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use stagelx_io::IoPlugin;
use stagelx_render::StageLxRenderPlugin;
use stagelx_ui::StageLxUiPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "stageLX — Phase 5".into(),
                resolution: (1600_u32, 900_u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(StageLxUiPlugin)
        .add_plugins(StageLxRenderPlugin)
        .add_plugins(IoPlugin)
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
    info!("  Right-drag  — Orbit FOH camera");
    info!("  Middle-drag — Pan FOH camera");
    info!("  Scroll      — Zoom FOH camera");
}
