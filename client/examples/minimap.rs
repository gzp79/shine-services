use bevy::prelude::*;
use bevy::{color::palettes::css, render::view::NoIndirectDrawing};
use shine_game::input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, VirtualDPad, VirtualPad};
use shine_game::{
    application,
    camera_rig::{rigs, CameraRig},
};

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::application::{create_application, platform::Config};

    application::init(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}

fn setup_game(app: &mut App) {
    app.add_plugins(InputManagerPlugin::<Action>::default());
    app.insert_resource(MinimapState::default());
    app.add_systems(Startup, spawn_world);
    app.add_systems(
        Update,
        (
            handle_input,
            handle_minimap_toggle,
            follow_player,
            sync_minimap_entities,
            update_minimap_camera,
            manage_minimap_menu,
            handle_menu_buttons,
        )
            .chain(),
    );
    app.add_systems(Update, update_minimap_on_resize);
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Action {
    Movement,
    Rotate,
    SwitchMinimap,
}

#[derive(Resource, Default)]
struct MinimapState {
    is_fullscreen: bool,
}

// Component markers for entities that exist in both 3D world and 2D minimap
#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Floor;

// Component markers for minimap-specific entities
#[derive(Component)]
pub struct MinimapEntity;

#[derive(Component)]
pub struct MinimapPlayer;

#[derive(Component)]
pub struct MinimapFloor;

#[derive(Component)]
pub struct MinimapBackground;

#[derive(Component)]
pub struct MinimapCamera;

#[derive(Component)]
pub struct MainCamera;

// UI Menu components
#[derive(Component)]
pub struct MinimapMenu;

#[derive(Component)]
pub struct MenuButton {
    pub action: MenuAction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Settings,
    Inventory,
    Map,
    Exit,
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut col_materials: ResMut<Assets<ColorMaterial>>,
) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "Minimap POC".to_string();

    let start_position = Vec3::new(0.0, 0.0, 0.0);

    let input_map = InputMap::new()
        .with_binding(Action::Movement, VirtualDPad::wasd())?
        .with_binding(Action::Rotate, VirtualPad::from_keys(KeyCode::KeyQ, KeyCode::KeyE))?
        .with_binding(Action::SwitchMinimap, KeyboardInput::new(KeyCode::KeyM))?;

    // Spawn 3D player with distinct tetrahedron mesh
    let player = (
        Mesh3d(meshes.add(Tetrahedron::new(
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.5, -1.0),
        ))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_translation(start_position),
        Player,
        input_map,
    );
    commands.spawn(player);

    // Spawn 3D floor with distinct mesh from minimap
    let floor = (
        Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().subdivisions(10).size(15.0, 15.0)))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_GREEN))),
        Floor,
    );
    commands.spawn(floor);

    // Spawn additional 3D objects for more interesting minimap
    spawn_world_objects(&mut commands, &mut meshes, &mut materials);

    // Spawn minimap entities with different meshes
    spawn_minimap_entities(&mut commands, &mut meshes, &mut col_materials, start_position);

    let light = (
        PointLight {
            shadows_enabled: true,
            range: 100.0,
            intensity: 2000.0 * 1000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0),
    );
    commands.spawn(light);

    // Spawn main 3D camera
    let rig = CameraRig::builder()
        .with(rigs::Position::new(start_position))
        .with(rigs::Rotation::new(Quat::default()))
        .with(rigs::Smooth::new_position(1.25).predictive(true))
        .with(rigs::Arm::new(Vec3::new(0.0, 3.5, -5.5)))
        .with(rigs::Smooth::new_position(2.5).predictive(true))
        .with(
            rigs::LookAt::new(start_position + Vec3::Y)
                .smoothness(1.25)
                .predictive(true),
        )
        .build();

    let camera = (
        Camera3d::default(),
        Camera { order: 0, ..Default::default() },
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        *rig.transform(),
        rig,
        MainCamera,
    );
    commands.spawn(camera);

    // Spawn minimap camera (2D, top-down view)
    spawn_minimap_camera(&mut commands, &window);

    Ok(())
}

