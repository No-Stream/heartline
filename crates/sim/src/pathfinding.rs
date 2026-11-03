use crate::world::{Path, Position, World};
use std::collections::{BTreeMap, VecDeque};

pub(crate) struct Graph {
    pub(crate) paths: Vec<Path>,
    pub(crate) neighbours: Vec<Vec<usize>>,
    pub(crate) indices: BTreeMap<Position, usize>,
    pub(crate) components: Vec<usize>,
    pub(crate) goals: Vec<Vec<usize>>,
    pub(crate) fields: BTreeMap<usize, BTreeMap<usize, usize>>,
}

impl Graph {
    pub(crate) fn with_entrance(world: &World, entrance: Option<Position>) -> Self {
        let paths: Vec<_> = world.paths().collect();
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
                let count = nodes.len().min(8);
                (0..count)
                    .map(|index| nodes[index * nodes.len() / count])
                    .collect()
            })
            .collect();
        if let Some(node) = entrance.and_then(|position| indices.get(&position).copied()) {
            let component_goals = &mut goals[components[node]];
            if !component_goals.contains(&node) {
                component_goals.push(node);
            }
        }
        Self {
            paths,
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
