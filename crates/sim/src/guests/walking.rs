//! Deterministic group walking on a shared, layered path graph.

use std::collections::BTreeMap;

use crate::executor::{Executor, SequentialExecutor};
use crate::guests::pathfinding::Graph;
use crate::guests::{GuestId, GuestStore, generate_guests};
use crate::world::{Direction, Path, Position, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkingGuest {
    pub x: f32,
    pub y: f32,
    /// Height in tile units (path heights are quarter-tile steps).
    pub height: f32,
    pub heading_x: f32,
    pub heading_y: f32,
    pub distance: f32,
    pub moving: bool,
    pub path: Option<Position>,
    node: usize,
    next: usize,
    progress: f32,
    trip: u32,
    goal: usize,
    offset: f32,
}

impl WalkingGuest {
    pub fn settled_path(&self) -> Option<Position> {
        if self.progress == 0.0 {
            self.path
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WalkControl {
    Wander,
    Destination(Position),
    Stop,
}

pub struct WalkingSimulation {
    seed: u64,
    entrance: Option<Position>,
    world_id: usize,
    traits: GuestStore,
    guests: Vec<WalkingGuest>,
    group_for_guest: Vec<usize>,
    group_speeds: Vec<f32>,
    graph: Graph,
    revision: u64,
    tick_count: u64,
}

struct NearestPathNode {
    path: usize,
    axis: usize,
    left: Option<usize>,
    right: Option<usize>,
}

struct NearestPaths {
    nodes: Vec<NearestPathNode>,
    root: Option<usize>,
}

fn path_coordinate(path: &Path, axis: usize) -> f32 {
    match axis {
        0 => f32::from(path.position.x) + 0.5,
        1 => f32::from(path.position.y) + 0.5,
        _ => f32::from(path.position.height) * 0.25,
    }
}

impl NearestPaths {
    fn new(paths: &[Path]) -> Self {
        fn build(
            paths: &[Path],
            indices: &mut [usize],
            nodes: &mut Vec<NearestPathNode>,
            depth: usize,
        ) -> Option<usize> {
            if indices.is_empty() {
                return None;
            }
            let axis = depth % 3;
            let middle = indices.len() / 2;
            indices.select_nth_unstable_by(middle, |a, b| {
                path_coordinate(&paths[*a], axis)
                    .total_cmp(&path_coordinate(&paths[*b], axis))
                    .then(a.cmp(b))
            });
            let (left, remaining) = indices.split_at_mut(middle);
            let (center, right) = remaining.split_first_mut().unwrap();
            let index = nodes.len();
            nodes.push(NearestPathNode {
                path: *center,
                axis,
                left: None,
                right: None,
            });
            let left = build(paths, left, nodes, depth + 1);
            let right = build(paths, right, nodes, depth + 1);
            nodes[index].left = left;
            nodes[index].right = right;
            Some(index)
        }
        let mut nodes = Vec::with_capacity(paths.len());
        let root = build(
            paths,
            &mut (0..paths.len()).collect::<Vec<_>>(),
            &mut nodes,
            0,
        );
        Self { nodes, root }
    }

    fn nearest(&self, paths: &[Path], x: f32, y: f32, height: f32) -> Option<usize> {
        fn search(
            index: &NearestPaths,
            paths: &[Path],
            node: Option<usize>,
            target: [f32; 3],
            best: &mut Option<(f32, usize)>,
        ) {
            let Some(node) = node else {
                return;
            };
            let node = &index.nodes[node];
            let path = &paths[node.path];
            let dx = path_coordinate(path, 0) - target[0];
            let dy = path_coordinate(path, 1) - target[1];
            let dz = path_coordinate(path, 2) - target[2];
            let distance = dx * dx + dy * dy + dz * dz;
            if best.is_none_or(|(current, path)| {
                distance.total_cmp(&current).is_lt()
                    || (distance.total_cmp(&current).is_eq() && node.path < path)
            }) {
                *best = Some((distance, node.path));
            }
            let difference = path_coordinate(path, node.axis) - target[node.axis];
            let (near, far) = if difference > 0.0 {
                (node.left, node.right)
            } else {
                (node.right, node.left)
            };
            search(index, paths, near, target, best);
            if best.is_none_or(|(distance, _)| difference * difference <= distance) {
                search(index, paths, far, target, best);
            }
        }
        let mut best = None;
        search(self, paths, self.root, [x, y, height], &mut best);
        best.map(|(_, index)| index)
    }
}

impl WalkingSimulation {
    pub fn new(seed: u64, guest_count: usize, world: &World) -> Self {
        Self::build(seed, guest_count, world, None)
    }
    pub fn with_entrance(seed: u64, guest_count: usize, world: &World, entrance: Position) -> Self {
        assert!(
            world.path(entrance).is_some(),
            "entrance must be an existing path"
        );
        Self::build(seed, guest_count, world, Some(entrance))
    }
    fn build(seed: u64, guest_count: usize, world: &World, entrance: Option<Position>) -> Self {
        let traits = generate_guests(seed, guest_count);
        let graph = Graph::with_entrance(world, entrance);
        let spawn_nodes: Vec<_> = graph
            .normal
            .iter()
            .enumerate()
            .filter_map(|(node, &normal)| normal.then_some(node))
            .collect();
        let mut group_for_guest = vec![0; guest_count];
        let mut group_speeds = Vec::with_capacity(traits.groups.len());
        let mut guests = vec![
            WalkingGuest {
                x: 0.0,
                y: 0.0,
                height: 0.0,
                heading_x: 0.0,
                heading_y: 0.0,
                distance: 0.0,
                moving: false,
                path: None,
                node: 0,
                next: 0,
                progress: 0.0,
                trip: 0,
                goal: usize::MAX,
                offset: 0.0
            };
            guest_count
        ];
        for (group_index, group) in traits.groups.iter().enumerate() {
            let speed = group
                .members
                .iter()
                .map(|&id| traits.walking_speed(id))
                .min()
                .unwrap();
            group_speeds.push(f32::from(speed) * 0.0003);
            for (member_index, &GuestId(id)) in group.members.iter().enumerate() {
                let index = id as usize;
                group_for_guest[index] = group_index;
                if !spawn_nodes.is_empty() {
                    let node = spawn_nodes[group_index % spawn_nodes.len()];
                    guests[index].node = node;
                    guests[index].next = node;
                    guests[index].path = Some(graph.paths[node].position);
                    set_position(&mut guests[index], &graph, 0.0);
                    guests[index].x +=
                        (member_index as f32 - (group.members.len() - 1) as f32 * 0.5) * 0.018;
                    guests[index].y += (member_index % 2) as f32 * 0.018;
                }
            }
        }
        Self {
            seed,
            entrance,
            world_id: world.instance_id(),
            traits,
            guests,
            group_for_guest,
            group_speeds,
            graph,
            revision: world.revision(),
            tick_count: 0,
        }
    }
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
    pub fn traits(&self) -> &GuestStore {
        &self.traits
    }
    pub fn traits_mut(&mut self) -> &mut GuestStore {
        &mut self.traits
    }
    pub fn set_entrance(&mut self, world: &World, entrance: Option<Position>) {
        if self.entrance != entrance {
            self.entrance = entrance;
            self.revision = world.revision().wrapping_sub(1);
            self.refresh_graph(world);
        }
    }
    pub fn admit(&mut self, ids: &[GuestId]) {
        let entrance = self.entrance.expect("admissions require an entrance");
        let node = self.graph.indices[&entrance];
        for &GuestId(id) in ids {
            let guest = &mut self.guests[id as usize];
            guest.node = node;
            guest.next = node;
            guest.progress = 0.0;
            guest.offset = 0.0;
            guest.goal = usize::MAX;
            guest.path = Some(entrance);
            guest.moving = false;
            set_position(guest, &self.graph, 0.0);
        }
    }
    pub fn place_on_path(&mut self, id: GuestId, position: Position) {
        let node = *self
            .graph
            .indices
            .get(&position)
            .expect("initial guest position must be a path");
        let guest = &mut self.guests[id.0 as usize];
        guest.node = node;
        guest.next = node;
        guest.progress = 0.0;
        guest.offset = 0.0;
        guest.goal = usize::MAX;
        guest.path = Some(position);
        guest.moving = false;
        set_position(guest, &self.graph, 0.0);
    }
    pub fn path_positions(&self) -> impl Iterator<Item = Position> + '_ {
        self.graph
            .paths
            .iter()
            .zip(&self.graph.normal)
            .filter_map(|(path, &normal)| normal.then_some(path.position))
    }
    pub fn guests(&self) -> &[WalkingGuest] {
        &self.guests
    }
    pub fn path_component(&self, position: Position) -> Option<usize> {
        self.graph
            .indices
            .get(&position)
            .map(|&node| self.graph.components[node])
    }
    pub fn tick(&mut self, world: &World) {
        self.tick_with(world, &SequentialExecutor::new(256));
    }
    pub fn tick_with<E: Executor<WalkingGuest>>(&mut self, world: &World, executor: &E) {
        self.tick_internal(world, executor, None);
    }
    pub fn tick_with_controls<E: Executor<WalkingGuest>>(
        &mut self,
        world: &World,
        executor: &E,
        controls: &[WalkControl],
    ) {
        assert_eq!(controls.len(), self.guests.len());
        self.tick_internal(world, executor, Some(controls));
    }
    pub fn refresh_graph(&mut self, world: &World) {
        assert_eq!(
            self.world_id,
            world.instance_id(),
            "walking simulation received a different world"
        );
        if self.revision != world.revision() {
            let old_graph =
                std::mem::replace(&mut self.graph, Graph::with_entrance(world, self.entrance));
            self.revision = world.revision();
            let mut nearest_paths = None;
            for guest in &mut self.guests {
                let current = guest
                    .path
                    .and_then(|path| self.graph.indices.get(&path).copied());
                let next = old_graph
                    .paths
                    .get(guest.next)
                    .and_then(|path| self.graph.indices.get(&path.position).copied());
                if let (Some(node), Some(next)) = (current, next) {
                    if node == next || self.graph.neighbours[node].contains(&next) {
                        guest.node = node;
                        guest.next = next;
                        guest.goal = usize::MAX;
                        continue;
                    }
                }
                let nearest = nearest_paths
                    .get_or_insert_with(|| NearestPaths::new(&self.graph.paths))
                    .nearest(&self.graph.paths, guest.x, guest.y, guest.height);
                let Some(nearest) = nearest else {
                    guest.path = None;
                    guest.moving = false;
                    continue;
                };
                guest.node = nearest;
                guest.next = nearest;
                guest.progress = 0.0;
                guest.offset = 0.0;
                guest.goal = usize::MAX;
                guest.path = Some(self.graph.paths[nearest].position);
                set_position(guest, &self.graph, 0.0);
            }
        }
    }
    fn tick_internal<E: Executor<WalkingGuest>>(
        &mut self,
        world: &World,
        executor: &E,
        controls: Option<&[WalkControl]>,
    ) {
        self.refresh_graph(world);
        for (index, guest) in self.guests.iter_mut().enumerate() {
            if guest.path.is_none() {
                continue;
            }
            if let Some(controls) = controls {
                match controls[index] {
                    WalkControl::Stop => continue,
                    WalkControl::Destination(position) => {
                        if let Some(&goal) = self.graph.indices.get(&position) {
                            if self.graph.components[guest.node] == self.graph.components[goal] {
                                guest.goal = goal;
                                self.graph.ensure_field(goal);
                            } else {
                                guest.goal = usize::MAX;
                            }
                        } else {
                            guest.goal = usize::MAX;
                        }
                        continue;
                    }
                    WalkControl::Wander => {
                        if guest.goal == guest.node && guest.progress == 0.0 {
                            guest.goal = usize::MAX;
                        }
                    }
                }
            }
            let component = self.graph.components[guest.node];
            let goals = &self.graph.goals[component];
            if goals.len() < 2 {
                continue;
            }
            if guest.goal == usize::MAX {
                let group = self.group_for_guest[index];
                let mut choice = goal_index(self.seed, group, guest.trip, goals.len());
                if goals[choice] == guest.node {
                    choice = (choice + 1) % goals.len();
                }
                guest.goal = goals[choice];
            }
            self.graph.ensure_field(guest.goal);
        }
        let graph = &self.graph;
        let mut occupancy: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
        for (index, guest) in self.guests.iter().enumerate() {
            if guest.path.is_some() && self.traits.is_active(GuestId(index as u32)) {
                occupancy
                    .entry((guest.x.floor() as i32, guest.y.floor() as i32))
                    .or_default()
                    .push(index);
            }
        }
        let old = &self.guests;
        self.guests = executor.execute(old.len(), |index| {
            let mut guest = old[index];
            if guest.path.is_none() {
                return guest;
            }
            if controls.is_some_and(|controls| controls[index] == WalkControl::Stop)
                || guest.goal == usize::MAX
                || (guest.goal == guest.node && guest.progress == 0.0)
            {
                guest.moving = false;
                return guest;
            }
            let group = self.group_for_guest[index];
            let component = graph.components[guest.node];
            if (self.tick_count + (group as u64 * 37)) % 300 < 12 {
                guest.moving = false;
                return guest;
            }
            if graph.goals[component].len() < 2
                && controls.is_none_or(|controls| controls[index] == WalkControl::Wander)
            {
                guest.moving = false;
                return guest;
            }
            if guest.next == guest.node {
                guest.next = graph.fields[&guest.goal][&guest.node];
            }
            let speed = self.group_speeds[group];
            guest.progress = (guest.progress + speed).min(1.0);
            guest.moving = true;
            let from = graph.paths[guest.node].position;
            let to = graph.paths[guest.next].position;
            guest.heading_x = f32::from(to.x) - f32::from(from.x);
            guest.heading_y = f32::from(to.y) - f32::from(from.y);
            // A bounded sample from this tile and connected adjacent tiles keeps
            // crowd work predictable even when many guests occupy one tile.
            let mut desired_offset = ((index % 5) as f32 - 2.0) * 0.045;
            let cell_x = guest.x.floor() as i32;
            let cell_y = guest.y.floor() as i32;
            for dy_cell in -1..=1 {
                for dx_cell in -1..=1 {
                    if let Some(occupants) = occupancy.get(&(cell_x + dx_cell, cell_y + dy_cell)) {
                        for sample in 0..occupants.len().min(4) {
                            let other = old[occupants[(index + sample) % occupants.len()]];
                            let dx = guest.x - other.x;
                            let dy = guest.y - other.y;
                            if (guest.height - other.height).abs() < 0.2 && dx * dx + dy * dy < 0.09
                            {
                                desired_offset += (dx * -guest.heading_y + dy * guest.heading_x)
                                    .clamp(-0.03, 0.03);
                            }
                        }
                    }
                }
            }
            let following = graph.fields[&guest.goal][&guest.next];
            let following_position = graph.paths[following].position;
            let continues_straight = following != guest.next
                && f32::from(following_position.x) - f32::from(to.x) == guest.heading_x
                && f32::from(following_position.y) - f32::from(to.y) == guest.heading_y;
            let envelope = if continues_straight {
                1.0
            } else {
                ((1.0 - guest.progress) * 2.0).min(1.0)
            };
            let target = desired_offset.clamp(-0.18, 0.18) * envelope;
            guest.offset += (target - guest.offset).clamp(-0.01, 0.01);
            // End at the center so changes of direction cannot cause a jump.
            guest.offset = guest.offset.clamp(-envelope * 0.18, envelope * 0.18);
            let previous_x = guest.x;
            let previous_y = guest.y;
            let offset = guest.offset;
            set_position(&mut guest, graph, offset);
            guest.distance +=
                ((guest.x - previous_x).powi(2) + (guest.y - previous_y).powi(2)).sqrt();
            if guest.progress >= 1.0 {
                guest.node = guest.next;
                guest.progress = 0.0;
                guest.path = Some(graph.paths[guest.node].position);
                if guest.goal == guest.node {
                    guest.trip = guest.trip.wrapping_add(1);
                    guest.goal = usize::MAX;
                }
            }
            guest
        });
        self.tick_count += 1;
    }
    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in self
            .seed
            .to_le_bytes()
            .into_iter()
            .chain(self.tick_count.to_le_bytes())
            .chain(
                self.entrance
                    .map_or(u64::MAX, |position| {
                        (u64::from(position.x) << 32)
                            | (u64::from(position.y) << 16)
                            | u64::from(position.height as u16)
                    })
                    .to_le_bytes(),
            )
            .chain(self.guests.iter().flat_map(|guest| {
                [
                    guest.x.to_bits(),
                    guest.y.to_bits(),
                    guest.height.to_bits(),
                    guest.distance.to_bits(),
                    guest.node as u32,
                    guest.next as u32,
                    guest.progress.to_bits(),
                    guest.trip,
                    guest.goal as u32,
                    guest.offset.to_bits(),
                    guest.heading_x.to_bits(),
                    guest.heading_y.to_bits(),
                    u32::from(guest.moving),
                    guest.path.map_or(u32::MAX, |path| {
                        (u32::from(path.x) << 16) | u32::from(path.y)
                    }),
                    guest.path.map_or(u32::MAX, |path| path.height as u32),
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
            }))
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        hash
    }
}

fn goal_index(seed: u64, group: usize, trip: u32, count: usize) -> usize {
    let mut value = seed
        ^ (group as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ u64::from(trip).wrapping_mul(0xd1b54a32d192ed03);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((value ^ (value >> 31)) % count as u64) as usize
}

fn set_position(guest: &mut WalkingGuest, graph: &Graph, offset: f32) {
    let from = graph.paths[guest.node];
    let to = graph.paths[guest.next];
    let t = guest.progress;
    guest.x = f32::from(from.position.x)
        + 0.5
        + (f32::from(to.position.x) - f32::from(from.position.x)) * t
        - guest.heading_y * offset;
    guest.y = f32::from(from.position.y)
        + 0.5
        + (f32::from(to.position.y) - f32::from(from.position.y)) * t
        + guest.heading_x * offset;
    let path = if t < 0.5 { from } else { to };
    let local_x = guest.x - f32::from(path.position.x);
    let local_y = guest.y - f32::from(path.position.y);
    let slope = match path.slope {
        None => 0.0,
        Some(Direction::North) => 1.0 - local_y,
        Some(Direction::East) => local_x,
        Some(Direction::South) => local_y,
        Some(Direction::West) => 1.0 - local_x,
    };
    guest.height = (f32::from(path.position.height) + slope) * 0.25;
}

#[cfg(test)]
mod recovery_tests {
    use super::{NearestPaths, WalkingSimulation};
    use crate::world::{Path, Position, Surface, World};

    #[test]
    fn nearest_paths_matches_full_search_for_layered_paths_and_ties() {
        let paths: Vec<_> = (0..120)
            .map(|index| Path {
                position: Position {
                    x: ((index * 37) % 43) as u16,
                    y: ((index * 19) % 47) as u16,
                    height: ((index * 11) % 31) as i16 - 15,
                },
                slope: None,
            })
            .collect();
        let index = NearestPaths::new(&paths);
        for sample in 0..500 {
            let x = ((sample * 17) % 90) as f32 / 2.0;
            let y = ((sample * 29) % 100) as f32 / 2.0;
            let height = ((sample * 7) % 44) as f32 / 4.0 - 5.5;
            let expected = paths
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let distance = |path: &Path| {
                        let dx = f32::from(path.position.x) + 0.5 - x;
                        let dy = f32::from(path.position.y) + 0.5 - y;
                        let dz = f32::from(path.position.height) * 0.25 - height;
                        dx * dx + dy * dy + dz * dz
                    };
                    distance(a).total_cmp(&distance(b))
                })
                .map(|(index, _)| index);
            assert_eq!(index.nearest(&paths, x, y, height), expected);
        }
        let tied = [
            Path {
                position: Position {
                    x: 0,
                    y: 0,
                    height: 0,
                },
                slope: None,
            },
            Path {
                position: Position {
                    x: 2,
                    y: 0,
                    height: 0,
                },
                slope: None,
            },
        ];
        assert_eq!(
            NearestPaths::new(&tied).nearest(&tied, 1.5, 0.5, 0.0),
            Some(0)
        );
        assert_eq!(NearestPaths::new(&[]).nearest(&[], 0.0, 0.0, 0.0), None);
    }

    #[test]
    fn each_group_member_recovers_to_their_nearest_path() {
        let mut world = World::new(
            3,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..3 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        let mut simulation = WalkingSimulation::new(7, 100, &world);
        let group = simulation
            .traits
            .groups
            .iter()
            .find(|group| group.members.len() >= 2)
            .unwrap();
        let first = group.members[0].0 as usize;
        let second = group.members[1].0 as usize;
        for (index, x) in [(first, 1.1), (second, 1.9)] {
            simulation.guests[index].x = x;
            simulation.guests[index].y = 0.5;
            simulation.guests[index].path = Some(Position {
                x: 1,
                y: 0,
                height: 0,
            });
        }
        world.remove_path(Position {
            x: 1,
            y: 0,
            height: 0,
        });
        simulation.tick(&world);
        assert_eq!(simulation.guests[first].path.unwrap().x, 0);
        assert_eq!(simulation.guests[second].path.unwrap().x, 2);
    }
}

#[cfg(test)]
mod crowd_tests {
    use super::WalkingSimulation;
    use crate::world::{Path, Position, Surface, World};

    fn straight_world() -> World {
        let mut world = World::new(
            64,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..64 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        world
    }

    #[test]
    fn nearby_guest_changes_walking_direction() {
        let world = straight_world();
        let mut crowded = WalkingSimulation::new(7, 4, &world);
        let mut alone = WalkingSimulation::new(7, 4, &world);
        for simulation in [&mut crowded, &mut alone] {
            simulation.tick_count = 20;
            for (index, guest) in simulation.guests.iter_mut().enumerate() {
                guest.node = index * 10;
                guest.next = index * 10 + 1;
                guest.progress = 0.5;
                guest.x = (index * 10 + 1) as f32;
                guest.y = 0.5;
                guest.path = Some(simulation.graph.paths[guest.node].position);
            }
        }
        crowded.guests[3].x = crowded.guests[2].x;
        crowded.guests[3].y = 0.6;
        crowded.tick(&world);
        alone.tick(&world);
        assert!(crowded.guests[2].y < alone.guests[2].y);
    }

    #[test]
    fn feasible_crowd_remains_spread_across_path_centers() {
        let world = straight_world();
        let mut simulation = WalkingSimulation::new(7, 60, &world);
        let mut maximum_close = 0;
        for _ in 0..250 {
            simulation.tick(&world);
            for guest in simulation.guests() {
                let nearby = simulation
                    .guests()
                    .iter()
                    .filter(|other| {
                        (guest.x - other.x).abs() < 0.025 && (guest.y - other.y).abs() < 0.025
                    })
                    .count();
                maximum_close = maximum_close.max(nearby);
            }
        }
        assert!(
            maximum_close <= 8,
            "{maximum_close} guests occupied a small area"
        );
        // At least one guest crossing a tile center keeps a visible lateral offset.
        assert!(
            simulation
                .guests()
                .iter()
                .any(|guest| guest.progress < 0.04 && guest.offset.abs() > 0.02)
        );
    }
}

#[cfg(test)]
mod destination_tests {
    use super::{WalkControl, WalkingSimulation, goal_index};
    use crate::world::{Path, Position, Surface, World};

    #[test]
    fn seeded_destinations_are_reproducible_and_vary_with_trip_and_seed() {
        let choices = |seed| {
            (0..32)
                .map(|trip| goal_index(seed, 3, trip, 8))
                .collect::<Vec<_>>()
        };
        assert_eq!(choices(17), choices(17));
        assert_ne!(choices(17), choices(18));
        assert!(
            choices(17)
                .windows(2)
                .any(|pair| pair[1] != (pair[0] + 1) % 8)
        );
        assert!(
            choices(17)
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 4
        );
    }

    #[test]
    fn explicit_destination_routes_and_stop_holds_position() {
        let mut world = World::new(
            12,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..12 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        let mut simulation = WalkingSimulation::new(7, 1, &world);
        let destination = Position {
            x: 11,
            y: 0,
            height: 0,
        };
        for _ in 0..1000 {
            simulation.tick_with_controls(
                &world,
                &crate::executor::SequentialExecutor::new(16),
                &[WalkControl::Destination(destination)],
            );
        }
        assert_eq!(simulation.guests()[0].path, Some(destination));
        let before = simulation.guests()[0];
        simulation.tick_with_controls(
            &world,
            &crate::executor::SequentialExecutor::new(16),
            &[WalkControl::Stop],
        );
        assert_eq!(simulation.guests()[0].x, before.x);
        assert_eq!(simulation.guests()[0].y, before.y);
    }
}

#[cfg(test)]
mod entrance_tests {
    use super::WalkingSimulation;
    use crate::world::{Path, Position, Surface, World};

    fn fixture() -> World {
        let mut world = World::new(
            16,
            1,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        for x in 0..16 {
            world.place_path(Path {
                position: Position { x, y: 0, height: 0 },
                slope: None,
            });
        }
        world
    }

    #[test]
    fn entrance_is_a_destination_and_remains_available_after_graph_rebuild() {
        let mut world = fixture();
        let entrance = Position {
            x: 1,
            y: 0,
            height: 0,
        };
        let mut simulation = WalkingSimulation::with_entrance(1, 20, &world, entrance);
        let entrance_index = simulation.graph.indices[&entrance];
        assert!(
            simulation.graph.goals[simulation.graph.components[entrance_index]]
                .contains(&entrance_index)
        );
        world.remove_path(Position {
            x: 15,
            y: 0,
            height: 0,
        });
        simulation.tick(&world);
        let entrance_index = simulation.graph.indices[&entrance];
        assert!(
            simulation.graph.goals[simulation.graph.components[entrance_index]]
                .contains(&entrance_index)
        );
    }

    #[test]
    fn isolated_guests_wait_on_their_valid_path() {
        let mut world = fixture();
        let entrance = Position {
            x: 1,
            y: 0,
            height: 0,
        };
        let mut simulation = WalkingSimulation::with_entrance(1, 100, &world, entrance);
        world.remove_path(Position {
            x: 14,
            y: 0,
            height: 0,
        });
        let isolated = Position {
            x: 15,
            y: 0,
            height: 0,
        };
        let indices: Vec<_> = simulation
            .guests()
            .iter()
            .enumerate()
            .filter(|(_, guest)| guest.path == Some(isolated))
            .map(|(index, _)| index)
            .collect();
        assert!(!indices.is_empty());
        for _ in 0..30 {
            simulation.tick(&world);
        }
        for index in indices {
            assert_eq!(simulation.guests()[index].path, Some(isolated));
            assert!(!simulation.guests()[index].moving);
        }
    }

    #[test]
    #[should_panic(expected = "entrance must be an existing path")]
    fn invalid_entrance_is_rejected() {
        let world = fixture();
        WalkingSimulation::with_entrance(
            1,
            1,
            &world,
            Position {
                x: 15,
                y: 0,
                height: 4,
            },
        );
    }

    #[test]
    #[should_panic(expected = "walking simulation received a different world")]
    fn another_world_with_the_same_revision_is_rejected() {
        let first = fixture();
        let second = fixture();
        let mut simulation = WalkingSimulation::new(1, 1, &first);
        simulation.tick(&second);
    }
}
