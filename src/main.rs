use std::f32::consts::PI;

use bevy::prelude::*;
use bracket_pathfinding::prelude::{
    a_star_search, Algorithm2D, BaseMap, NavigationPath, Point as BracketPoint, SmallVec,
};
use rand::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Options>()
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_move_camera)
        .add_systems(Update, keyboard_set_options)
        .add_systems(Update, position_objects_on_grid)
        .add_systems(Update, move_light)
        .add_systems(Update, move_cursor)
        .add_systems(Update, add_buildings)
        .add_systems(Update, reset_paths_after_city_changes)
        .add_systems(Update, people_walk)
        .add_systems(Update, apply_velocities)
        .run();
}

const CAMERA_MOVE_SPEED: f32 = 3.0;
const CAMERA_ZOOM_SPEED: f32 = 0.2;

const NUM_PEOPLE: usize = 10;
const PERSON_HEIGHT: f32 = 0.1;
const PERSON_SPEED: f32 = 1.0;

#[derive(Default, Resource)]
struct Options {
    draw_paths: bool,
    draw_selection: bool,
}

const STARTING_CITY: [Height; 25] = [
    0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, //
    0, 0, 3, 1, 0, //
    0, 1, 0, 0, 0, //
    0, 2, 0, 0, 0, //
];

#[derive(Resource)]
struct City<const L: usize> {
    heights: [Height; L],
    x_len: usize,
    y_len: usize,
}

impl<const L: usize> City<L> {
    fn new(heights: [Height; L]) -> Self {
        let size_f = (heights.len() as f32).sqrt();
        let floor = size_f.floor();
        assert_eq!(size_f, floor);
        let size = floor as usize;
        assert_eq!(size, 5); // TODO

        Self {
            heights,
            x_len: size,
            y_len: size,
        }
    }

    fn buildings_iter<'a>(&'a self) -> impl Iterator<Item = (GridCoords, Height)> + 'a {
        self.heights.iter().enumerate().flat_map(move |(i, &h)| {
            if h > 0 {
                Some((self.index_to_coords(i), h))
            } else {
                None
            }
        })
    }

    fn height_at_coords(&self, coords: GridCoords) -> Option<Height> {
        let idx = self.coords_to_index(coords)?;
        let h = self.heights[idx];
        if h > 0 {
            Some(h)
        } else {
            None
        }
    }

    fn set_height_at_coords(&mut self, coords: GridCoords, height: Option<Height>) {
        let Some(idx) = self.coords_to_index(coords) else {
            return;
        };
        self.heights[idx] = height.unwrap_or(0);
    }

    fn coords_to_index(&self, coords: GridCoords) -> Option<usize> {
        let shifted_y = coords.y + (self.y_len as i8 / 2);
        let shifted_x = coords.x + (self.x_len as i8 / 2);
        if shifted_x < 0
            || shifted_x as usize >= self.x_len
            || shifted_y < 0
            || shifted_y as usize >= self.y_len
        {
            None
        } else {
            Some(shifted_y as usize * self.y_len + shifted_x as usize)
        }
    }

    fn index_to_coords(&self, idx: usize) -> GridCoords {
        let half_xl = (self.x_len / 2) as i8;
        let half_yl = (self.y_len / 2) as i8;

        let x = idx % self.x_len;
        let y = idx / self.y_len;

        GridCoords::new(x as i8 - half_xl, (y as i8) - half_yl)
    }

    fn index_to_world(&self, idx: usize, elevation: f32) -> Vec3 {
        self.index_to_coords(idx).to_world(elevation)
    }

    fn valid_exit(&self, coords: GridCoords) -> Option<usize> {
        if let None = self.height_at_coords(coords) {
            self.coords_to_index(coords)
        } else {
            None
        }
    }
}

impl<const L: usize> BaseMap for City<L> {
    fn get_available_exits(&self, idx: usize) -> SmallVec<[(usize, f32); 10]> {
        let mut exits = SmallVec::new();
        let coords = self.index_to_coords(idx);

        if let Some(idx) = self.valid_exit(coords.up()) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(coords.down()) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(coords.left()) {
            exits.push((idx, 1.0))
        }
        if let Some(idx) = self.valid_exit(coords.right()) {
            exits.push((idx, 1.0))
        }

        exits
    }

