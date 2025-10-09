use bevy::{
    app::{App, Startup, Update},
    asset::AssetServer,
    camera::Camera3d,
    ecs::{
        error::BevyError,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    gltf::GltfAssetLabel,
    light::{CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightShadowMap, EnvironmentMapLight},
    math::{EulerRot, Quat, Vec3},
    scene::SceneRoot,
    state::{
        app::AppExtStates,
        state::{NextState, OnEnter, States},
    },
    time::Time,
    transform::components::Transform,
    utils::default,
};
use shine_game::{
    app::{init_application, platform, PlatformInit},
    assets::{AssetPlugin, AssetSourcePlugin, GameManifestRequests, GameManifests},
    bevy_ext::ScheduleExt,
};
use std::f32::consts::{FRAC_PI_4, PI};

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::app::{create_application, platform::Config};

    init_application(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(setup_game);
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    Ready,
    Loading,
}

fn setup_game(app: &mut App, config: &platform::Config) {
    //app.add_plugins(AssetSourcePlugin::new("https://assets.local.scytta.com:8093", true));
    app.add_plugins(AssetSourcePlugin::new("https://assets.scytta.com", false));
    app.platform_init(config);
    app.add_plugins(AssetPlugin::default());

    app.insert_resource(DirectionalLightShadowMap { size: 4096 });

    app.insert_state(GameState::Loading);

    app.add_systems(Startup, start_loading)
        .add_systems(Update, wait_loading_complete.in_state(GameState::Loading));
    app.add_systems(OnEnter(GameState::Ready), setup_scene)
        .add_systems(Update, animate_light_direction.in_state(GameState::Ready));
}

fn start_loading(mut game_manifests: ResMut<GameManifestRequests>) {
    game_manifests.add_manifests(
        [("ui", "ui/assets.json"), ("props", "props/assets.json")].map(|(n, p)| (n.to_string(), p.to_string())),
    );
}

fn wait_loading_complete(game_manifests: Res<GameManifests>, mut next_state: ResMut<NextState<GameState>>) {
    if game_manifests.is_loaded(["props", "ui"]) {
        log::info!("All game manifests loaded.");
        next_state.set(GameState::Ready);
    }
}

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_manifests: ResMut<GameManifests>,
) -> Result<(), BevyError> {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 250.0,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        // This is a relatively small scene, so use tighter shadow
        // cascade bounds than the default for better quality.
        // We also adjusted the shadow map to be larger since we're
        // only using a single cascade.
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 1.6,
            ..default()
        }
        .build(),
    ));

    let asset = game_manifests.resolve_path("props/examplesFlightHelmet")?;
    commands.spawn(SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(asset))));

    Ok(())
}

fn animate_light_direction(time: Res<Time>, mut query: Query<&mut Transform, With<DirectionalLight>>) {
    for mut transform in &mut query {
        transform.rotation = Quat::from_euler(EulerRot::ZYX, 0.0, time.elapsed_secs() * PI / 5.0, -FRAC_PI_4);
    }
}
