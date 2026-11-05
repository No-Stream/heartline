use crate::state_hash::StateHasher;
use crate::world::{Path, PathKind, Position, World};
use std::collections::{BTreeMap, VecDeque};

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Graph {
    pub(crate) paths: Vec<Path>,
    pub(crate) normal: Vec<bool>,
    pub(crate) queue_components: Vec<Option<usize>>,
    pub(crate) queue_ends: Vec<Vec<usize>>,
    pub(crate) queue_has_entrance: Vec<bool>,
    pub(crate) queue_entrances: Vec<Vec<usize>>,
    pub(crate) neighbours: Vec<Vec<usize>>,
    pub(crate) indices: BTreeMap<Position, usize>,
    pub(crate) components: Vec<usize>,
    pub(crate) goals: Vec<Vec<usize>>,
    pub(crate) fields: BTreeMap<usize, BTreeMap<usize, usize>>,
}

impl Graph {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"GuestGraph-v1");
        hash.write_len(self.paths.len());
        for path in &self.paths {
            write_position(hash, path.position);
            hash.write_option(path.slope, |hash, direction| {
                hash.write_u8(direction as u8);
            });
        }
        hash.write_len(self.normal.len());
        for &normal in &self.normal {
            hash.write_bool(normal);
        }
        hash.write_len(self.queue_components.len());
        for &component in &self.queue_components {
            hash.write_option(component, |hash, component| hash.write_len(component));
        }
        hash.write_len(self.queue_ends.len());
        for ends in &self.queue_ends {
            write_indices(hash, ends);
        }
        hash.write_len(self.queue_has_entrance.len());
        for &has_entrance in &self.queue_has_entrance {
            hash.write_bool(has_entrance);
        }
        hash.write_len(self.queue_entrances.len());
        for entrances in &self.queue_entrances {
            write_indices(hash, entrances);
        }
        hash.write_len(self.neighbours.len());
        for neighbours in &self.neighbours {
            write_indices(hash, neighbours);
        }
        hash.write_len(self.indices.len());
        for (position, &index) in &self.indices {
            write_position(hash, *position);
            hash.write_len(index);
        }
        write_indices(hash, &self.components);
        hash.write_len(self.goals.len());
        for goals in &self.goals {
            write_indices(hash, goals);
        }
    }

    pub(crate) fn with_entrance(world: &World, entrance: Option<Position>) -> Self {
        let paths: Vec<_> = world.paths().collect();
        let normal: Vec<_> = paths
            .iter()
            .map(|path| world.path_metadata(path.position).unwrap().kind == PathKind::Normal)
            .collect();
        let indices: BTreeMap<_, _> = paths
            .iter()
            .enumerate()
            .map(|(i, path)| (path.position, i))
            .collect();
        let neighbours: Vec<Vec<usize>> = paths
            .iter()
            .map(|path| {
                world
                    .neighbours(path.position)
                    .iter()
                    .map(|position| indices[position])
                    .collect()
            })
            .collect();
        let mut queue_components = vec![None; paths.len()];
        let mut queue_ends = Vec::new();
        let mut queue_has_entrance = Vec::new();
        let mut queue_entrances = Vec::new();
        for start in 0..paths.len() {
            if normal[start] || queue_components[start].is_some() {
                continue;
            }
            let component = queue_ends.len();
            let mut pending = VecDeque::from([start]);
            queue_components[start] = Some(component);
            let mut ends = Vec::new();
            let mut entrance = false;
            let mut entrances = Vec::new();
            while let Some(node) = pending.pop_front() {
                let mut queue_degree = 0;
                let mut touches_normal = false;
                for &next in &neighbours[node] {
                    if normal[next] {
                        entrance = true;
                        touches_normal = true;
                        entrances.push(next);
                    } else {
                        queue_degree += 1;
                        if queue_components[next].is_none() {
                            queue_components[next] = Some(component);
                            pending.push_back(next);
                        }
                    }
                }
                if queue_degree <= 1 && (!touches_normal || queue_degree == 0) {
                    ends.push(node);
                }
            }
            queue_ends.push(ends);
            queue_has_entrance.push(entrance);
            entrances.sort_unstable();
            entrances.dedup();
            queue_entrances.push(entrances);
        }
        let mut components = vec![usize::MAX; paths.len()];
        let mut members: Vec<Vec<usize>> = Vec::new();
        for start in 0..paths.len() {
            if components[start] != usize::MAX {
                continue;
            }
            let component = members.len();
            let mut queue = VecDeque::from([start]);
            components[start] = component;
            let mut nodes = Vec::new();
            while let Some(node) = queue.pop_front() {
                nodes.push(node);
                for &next in &neighbours[node] {
                    if components[next] == usize::MAX {
                        components[next] = component;
                        queue.push_back(next);
                    }
                }
            }
            nodes.sort_unstable();
            members.push(nodes);
        }
        let mut goals: Vec<Vec<usize>> = members
            .iter()
            .map(|nodes| {
                let nodes: Vec<_> = nodes.iter().copied().filter(|&node| normal[node]).collect();
                let count = nodes.len().min(8);
                (0..count)
                    .map(|index| nodes[index * nodes.len() / count])
                    .collect()
            })
            .collect();
        if let Some(node) = entrance
            .filter(|&position| {
                world
                    .path_metadata(position)
                    .is_some_and(|metadata| metadata.kind == PathKind::Normal)
            })
            .and_then(|position| indices.get(&position).copied())
        {
            let component_goals = &mut goals[components[node]];
            if !component_goals.contains(&node) {
                component_goals.push(node);
            }
        }
        Self {
            paths,
            normal,
            queue_components,
            queue_ends,
            queue_has_entrance,
            queue_entrances,
            neighbours,
            indices,
            components,
            goals,
            fields: BTreeMap::new(),
        }
    }
    pub(crate) fn ensure_field(&mut self, goal: usize) {
        if self.fields.contains_key(&goal) {
            return;
        }
        let mut next_step = BTreeMap::from([(goal, goal)]);
        let mut queue = VecDeque::from([goal]);
        while let Some(node) = queue.pop_front() {
            for &next in &self.neighbours[node] {
                if let std::collections::btree_map::Entry::Vacant(entry) = next_step.entry(next) {
                    entry.insert(node);
                    queue.push_back(next);
                }
            }
        }
        self.fields.insert(goal, next_step);
    }
}

fn write_position(hash: &mut StateHasher, position: Position) {
    hash.write_u16(position.x);
    hash.write_u16(position.y);
    hash.write_i16(position.height);
}

fn write_indices(hash: &mut StateHasher, indices: &[usize]) {
    hash.write_len(indices.len());
    for &index in indices {
        hash.write_len(index);
    }
}