    fn get_pathing_distance(&self, idx1: usize, idx2: usize) -> f32 {
        let coords1 = self.index_to_coords(idx1);
        let coords2 = self.index_to_coords(idx2);
        coords1.manhattan_dist(coords2) as f32
    }
}

impl<const L: usize> Algorithm2D for City<L> {
    fn dimensions(&self) -> BracketPoint {
        BracketPoint::new(self.x_len, self.y_len)
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut window_query: Query<&mut Window>,
) {
    let city = City::new(STARTING_CITY);
    let building_coords = city.buildings_iter();

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
            material: materials.add(Color::BLACK.into()),
            ..default()
        })
        .insert(Cursor);

    // person
    // TODO bundle me
    let mut rng = rand::thread_rng();
    for _ in 0..NUM_PEOPLE {
        let x = rng.gen_range(-2.0..2.0);
        let z = rng.gen_range(-2.0..2.0);
        commands
            .spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Cylinder {
                    radius: 0.025,
                    height: PERSON_HEIGHT,
                    ..default()
                })),
                material: materials.add(Color::rgb(0.1, 0.1, 0.1).into()),
                transform: Transform::from_xyz(x, PERSON_HEIGHT * 0.5, z),
                ..default()
            })
            .insert(Person::default())
            .insert(Velocity::ZERO);
    }

    commands.insert_resource(city);
}

fn position_objects_on_grid(mut q: Query<(&mut Transform, &GridCoords)>) {
    for (mut tx, coords) in &mut q {
        tx.translation = coords.to_world(0.5); // TODO
    }
}

fn keyboard_move_camera(
    time: Res<Time>,
    keys: Res<Input<KeyCode>>,
    mut q: Query<(&mut Projection, &mut Transform)>,
) {
    let secs = time.delta_seconds();
    let (mut proj, mut camera_tx) = q.single_mut();

    let velocity_right = if keys.pressed(KeyCode::A) {
        1.0
    } else if keys.pressed(KeyCode::D) {
        -1.0
    } else {
        0.0
    };

    let velocity = velocity_right * camera_tx.right();
    if velocity != Vec3::ZERO {
        camera_tx.translation += velocity * CAMERA_MOVE_SPEED * secs;
        camera_tx.look_at(Vec3::ZERO, Vec3::Y);
    }

    // trickery to deal with the Mut<> of an enum
    let Projection::Orthographic(proj) = &mut *proj else {
        unreachable!("projection is no longer orthographic");
    };
    let scale_amount = (CAMERA_ZOOM_SPEED * CAMERA_MOVE_SPEED * secs).clamp(0.0, 0.1);
    if keys.pressed(KeyCode::W) {
        let factor = 1.0 - scale_amount;
        proj.scale = (proj.scale * factor).max(3.0);
    } else if keys.pressed(KeyCode::S) {
        let factor = 1.0 + scale_amount;
        proj.scale = (proj.scale * factor).min(100.0);
    }
}

