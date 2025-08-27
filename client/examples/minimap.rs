use bevy::{color::palettes::css, prelude::*, render::view::NoIndirectDrawing};
use shine_game::{
    app::init_application,
    camera_rig::{rigs, CameraRig, CameraRigPlugin},
    input_manager::{ActionState, InputManagerPlugin, InputMap, KeyboardInput, VirtualDPad, VirtualPad},
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
    app.add_plugins(CameraRigPlugin::default());
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
            handle_tab_buttons,
            update_content_visibility,
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
    active_tab: TabAction,
}

impl Default for TabAction {
    fn default() -> Self {
        TabAction::Map
    }
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
pub struct MinimapRoot;

#[derive(Component)]
pub struct UiRoot;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabAction {
    Settings,
    Inventory,
    Map,
    Exit,
}

#[derive(Component)]
pub struct TabButton {
    pub action: TabAction,
}

#[derive(Component)]
pub struct ContentPlaceholder {
    pub tab: TabAction,
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
    let camera = {
        let mut rig = CameraRig::new()
            .with(rigs::Position::new(start_position))
            .with(rigs::Rotation::new(Quat::default()))
            .with(rigs::Predict::position(1.25))
            .with(rigs::Arm::new(Vec3::new(0.0, 3.5, -5.5)))
            .with(rigs::Predict::position(2.5))
            .with(
                rigs::LookAt::new(start_position + Vec3::Y)
                    .smoothness(1.25)
                    .predictive(true),
            );

        (
            Camera3d::default(),
            Camera { order: 0, ..Default::default() },
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            rig.calculate_transform(0.0, None),
            rig,
            MainCamera,
        )
    };
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
    // Create minimap root node
    let minimap_root = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            MinimapRoot,
            MinimapEntity,
            Name::new("MinimapRoot"),
        ))
        .id();

    // Minimap gradient background - render behind everything else
    // Use a large size that will cover any reasonable viewport
    let gradient_material = create_gradient_material(col_materials);
    let minimap_background = commands
        .spawn((
            Mesh2d(meshes.add(Rectangle::new(1000.0, 1000.0))), // Large enough to cover any viewport
            MeshMaterial2d(gradient_material),
            Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)), // Behind other elements
            MinimapEntity,
            MinimapBackground,
            Name::new("MinimapBackground"),
        ))
        .id();

    // Minimap player - different shape (circle) - made much bigger
    let minimap_player = commands
        .spawn((
            Mesh2d(meshes.add(Circle::new(2.0))), // Increased from 1.2 to 2.0
            MeshMaterial2d(col_materials.add(Color::Srgba(css::BLUE))),
            Transform::from_translation(Vec3::new(start_position.x, start_position.z, 0.0)),
            MinimapEntity,
            MinimapPlayer,
            Name::new("MinimapPlayer"),
        ))
        .id();

    // Minimap floor - different representation (large square) - keep the same
    let minimap_floor = commands
        .spawn((
            Mesh2d(meshes.add(Rectangle::new(15.0, 15.0))),
            MeshMaterial2d(col_materials.add(Color::Srgba(css::GREEN).with_alpha(0.3))),
            Transform::from_translation(Vec3::ZERO),
            MinimapEntity,
            MinimapFloor,
            Name::new("MinimapFloor"),
        ))
        .id();

    // Minimap objects (small squares for the cubes) - made much bigger
    let cube_positions = [
        Vec3::new(3.0, 0.0, 3.0),
        Vec3::new(-3.0, 0.0, 3.0),
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(-3.0, 0.0, -3.0),
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 0.0),
    ];

    let mut minimap_objects = Vec::new();
    for (i, position) in cube_positions.iter().enumerate() {
        let object = commands
            .spawn((
                Mesh2d(meshes.add(Rectangle::new(2.0, 2.0))), // Increased from 1.2 to 2.0
                MeshMaterial2d(col_materials.add(Color::Srgba(css::ORANGE_RED))),
                Transform::from_translation(Vec3::new(position.x, position.z, 0.0)),
                MinimapEntity,
                Name::new(format!("MinimapObject_{}", i)),
            ))
            .id();
        minimap_objects.push(object);
    }

    // Parent all minimap entities to the root
    commands
        .entity(minimap_root)
        .add_children(&[minimap_background, minimap_player, minimap_floor]);
    commands.entity(minimap_root).add_children(&minimap_objects);
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
    existing_menu: Query<Entity, With<UiRoot>>,
    windows: Query<&Window>,
    mut last_fullscreen_state: Local<Option<bool>>,
) -> Result<(), BevyError> {
    let window = windows.single()?;

    // Only recreate UI when fullscreen state changes, not when active tab changes
    let current_fullscreen = minimap_state.is_fullscreen;
    let should_update = last_fullscreen_state.is_none() || *last_fullscreen_state != Some(current_fullscreen);

    if should_update {
        *last_fullscreen_state = Some(current_fullscreen);

        // Remove existing UI root if any
        for entity in existing_menu.iter() {
            commands.entity(entity).despawn();
        }

        // Only spawn menu when fullscreen
        if minimap_state.is_fullscreen {
            spawn_fullscreen_ui(&mut commands, window)?;
        }
    }

    Ok(())
}

