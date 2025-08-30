use bevy::{color::palettes::css, prelude::*, render::view::NoIndirectDrawing};
use shine_game::{
    app::init_application,
    math::value::{TemporalValue, TemporalValueExt},
};

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

fn setup_game(app: &mut App) {
    app.add_systems(Startup, spawn_world);
    app.add_systems(Update, (show_axis, show_exp_smooth_curve));
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
        let mut param = START.smoothed(MAX_T);
        let mut pos = Vec::new();

        param.set(END);
        let y = param.update(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T * 1.1 {
            let y = param.update(DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::GREEN);
    }

    {
        let mut param = START.predicted(MAX_T);
        let mut pos = Vec::new();

        param.set(END);
        let y = param.update(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T * 1.1 {
            let y = param.update(DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::BLUE);
    }

    {
        let mut param = START.smoothed(MAX_T);
        let mut pos = Vec::new();

        param.set(END);
        let y = param.update(0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut updated = 0;
        let mut t = 0.0;
        while t < MAX_T * 1.7 {
            let y = param.update(DELTA_T);
            t += DELTA_T;

            if updated == 0 && t > MAX_T * 0.3 {
                param.set(END + (END - START));
                updated = 1;
            }

            if updated == 1 && t > MAX_T * 0.6 {
                param.set(END - (END - START));
                updated = 2;
            }

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::YELLOW);
    }

    /*{
        let mut pos = Vec::new();
        let mut smooth = ExpSmoothed::with_start(START).duration(MAX_T);

        let y = smooth.predict_from(&0.0, 0.0);
        pos.push(Vec3::new(0.0, y, 0.0));

        let mut t = 0.0;
        while t < MAX_T {
            let y = smooth.predict_from(&END, DELTA_T);
            t += DELTA_T;

            pos.push(Vec3::new(t * SCALE_T, y, 0.0));
        }

        gizmos.linestrip(pos, css::BLUE);
    }*/
}