fn keyboard_set_options(keys: Res<Input<KeyCode>>, mut options: ResMut<Options>) {
    if keys.just_pressed(KeyCode::P) {
        options.draw_paths = !options.draw_paths;
    }
    if keys.just_pressed(KeyCode::E) {
        options.draw_selection = !options.draw_selection;
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

    fn to_world(&self, elevation: f32) -> Vec3 {
        // grid xy is world xz (world y is elevation)
        Vec3::new(self.x as f32, elevation, self.y as f32)
    }

    fn manhattan_dist(&self, dest: Self) -> i8 {
        (dest.x - self.x).abs() + (dest.y - self.y).abs()
    }

    fn up(&self) -> Self {
        Self {
            x: self.x,
            y: self.y + 1,
        }
    }

    fn down(&self) -> Self {
        Self {
            x: self.x,
            y: self.y - 1,
        }
    }

    fn left(&self) -> Self {
        Self {
            x: self.x - 1,
            y: self.y,
        }
    }

    fn right(&self) -> Self {
        Self {
            x: self.x + 1,
            y: self.y,
        }
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

fn move_cursor(
    mut cursor_query: Query<&mut Transform, With<Cursor>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    ground_query: Query<&GlobalTransform, With<Ground>>,
    window_query: Query<&Window>,
    building_query: Query<(&GridCoords, &Building)>,
    options: Res<Options>,
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

    if options.draw_selection {
        // TODO make this not a linear scan each time
        let building = building_query
            .iter()
            .find(|(&coords, _)| grid == coords)
            .map(|(_, building)| building);
        let height = building.map_or(0, |b| b.height) as f32;
        let selection_center = grid.to_world(height);

        let rotation = Quat::from_rotation_x(PI * 0.5);
        gizmos.rect(selection_center, rotation, Vec2::ONE, Color::ANTIQUE_WHITE);
    }
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
    mut city: ResMut<City<25>>,
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

        city.set_height_at_coords(grid, Some(building.height));

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
        city.set_height_at_coords(grid, Some(1));

        commands
            .spawn(BuildingBundle::add(
                &mut meshes,
                &mut materials,
                Building { height: 1 },
            ))
            .insert(grid);
    }
}

#[derive(Component)]
struct Velocity(Vec3);

impl Velocity {
    const ZERO: Self = Self(Vec3::ZERO);
}

#[derive(Component)]
struct Person {
    goal: Option<GridCoords>,
    path: NavigationPath,
}

impl Person {
    fn reset_path(&mut self) {
        self.path = default();
    }
}

impl Default for Person {
    fn default() -> Self {
        Person {
            goal: None,
            path: default(),
        }
    }
}

fn reset_paths_after_city_changes(city: Res<City<25>>, mut people: Query<&mut Person>) {
    if city.is_changed() {
        for mut person in &mut people {
            person.reset_path();
        }
    }
}

fn people_walk(
    city: Res<City<25>>,
    mut query: Query<(&mut Person, &Transform, &mut Velocity)>,
    options: Res<Options>,
    mut gizmos: Gizmos,
) {
    for (mut person, tx, mut velocity) in &mut query {
        let mut rng = rand::thread_rng();

        let coords = GridCoords::from_world(tx.translation);

        if person.goal.is_none() || person.goal.is_some_and(|goal| goal == coords) {
            let goal = GridCoords::new(rng.gen_range(-2..=2), rng.gen_range(-2..=2));
            eprintln!("new goal: {:?}", goal);
            dbg!(city.height_at_coords(goal));
            person.goal = Some(goal);

            person.reset_path();
        }

        if person.path.steps.is_empty() {
            eprintln!("empty path, replanning");
            let goal = person.goal.unwrap(); // previous condition assigned it
            let path = a_star_search(
                city.coords_to_index(coords).unwrap(),
                city.coords_to_index(goal).unwrap(),
                city.as_ref(),
            );

            if path.steps.is_empty() {
                eprintln!("unreachable goal, try again later");
                person.goal = None;
            } else {
                person.path = path;
                dbg!(&person.path.steps);
            }
        }

        if options.draw_paths {
            let mut path_dbg_from = tx.translation;
            for &step in &person.path.steps {
                let path_dbg_to = city.index_to_world(step, PERSON_HEIGHT * 0.5);
                gizmos.line(path_dbg_from, path_dbg_to, Color::rgba_u8(0, 0, 0, 100));
                path_dbg_from = path_dbg_to;
            }
        }

        if let Some(&step) = person.path.steps.first() {
            let goal_coords = city.index_to_coords(step);

            if goal_coords == coords {
                velocity.0 = Vec3::ZERO;
                person.path.steps = person.path.steps[1..].to_vec(); // TODO inefficient
                eprintln!("reached next step, steps now: {:?}", person.path.steps);
            } else {
                let goal_center = goal_coords.to_world(PERSON_HEIGHT * 0.5);
                let direction = goal_center - tx.translation;
                velocity.0 = direction.normalize_or_zero() * PERSON_SPEED;
            }
        } else {
            eprintln!("nowhere to go for now");
            velocity.0 = Vec3::ZERO;
        }
    }
}

fn apply_velocities(time: Res<Time>, mut q: Query<(&mut Transform, &Velocity)>) {
    let secs = time.delta_seconds();
    for (mut tx, &Velocity(v)) in &mut q {
        tx.translation += v * secs;
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
            let world = grid.to_world(0.5);
            assert_eq!(grid, GridCoords::from_world(world));
        }

        for world in vec![
            Vec3::ZERO + 0.5 * Vec3::Y,
            Vec3::X + 0.5 * Vec3::Y,
            Vec3::Z + 0.5 * Vec3::Y,
            Vec3::new(-4.0, 0.5, -1.0),
            Vec3::new(4.0, -1.5, -1.0),
        ] {
            let grid = GridCoords::from_world(world);
            assert_eq!(world, grid.to_world(world.y));
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