fn spawn_fullscreen_ui(commands: &mut Commands, window: &Window) -> Result<(), BevyError> {
    info!(
        "Spawning fullscreen UI for window: {}x{}",
        window.width(),
        window.height()
    );

    // Create UI root node
    let ui_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            UiRoot,
            MinimapMenu,
            Name::new("UiRoot"),
        ))
        .id();

    // Create tab bar at the top
    let tab_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
            Name::new("TabBar"),
        ))
        .id();

    // Create tab buttons
    let tabs = [
        ("Settings", TabAction::Settings),
        ("Inventory", TabAction::Inventory),
        ("Map", TabAction::Map),
        ("Exit", TabAction::Exit),
    ];

    let mut tab_buttons = Vec::new();
    for (text, action) in tabs.iter() {
        let is_active = *action == TabAction::Map; // Map is default active
        let tab_button = commands
            .spawn((
                Button,
                Node {
                    width: Val::Px(120.0),
                    height: Val::Px(40.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::horizontal(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(if is_active {
                    Color::srgba(0.4, 0.4, 0.5, 1.0)
                } else {
                    Color::srgba(0.2, 0.2, 0.3, 0.9)
                }),
                BorderColor(Color::srgba(0.5, 0.5, 0.6, 1.0)),
                BorderRadius::all(Val::Px(8.0)),
                TabButton { action: *action },
                Name::new(format!("TabButton_{:?}", action)),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new(*text),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::WHITE),
                    Name::new(format!("TabText_{:?}", action)),
                ));
            })
            .id();
        tab_buttons.push(tab_button);
    }

    // Create content area
    let content_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("ContentArea"),
        ))
        .id();

    // Create content placeholders for non-map tabs
    let mut content_placeholders = Vec::new();

    // Settings placeholder
    let settings_content = commands.spawn((
        Node {
            width: Val::Percent(80.0),
            height: Val::Percent(80.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.3, 0.2, 0.9)),
        BorderRadius::all(Val::Px(10.0)),
        Visibility::Hidden, // Hidden by default
        ContentPlaceholder { tab: TabAction::Settings },
        Name::new("SettingsContent"),
    )).with_children(|parent| {
        parent.spawn((
            Text::new("SETTINGS PANEL"),
            TextFont { font_size: 32.0, ..default() },
            TextColor(Color::WHITE),
            Node { margin: UiRect::bottom(Val::Px(20.0)), ..default() },
        ));
        parent.spawn((
            Text::new("This is a placeholder for the Settings UI.\nHere you would configure game options,\nvideo settings, audio settings, etc."),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
        ));
    }).id();

    // Inventory placeholder
    let inventory_content = commands.spawn((
        Node {
            width: Val::Percent(80.0),
            height: Val::Percent(80.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.3, 0.2, 0.2, 0.9)),
        BorderRadius::all(Val::Px(10.0)),
        Visibility::Hidden, // Hidden by default
        ContentPlaceholder { tab: TabAction::Inventory },
        Name::new("InventoryContent"),
    )).with_children(|parent| {
        parent.spawn((
            Text::new("INVENTORY PANEL"),
            TextFont { font_size: 32.0, ..default() },
            TextColor(Color::WHITE),
            Node { margin: UiRect::bottom(Val::Px(20.0)), ..default() },
        ));
        parent.spawn((
            Text::new("This is a placeholder for the Inventory UI.\nHere you would see your items,\nequipment, resources, etc."),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
        ));
    }).id();

    // Exit placeholder
    let exit_content = commands
        .spawn((
            Node {
                width: Val::Percent(80.0),
                height: Val::Percent(80.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.3, 0.2, 0.3, 0.9)),
            BorderRadius::all(Val::Px(10.0)),
            Visibility::Hidden, // Hidden by default
            ContentPlaceholder { tab: TabAction::Exit },
            Name::new("ExitContent"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("EXIT CONFIRMATION"),
                TextFont { font_size: 32.0, ..default() },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ));
            parent.spawn((
                Text::new("Are you sure you want to exit the game?\nThis is a placeholder for exit confirmation."),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
            ));
        })
        .id();

    content_placeholders.extend([settings_content, inventory_content, exit_content]);

    // Parent everything to the UI root
    commands.entity(ui_root).add_child(tab_bar);
    commands.entity(ui_root).add_child(content_area);
    commands.entity(tab_bar).add_children(&tab_buttons);
    commands.entity(content_area).add_children(&content_placeholders);

    Ok(())
}

