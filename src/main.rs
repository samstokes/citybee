use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_move_things)
        .add_systems(Update, position_objects_on_grid)
        .add_systems(Update, move_light)
        .add_systems(Update, move_cursor)
        .run();
}

const CITY: [u8; 25] = [
    0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, //
    0, 0, 3, 1, 0, //
    0, 1, 0, 0, 0, //
    0, 2, 0, 0, 0, //
];

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let building_coords = parse_city(CITY);

    // camera
    commands.spawn(Camera3dBundle {
        projection: OrthographicProjection {
            scale: 3.0,
            scaling_mode: bevy::render::camera::ScalingMode::FixedVertical(2.0),
            ..default()
        }
        .into(),
        transform: Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // plane
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(shape::Plane::from_size(6.0).into()),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
            ..default()
        })
        .insert(Ground);

    for (coords, height) in building_coords {
        commands
            .spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Box {
                    min_x: -0.5,
                    max_x: 0.5,
                    min_y: -0.5,
                    max_y: -0.5 + (height as f32),
                    min_z: -0.5,
                    max_z: 0.5,
                })),
                material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
                ..default()
            })
            .insert(coords);
    }

    // light
    commands.spawn(PointLightBundle {
        transform: Transform::from_xyz(0.0, 8.0, 0.0),
        ..default()
    });

    // cursor
    commands
        .spawn(PbrBundle {
            // the cursor is a lump of coal until I can be bothered
            mesh: meshes.add(Mesh::from(shape::UVSphere {
                radius: 0.1,
                sectors: 5,
                stacks: 5,
            })),
            material: materials.add(Color::rgb(0.1, 0.1, 0.1).into()),
            transform: Transform::from_xyz(3.0, 0.0, 1.0),
            ..default()
        })
        .insert(Cursor);
}

fn position_objects_on_grid(mut q: Query<(&mut Transform, &GridCoords)>) {
    for (mut tx, coords) in &mut q {
        tx.translation = coords.to_world();
    }
}

fn keyboard_move_things(keys: Res<Input<KeyCode>>, mut q: Query<&mut GridCoords>) {
    // the y direction goes the opposite way my brain thinks it should, so
    // W and S are inverted.
    let (dx, dy) = if keys.just_pressed(KeyCode::W) {
        (0, -1)
    } else if keys.just_pressed(KeyCode::A) {
        (-1, 0)
    } else if keys.just_pressed(KeyCode::S) {
        (0, 1)
    } else if keys.just_pressed(KeyCode::D) {
        (1, 0)
    } else {
        (0, 0)
    };

    if dx != 0 || dy != 0 {
        for mut coords in &mut q {
            coords.x += dx;
            coords.y += dy;
        }
    }
}

fn move_light(time: Res<Time>, mut light_tx: Query<&mut Transform, With<PointLight>>) {
    let mut light_tx = light_tx.get_single_mut().unwrap();
    let mut light_pos = &mut light_tx.translation;
    light_pos.x = 3.0 * time.elapsed_seconds().sin();
    light_pos.z = 5.0 * time.elapsed_seconds().cos();
}

type Height = u8;

#[derive(Component, Clone, Copy)]
struct GridCoords {
    x: i8,
    y: i8,
}

impl GridCoords {
    #[allow(dead_code)]
    const ORIGIN: GridCoords = GridCoords { x: 0, y: 0 };

    fn new(x: i8, y: i8) -> Self {
        Self { x, y }
    }

    fn to_world(&self) -> Vec3 {
        // grid xy is world xz (world y is height)
        Vec3::new(self.x as f32, 0.5, self.y as f32)
    }
}

#[derive(Component)]
struct Cursor;

#[derive(Component)]
struct Ground;

fn parse_city<const N: usize>(city: [u8; N]) -> Vec<(GridCoords, Height)> {
    let size_f = (city.len() as f32).sqrt();
    let floor = size_f.floor();
    assert_eq!(size_f, floor);
    let size = floor as usize;
    assert_eq!(size, 5);
    let half_size = (size / 2) as i8;

    city.into_iter()
        .enumerate()
        .flat_map(|(i, h)| {
            if h > 0 {
                let x = i % size;
                let y = i / size;
                Some((
                    GridCoords::new(x as i8 - half_size, (y as i8) - half_size),
                    h,
                ))
            } else {
                None
            }
        })
        .collect()
}

fn move_cursor(
    mut cursor_query: Query<&mut Transform, With<Cursor>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    ground_query: Query<&GlobalTransform, With<Ground>>,
    windows: Query<&Window>,
) {
    let mut cursor_tx = cursor_query.single_mut();
    let (camera, camera_gtx) = camera_query.single();
    let ground_gtx = ground_query.single();

    let Some(cursor_pos) = windows.single().cursor_position() else {
        return;
    };

    let Some(ray) = camera.viewport_to_world(camera_gtx, cursor_pos) else {
        return;
    };

    let Some(distance) = ray.intersect_plane(ground_gtx.translation(), ground_gtx.up()) else {
        return;
    };
    let point = ray.get_point(distance);

    cursor_tx.translation = point;
}