fn spawn_world_objects(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    // Add some cubes scattered around
    let cube_positions = [
        Vec3::new(3.0, 0.5, 3.0),
        Vec3::new(-3.0, 0.5, 3.0),
        Vec3::new(3.0, 0.5, -3.0),
        Vec3::new(-3.0, 0.5, -3.0),
        Vec3::new(0.0, 0.5, 5.0),
        Vec3::new(5.0, 0.5, 0.0),
    ];

    for position in cube_positions.iter() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(Color::Srgba(css::ORANGE))),
            Transform::from_translation(*position),
        ));
    }
}

fn spawn_minimap_entities(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    col_materials: &mut ResMut<Assets<ColorMaterial>>,
    start_position: Vec3,
) {
    // Minimap gradient background - render behind everything else
    // Use a large size that will cover any reasonable viewport
    let gradient_material = create_gradient_material(col_materials);
    let minimap_background = (
        Mesh2d(meshes.add(Rectangle::new(1000.0, 1000.0))), // Large enough to cover any viewport
        MeshMaterial2d(gradient_material),
        Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)), // Behind other elements
        MinimapEntity,
        MinimapBackground,
    );
    commands.spawn(minimap_background);

    // Minimap player - different shape (circle) - made much bigger
    let minimap_player = (
        Mesh2d(meshes.add(Circle::new(2.0))), // Increased from 1.2 to 2.0
        MeshMaterial2d(col_materials.add(Color::Srgba(css::BLUE))),
        Transform::from_translation(Vec3::new(start_position.x, start_position.z, 0.0)),
        MinimapEntity,
        MinimapPlayer,
    );
    commands.spawn(minimap_player);

    // Minimap floor - different representation (large square) - keep the same
    let minimap_floor = (
        Mesh2d(meshes.add(Rectangle::new(15.0, 15.0))),
        MeshMaterial2d(col_materials.add(Color::Srgba(css::GREEN).with_alpha(0.3))),
        Transform::from_translation(Vec3::ZERO),
        MinimapEntity,
        MinimapFloor,
    );
    commands.spawn(minimap_floor);

    // Minimap objects (small squares for the cubes) - made much bigger
    let cube_positions = [
        Vec3::new(3.0, 0.0, 3.0),
        Vec3::new(-3.0, 0.0, 3.0),
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(-3.0, 0.0, -3.0),
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 0.0),
    ];

    for position in cube_positions.iter() {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(2.0, 2.0))), // Increased from 1.2 to 2.0
            MeshMaterial2d(col_materials.add(Color::Srgba(css::ORANGE_RED))),
            Transform::from_translation(Vec3::new(position.x, position.z, 0.0)),
            MinimapEntity,
        ));
    }
}

fn create_gradient_material(col_materials: &mut ResMut<Assets<ColorMaterial>>) -> Handle<ColorMaterial> {
    // Create a subtle radial gradient from dark blue at the edges to lighter blue in the center
    // This creates a nice vignette effect for the minimap
    let gradient_color = Color::linear_rgba(0.1, 0.2, 0.4, 0.8); // Dark blue with transparency
    col_materials.add(ColorMaterial::from(gradient_color))
}