fn handle_tab_buttons(
    mut interaction_query: Query<
        (&Interaction, &TabButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut minimap_state: ResMut<MinimapState>,
    mut tab_buttons: Query<(&TabButton, &mut BackgroundColor), Without<Interaction>>,
) {
    for (interaction, tab_button, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                info!("{:?} tab selected", tab_button.action);

                // Update active tab
                minimap_state.active_tab = tab_button.action;

                // Update all tab button colors
                for (tab, mut bg_color) in tab_buttons.iter_mut() {
                    if tab.action == minimap_state.active_tab {
                        *bg_color = BackgroundColor(Color::srgba(0.4, 0.4, 0.5, 1.0));
                    // Active
                    } else {
                        *bg_color = BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.9));
                        // Inactive
                    }
                }

                // Set pressed color
                *color = BackgroundColor(Color::srgba(0.5, 0.5, 0.6, 1.0));
            }
            Interaction::Hovered => {
                if minimap_state.active_tab != tab_button.action {
                    *color = BackgroundColor(Color::srgba(0.3, 0.3, 0.4, 0.9));
                }
            }
            Interaction::None => {
                if minimap_state.active_tab == tab_button.action {
                    *color = BackgroundColor(Color::srgba(0.4, 0.4, 0.5, 1.0)); // Active
                } else {
                    *color = BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.9)); // Inactive
                }
            }
        }
    }
}

fn update_content_visibility(
    minimap_state: Res<MinimapState>,
    mut minimap_root_query: Query<&mut Visibility, (With<MinimapRoot>, Without<ContentPlaceholder>)>,
    mut content_query: Query<(&ContentPlaceholder, &mut Visibility), Without<MinimapRoot>>,
) -> Result<(), BevyError> {
    if minimap_state.is_changed() {
        // Handle minimap root visibility
        if let Ok(mut minimap_visibility) = minimap_root_query.single_mut() {
            // Minimap is always visible when minimized, only hidden in fullscreen when non-map tab is active
            if !minimap_state.is_fullscreen || minimap_state.active_tab == TabAction::Map {
                *minimap_visibility = Visibility::Visible;
            } else {
                *minimap_visibility = Visibility::Hidden;
            }
        }

        // Handle content placeholder visibility (only when fullscreen)
        if minimap_state.is_fullscreen {
            for (placeholder, mut visibility) in content_query.iter_mut() {
                if placeholder.tab == minimap_state.active_tab && minimap_state.active_tab != TabAction::Map {
                    *visibility = Visibility::Visible;
                } else {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    Ok(())
}
