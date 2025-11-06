use bevy::{
    app::{App, Startup, Update},
    asset::Assets,
    camera::Camera2d,
    color::{palettes::css, Color},
    ecs::system::{Commands, Query, ResMut},
    gizmos::gizmos::Gizmos,
    math::{primitives::Cuboid, Vec3},
    mesh::{Mesh, Mesh3d},
    pbr::{MeshMaterial3d, StandardMaterial},
    render::view::NoIndirectDrawing,
    tasks::BoxedFuture,
    transform::components::Transform,
    window::Window,
};
use shine_game::{
    app::{init_application, platform, GameSetup, PlatformInit},
    math::value::{AnimatedValue, IntoAnimatedVariable},
};

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

struct GameExample;

impl GameSetup for GameExample {
    type GameConfig = ();

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {})
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, _game_config: ()) {
        app.platform_init(config);

        app.add_systems(Startup, spawn_world);
        app.add_systems(Update, (show_axis, show_exp_smooth_curve));
    }
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut window = windows.single_mut().unwrap();
    window.title = "Curves".to_string();

    let player = (
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    );
    commands.spawn(player);

    let camera = {
        (
            Camera2d,
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            Transform::from_scale(Vec3::splat(0.05)).with_translation(Vec3::new(MAX_T * SCALE_T * 0.5, END, 0.0)),
        )
    };
    commands.spawn(camera);
}

const START: f32 = 3.0;
const END: f32 = 10.0;
const SCALE_T: f32 = 10.0;
const MAX_T: f32 = 1.5;
const AXIS_T: f32 = 0.1;
const DELTA_T: f32 = 0.01;

fn show_axis(mut gizmos: Gizmos) {
    const MAX_Y: f32 = 2.0 * END;

    gizmos.line(Vec3::ZERO, Vec3::new(1.5 * MAX_T * SCALE_T, 0.0, 0.0), css::GRAY);
    let mut t = 0.0;
    while t < MAX_T * 1.5 {
        gizmos.line(
            Vec3::new(t * SCALE_T, -0.25, 0.0),
            Vec3::new(t * SCALE_T, MAX_Y, 0.0),
            css::DARK_SLATE_GRAY,
        );
        t += AXIS_T;
    }

    gizmos.line(Vec3::ZERO, Vec3::new(0.0, MAX_Y, 0.0), css::GRAY);
    gizmos.line(
        Vec3::new(-AXIS_T * SCALE_T, START, 0.0),
        Vec3::new(1.5 * MAX_T * SCALE_T, START, 0.0),
        css::RED,
    );
    gizmos.line(
        Vec3::new(-AXIS_T * SCALE_T, END, 0.0),
        Vec3::new(1.5 * MAX_T * SCALE_T, END, 0.0),
        css::RED,
    );
    gizmos.line(
        Vec3::new(-AXIS_T * SCALE_T, END + (END - START), 0.0),
        Vec3::new(1.5 * MAX_T * SCALE_T, END + (END - START), 0.0),
        css::RED,
    );

    const PRECISION: f32 = 0.99;
    const LIMIT: f32 = (END - START) * (1.0 - PRECISION);
    gizmos.line(
        Vec3::new(-AXIS_T * SCALE_T, END - LIMIT, 0.0),
        Vec3::new(1.5 * MAX_T * SCALE_T, END - LIMIT, 0.0),
        css::DARK_RED,
    );
    gizmos.line(
        Vec3::new(MAX_T * SCALE_T, -0.25, 0.0),
        Vec3::new(MAX_T * SCALE_T, MAX_Y, 0.0),
        css::DARK_RED,
    );
}

fn show_exp_smooth_curve(mut gizmos: Gizmos) {
    {
        let mut param = START.animated().smooth(MAX_T);
        let mut pos = Vec::new();

        param.set_target(END);
        let y = param.animate(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T * 1.1 {
            let y = param.animate(DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::GREEN);
    }

    {
        let mut param = START.animated().predict(MAX_T);
        let mut pos = Vec::new();

        param.set_target(END);
        let y = param.animate(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T * 1.1 {
            let y = param.animate(DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::BLUE);
    }

    {
        let mut param = START.animated().smooth(MAX_T);
        let mut pos = Vec::new();

        param.set_target(END);
        let y = param.animate(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut updated = 0;
        let mut t = 0.0;
        while t < MAX_T * 1.7 {
            let y = param.animate(DELTA_T);
            t += DELTA_T;

            if updated == 0 && t > MAX_T * 0.3 {
                param.set_target(END + (END - START));
                updated = 1;
            }

            if updated == 1 && t > MAX_T * 0.6 {
                param.set_target(END - (END - START));
                updated = 2;
            }

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::YELLOW);
    }

    {
        let mut param = START.animated().predict(MAX_T);
        let mut pos = Vec::new();

        param.set_target(END);
        let y = param.animate(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut updated = 0;
        let mut t = 0.0;
        while t < MAX_T * 1.7 {
            let y = param.animate(DELTA_T);
            t += DELTA_T;

            if updated == 0 && t > MAX_T * 0.3 {
                param.set_target(END + (END - START));
                updated = 1;
            }

            if updated == 1 && t > MAX_T * 0.6 {
                param.set_target(END - (END - START));
                updated = 2;
            }

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::LIGHT_CYAN);
    }

    {
        let mut param = AnimatedValue::time();
        let mut pos = Vec::new();

        let y = param.animate(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T * 1.7 {
            let y = param.animate(DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::ORANGE);
    }
}