fn spawn_minimap_camera(commands: &mut Commands, window: &Window) {
    let minimap_viewport = calculate_minimap_viewport(window, false);

    let minimap_camera = (
        Camera2d::default(),
        Camera {
            order: 1,
            viewport: Some(minimap_viewport),
            ..Default::default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::render::camera::ScalingMode::FixedVertical { viewport_height: 30.0 },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        MinimapCamera,
    );
    commands.spawn(minimap_camera);
}

fn calculate_minimap_viewport(window: &Window, is_fullscreen: bool) -> bevy::render::camera::Viewport {
    let window_width = window.physical_width() as f32;
    let window_height = window.physical_height() as f32;

    if is_fullscreen {
        // 80% of screen, centered
        let size_x = window_width * 0.8;
        let size_y = window_height * 0.8;
        let pos_x = (window_width - size_x) * 0.5;
        let pos_y = (window_height - size_y) * 0.5;

        bevy::render::camera::Viewport {
            physical_position: UVec2::new(pos_x as u32, pos_y as u32),
            physical_size: UVec2::new(size_x as u32, size_y as u32),
            ..Default::default()
        }
    } else {
        // Small minimap in top-right corner
        let minimap_size = 200.0_f32.min(window_width * 0.25).min(window_height * 0.25);
        let margin = 10.0;

        bevy::render::camera::Viewport {
            physical_position: UVec2::new((window_width - minimap_size - margin) as u32, margin as u32),
            physical_size: UVec2::new(minimap_size as u32, minimap_size as u32),
            ..Default::default()
        }
    }
}

fn handle_input(
    mut player_q: Query<(&ActionState<Action>, &mut Transform), With<Player>>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let (action, mut transform) = player_q.single_mut()?;

    let movement = action.dual_axis_value(&Action::Movement);
    let mut rot = action.axis_value(&Action::Rotate);

    rot *= time.delta_secs() * 2.0;
    transform.rotation = Quat::from_rotation_y(rot) * transform.rotation;

    let mut move_vec = transform.rotation * Vec3::new(movement.x, 0.0, movement.y);
    move_vec.y = 0.0;
    if move_vec.length_squared() > 0.0 {
        move_vec = move_vec.normalize();
    }
    move_vec *= time.delta_secs() * 5.0;

    transform.translation += move_vec;
    Ok(())
}

fn handle_minimap_toggle(
    player_q: Query<&ActionState<Action>, With<Player>>,
    mut minimap_state: ResMut<MinimapState>,
) -> Result<(), BevyError> {
    let action = player_q.single()?;

    if action.just_pressed(&Action::SwitchMinimap) {
        minimap_state.is_fullscreen = !minimap_state.is_fullscreen;
    }

    Ok(())
}

fn follow_player(
    player_q: Query<&Transform, With<Player>>,
    mut camera_q: Query<(&mut CameraRig, &mut Transform), (With<MainCamera>, Without<Player>)>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let player_transform = player_q.single()?;
    let (mut camera_rig, mut camera_transform) = camera_q.single_mut()?;

    camera_rig.driver_mut::<rigs::Position>().position = player_transform.translation;
    camera_rig.driver_mut::<rigs::Rotation>().rotation = player_transform.rotation;
    camera_rig.driver_mut::<rigs::LookAt>().target = player_transform.translation + Vec3::Y;

    *camera_transform = camera_rig.update(time.delta_secs());
    Ok(())
}

fn sync_minimap_entities(
    player_q: Query<&Transform, With<Player>>,
    mut minimap_player_q: Query<&mut Transform, (With<MinimapPlayer>, Without<Player>)>,
) -> Result<(), BevyError> {
    let player_transform = player_q.single()?;
    let mut minimap_player_transform = minimap_player_q.single_mut()?;

    // Convert 3D world coordinates (X, Z) to 2D minimap coordinates (X, Y)
    // Scale down the position for the minimap
    minimap_player_transform.translation =
        Vec3::new(player_transform.translation.x, player_transform.translation.z, 0.0);

    Ok(())
}

fn update_minimap_camera(
    mut minimap_camera_q: Query<(&mut Camera, &mut Projection), With<MinimapCamera>>,
    windows: Query<&Window>,
    minimap_state: Res<MinimapState>,
) -> Result<(), BevyError> {
    let window = windows.single()?;
    let (mut camera, _projection) = minimap_camera_q.single_mut()?;

    // Only update if the state has changed
    if minimap_state.is_changed() {
        camera.viewport = Some(calculate_minimap_viewport(window, minimap_state.is_fullscreen));
    }

    Ok(())
}

fn update_minimap_on_resize(
    mut minimap_camera_q: Query<(&mut Camera, &mut Projection), With<MinimapCamera>>,
    windows: Query<&Window>,
    minimap_state: Res<MinimapState>,
    mut last_window_size: Local<Option<(u32, u32)>>,
) -> Result<(), BevyError> {
    let window = windows.single()?;
    let current_size = (window.physical_width(), window.physical_height());

    // Check if window size has changed
    if last_window_size.as_ref() != Some(&current_size) {
        *last_window_size = Some(current_size);

        let (mut camera, _projection) = minimap_camera_q.single_mut()?;
        camera.viewport = Some(calculate_minimap_viewport(window, minimap_state.is_fullscreen));
    }

    Ok(())
}

fn manage_minimap_menu(
    mut commands: Commands,
    minimap_state: Res<MinimapState>,
    existing_menu: Query<Entity, With<MinimapMenu>>,
    windows: Query<&Window>,
) -> Result<(), BevyError> {
    let window = windows.single()?;

    if minimap_state.is_changed() {
        // Remove existing menu if any
        for entity in existing_menu.iter() {
            commands.entity(entity).despawn();
        }

        // Only spawn menu when fullscreen
        if minimap_state.is_fullscreen {
            spawn_minimap_menu(&mut commands, window)?;
        }
    }

    Ok(())
}

fn spawn_minimap_menu(commands: &mut Commands, window: &Window) -> Result<(), BevyError> {
    info!("Spawning menu for window: {}x{}", window.width(), window.height());

    // Create a side menu panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),  // Position from right edge
                top: Val::Px(20.0),    // Position from top
                width: Val::Px(200.0), // Fixed width for side menu
                height: Val::Auto,     // Auto height based on content
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.9)), // Dark panel background
            ZIndex(1000),                                      // High z-index to ensure it's on top
            MinimapMenu,
        ))
        .with_children(|parent| {
            // Menu panel
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(15.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.95)),
                    BorderColor(Color::srgba(0.5, 0.5, 0.6, 1.0)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|menu_panel| {
                    // Title
                    menu_panel.spawn((
                        Text::new("MINIMAP MENU"),
                        TextFont { font_size: 24.0, ..default() },
                        TextColor(Color::WHITE),
                        Node {
                            margin: UiRect::bottom(Val::Px(10.0)),
                            ..default()
                        },
                    ));

                    // Menu buttons
                    let buttons = [
                        ("Settings", MenuAction::Settings),
                        ("Inventory", MenuAction::Inventory),
                        ("Map", MenuAction::Map),
                        ("Exit", MenuAction::Exit),
                    ];

                    for (text, action) in buttons.iter() {
                        menu_panel
                            .spawn((
                                Button,
                                Node {
                                    width: Val::Px(200.0),
                                    height: Val::Px(50.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    margin: UiRect::all(Val::Px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.3, 0.3, 0.4, 0.9)),
                                BorderColor(Color::srgba(0.5, 0.5, 0.6, 1.0)),
                                BorderRadius::all(Val::Px(8.0)),
                                MenuButton { action: *action },
                            ))
                            .with_children(|button| {
                                button.spawn((
                                    Text::new(*text),
                                    TextFont { font_size: 18.0, ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });
                    }
                });
        });

    Ok(())
}

fn handle_menu_buttons(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, menu_button, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgba(0.1, 0.1, 0.2, 0.9));
                // Handle button actions
                match menu_button.action {
                    MenuAction::Settings => info!("Settings button pressed"),
                    MenuAction::Inventory => info!("Inventory button pressed"),
                    MenuAction::Map => info!("Map button pressed"),
                    MenuAction::Exit => info!("Exit button pressed"),
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.3, 0.3, 0.4, 0.9));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.8));
            }
        }
    }
}
