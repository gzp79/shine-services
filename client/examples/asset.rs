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
    tasks::BoxedFuture,
    time::Time,
    transform::components::Transform,
    utils::default,
};
use shine_game::{
    app::{init_application, platform, GameSetup, PlatformInit},
    assets::{AssetPlugin, AssetSourcePlugin, GameManifestRequests, GameManifests, WebAssetConfig},
    bevy_ext::ScheduleExt,
};
use std::f32::consts::{FRAC_PI_4, PI};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn main() {
    use shine_game::app::platform::{start_game, Config};

    init_application(GameExample);
    start_game(Config::default());
}

#[cfg(target_family = "wasm")]
pub fn main() {
    init_application(GameExample);
}

#[cfg(target_os = "android")]
pub fn android_main() {
    use shine_game::app::platform::{start_game, Config};

    init_application(GameExample);
    start_game(Config::default());
}

struct GameConfig {
    asset_config: WebAssetConfig,
}

struct GameExample;

impl GameSetup for GameExample {
    type GameConfig = GameConfig;

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {
            let asset_config = WebAssetConfig {
                base_uri: "https://assets.scytta.com".to_string(),
                allow_insecure: false,
                //base_uri: "https://assets.local.scytta.com:8093".to_string(),
                //allow_insecure: true,
                version: None,
            }
            .with_loaded_version()
            .await
            .unwrap();

            GameConfig { asset_config }
        })
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, game_config: GameConfig) {
        app.add_plugins(AssetSourcePlugin::new(game_config.asset_config));
        app.platform_init(config);
        app.add_plugins(AssetPlugin::default());

        app.insert_resource(DirectionalLightShadowMap { size: 4096 });

        app.insert_state(GameState::Loading);

        app.add_systems(Startup, start_loading)
            .add_systems(Update, wait_loading_complete.in_state(GameState::Loading));
        app.add_systems(OnEnter(GameState::Ready), setup_scene)
            .add_systems(Update, animate_light_direction.in_state(GameState::Ready));
    }
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum GameState {
    Ready,
    Loading,
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
