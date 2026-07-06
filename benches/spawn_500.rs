//! Synthetic benchmark: spawn 500 fixtures in a minimal Bevy world.
//!
//! Measures the ECS command-queue throughput for fixture instantiation.

use bevy::prelude::*;
use bevy::app::TaskPoolPlugin;
use bevy::asset::AssetPlugin;
use std::time::Instant;

use stagelx_core::types::FixtureId;
use stagelx_render::fixture::{spawn_fixture, FixtureSpawnConfig};
use stagelx_render::beam::BeamMaterial;
use stagelx_render::beam_sprite::BeamSpriteMaterial;

fn main() {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());

    // Asset storages required by spawn_fixture
    app.init_asset::<Mesh>();
    app.init_asset::<Image>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<BeamMaterial>();
    app.init_asset::<BeamSpriteMaterial>();

    app.update();

    let world = app.world_mut();
    let mut state = bevy::ecs::system::SystemState::<(
        Commands,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<BeamMaterial>>,
        ResMut<Assets<BeamSpriteMaterial>>,
    )>::new(world);

    let (mut commands, mut meshes, mut materials, mut beam_mats, mut sprite_mats) =
        state.get_mut(world).unwrap();

    let open_gobo: Handle<Image> = Handle::default();

    // Warm-up: spawn once to ensure any lazy init is done.
    spawn_fixture(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut beam_mats,
        &mut sprite_mats,
        FixtureSpawnConfig {
            id: FixtureId(0),
            position: Vec3::ZERO,
            suspended: true,
            pan_range: 540.0,
            tilt_range: 270.0,
            beam_angle_deg: 10.0,
            body_mesh: None,
        },
        open_gobo.clone(),
    );
    drop(commands);
    drop(meshes);
    drop(materials);
    drop(beam_mats);
    drop(sprite_mats);
    state.apply(world);

    // ── Benchmark: spawn 500 fixtures ─────────────────────────────────────────
    let mut state = bevy::ecs::system::SystemState::<(
        Commands,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<BeamMaterial>>,
        ResMut<Assets<BeamSpriteMaterial>>,
    )>::new(world);

    let (mut commands, mut meshes, mut materials, mut beam_mats, mut sprite_mats) =
        state.get_mut(world).unwrap();

    const COUNT: usize = 500;
    let t0 = Instant::now();
    for i in 0..COUNT {
        spawn_fixture(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut beam_mats,
            &mut sprite_mats,
            FixtureSpawnConfig {
                id: FixtureId(i as u32),
                position: Vec3::new(i as f32 * 1.8 - 450.0, 6.0, 0.0),
                suspended: true,
                pan_range: 540.0,
                tilt_range: 270.0,
                beam_angle_deg: 10.0,
                body_mesh: None,
            },
            open_gobo.clone(),
        );
    }
    let spawn_dt = t0.elapsed();

    drop(commands);
    drop(meshes);
    drop(materials);
    drop(beam_mats);
    drop(sprite_mats);

    let t0 = Instant::now();
    state.apply(world);
    let apply_dt = t0.elapsed();

    println!(
        "Spawn {} fixtures:  queue {:.2?}  +  apply {:.2?}  =  total {:.2?}  ({:.1} fixtures/sec)",
        COUNT,
        spawn_dt,
        apply_dt,
        spawn_dt + apply_dt,
        COUNT as f64 / (spawn_dt + apply_dt).as_secs_f64()
    );
}
