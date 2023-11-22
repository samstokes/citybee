use std::f32::consts::PI;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_move_things)
        .add_systems(Update, position_objects_on_grid)
        .add_systems(Update, move_light)
        .add_systems(Update, move_cursor)
        .add_systems(Update, add_buildings)
        .run();
}

const STARTING_CITY: [Height; 25] = [
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
    mut window_query: Query<&mut Window>,
) {
    let building_coords = parse_city(STARTING_CITY);

    let mut window = window_query.single_mut();
    window.cursor.visible = false;

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
            .spawn(BuildingBundle::add(
                &mut meshes,
                &mut materials,
                Building { height },
            ))
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

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
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

    fn from_world(world: Vec3) -> Self {
        GridCoords::new(world.x.round() as i8, world.z.round() as i8)
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

#[derive(Component)]
struct Building {
    height: Height,
}

#[derive(Bundle)]
struct BuildingBundle {
    building: Building,
    pbr: PbrBundle,
}

impl BuildingBundle {
    fn add(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        building: Building,
    ) -> Self {
        let pbr = PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Box {
                min_x: -0.5,
                max_x: 0.5,
                min_y: -0.5,
                max_y: -0.5 + (building.height as f32),
                min_z: -0.5,
                max_z: 0.5,
            })),
            material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
            ..default()
        };
        Self { building, pbr }
    }
}

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
    window_query: Query<&Window>,
    building_query: Query<(&GridCoords, &Building)>,
    mut gizmos: Gizmos,
) {
    let mut cursor_tx = cursor_query.single_mut();
    let (camera, camera_gtx) = camera_query.single();
    let ground_gtx = ground_query.single();
    let window = window_query.single();

    let Some((grid, point)) = cursor_to_grid(window, camera, camera_gtx, ground_gtx) else {
        return;
    };
    // TODO store grid coords on cursor

    cursor_tx.translation = point;

    let grid_cell_center = grid.to_world();

    // TODO make this not a linear scan each time
    let building = building_query
        .iter()
        .find(|(&coords, _)| grid == coords)
        .map(|(_, building)| building);
    let height = -0.5 + building.map_or(0, |b| b.height) as f32;
    let selection_center = grid_cell_center + height * Vec3::Y;

    let rotation = Quat::from_rotation_x(PI * 0.5);
    gizmos.rect(selection_center, rotation, Vec2::ONE, Color::ANTIQUE_WHITE);
}

fn cursor_to_grid(
    window: &Window,
    camera: &Camera,
    camera_gtx: &GlobalTransform,
    ground_gtx: &GlobalTransform,
) -> Option<(GridCoords, Vec3)> {
    let cursor_pos = window.cursor_position()?;

    let ray = camera.viewport_to_world(camera_gtx, cursor_pos)?;

    let distance = ray.intersect_plane(ground_gtx.translation(), ground_gtx.up())?;
    let point = ray.get_point(distance);
    let grid = GridCoords::from_world(point);
    Some((grid, point))
}

fn add_buildings(
    // TODO clean this up once cursor carries its grid coords
    buttons: Res<Input<MouseButton>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    ground_query: Query<&GlobalTransform, With<Ground>>,
    window_query: Query<&Window>,
    // TODO clean these up once building adding is refactored
    mut building_query: Query<(&GridCoords, &mut Handle<Mesh>, &mut Building)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let (camera, camera_gtx) = camera_query.single();
    let ground_gtx = ground_query.single();
    let window = window_query.single();

    let Some((grid, _)) = cursor_to_grid(window, camera, camera_gtx, ground_gtx) else {
        return;
    };

    // TODO make this not a linear scan each time
    let building = building_query
        .iter_mut()
        .find(|(&coords, _, _)| grid == coords)
        .map(|(_, mesh, building)| (mesh, building));

    if let Some((mesh, mut building)) = building {
        // TODO make mesh update from the building height
        // use change detection https://bevy-cheatbook.github.io/programming/change-detection.html
        building.height += 1;
        let mesh = meshes.get_mut(&mesh).unwrap();
        *mesh = Mesh::from(shape::Box {
            min_x: -0.5,
            max_x: 0.5,
            min_y: -0.5,
            max_y: -0.5 + (building.height as f32),
            min_z: -0.5,
            max_z: 0.5,
        });
    } else {
        commands
            .spawn(BuildingBundle::add(
                &mut meshes,
                &mut materials,
                Building { height: 1 },
            ))
            .insert(grid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coords_roundtrip() {
        for grid in vec![
            GridCoords::ORIGIN,
            GridCoords::new(1, 2),
            GridCoords::new(-1, 0),
            GridCoords::new(-2, -3),
        ] {
            let world = grid.to_world();
            assert_eq!(grid, GridCoords::from_world(world));
        }

        for world in vec![
            Vec3::ZERO + 0.5 * Vec3::Y,
            Vec3::X + 0.5 * Vec3::Y,
            Vec3::Z + 0.5 * Vec3::Y,
            Vec3::new(-4.0, 0.5, -1.0),
            Vec3::new(4.0, 0.5, -1.0),
        ] {
            let grid = GridCoords::from_world(world);
            assert_eq!(world, grid.to_world());
        }
    }

    #[test]
    fn test_grid_from_world() {
        for (world, grid) in vec![
            (Vec3::ZERO, GridCoords::ORIGIN),
            (Vec3::X, GridCoords::new(1, 0)),
            (-Vec3::X, GridCoords::new(-1, 0)),
            (Vec3::Z, GridCoords::new(0, 1)),
            (-Vec3::Z, GridCoords::new(0, -1)),
            // round
            (Vec3::new(0.2, 0.0, 0.2), GridCoords::ORIGIN),
            (Vec3::new(0.8, 0.0, 0.8), GridCoords::new(1, 1)),
            (Vec3::new(-0.2, 0.0, -0.2), GridCoords::ORIGIN),
            (Vec3::new(-0.8, 0.0, -0.8), GridCoords::new(-1, -1)),
            // round away from zero
            (Vec3::new(0.5, 0.0, 0.5), GridCoords::new(1, 1)),
            (Vec3::new(-0.5, 0.0, -0.5), GridCoords::new(-1, -1)),
            // all points in Y column map to the same grid coords
            (Vec3::new(0.8, 1000.0, 0.8), GridCoords::new(1, 1)),
        ] {
            assert_eq!(grid, GridCoords::from_world(world), "{}", world);
        }
    }
}
