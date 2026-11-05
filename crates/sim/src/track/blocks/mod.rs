//! Placement and validation of powered block-brake runs.
mod throughput;

use crate::physics::{BlockOperation, Circuit, GRAVITY, PhysicsConfig, TIME_STEP, Train};
use crate::state_hash::StateHasher;
use crate::track::{Bank, Piece, PieceKind, Slope, TrainConfiguration};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

pub use throughput::ThroughputEstimator;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod hash_tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrakeRun {
    pub start_metres: f64,
    pub boundary_metres: f64,
    pub length_metres: f64,
    pub piece_indices: BrakePieces,
}

fn write_brake_run(hash: &mut StateHasher, run: &BrakeRun) {
    hash.write_f64(run.start_metres);
    hash.write_f64(run.boundary_metres);
    hash.write_f64(run.length_metres);
    write_indices(hash, &run.piece_indices);
}

fn write_indices(hash: &mut StateHasher, indices: &[usize]) {
    hash.write_len(indices.len());
    for &index in indices {
        hash.write_len(index);
    }
}

/// A contiguous view into the circuit's shared, doubled sequence of piece indices.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrakePieces {
    storage: Arc<[usize]>,
    start: usize,
    length: usize,
}

impl Deref for BrakePieces {
    type Target = [usize];
    fn deref(&self) -> &Self::Target {
        &self.storage[self.start..self.start + self.length]
    }
}

impl PartialEq for BrakePieces {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}
impl Eq for BrakePieces {}

impl From<Vec<usize>> for BrakePieces {
    fn from(indices: Vec<usize>) -> Self {
        let length = indices.len();
        Self {
            storage: Arc::from(indices),
            start: 0,
            length,
        }
    }
}

impl<'a> IntoIterator for &'a BrakePieces {
    type Item = &'a usize;
    type IntoIter = std::slice::Iter<'a, usize>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

fn brake_runs_overlap(first: &BrakeRun, second: &BrakeRun) -> bool {
    let intervals = |run: &BrakeRun| {
        let start = run.piece_indices.first().copied();
        let end = run.piece_indices.last().copied();
        match (start, end) {
            (Some(start), Some(end)) if start <= end => [Some((start, end)), None],
            (Some(start), Some(end)) => [Some((start, usize::MAX)), Some((0, end))],
            _ => [None, None],
        }
    };
    intervals(first)
        .into_iter()
        .flatten()
        .any(|(first_start, first_end)| {
            intervals(second)
                .into_iter()
                .flatten()
                .any(|(second_start, second_end)| {
                    first_start <= second_end && second_start <= first_end
                })
        })
}

struct SustainedCohorts {
    capacity: usize,
    remaining: Vec<u64>,
    completed: usize,
}

impl SustainedCohorts {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            capacity,
            remaining: Vec::new(),
            completed: 0,
        }
    }

    fn observe_active(&mut self, active: &[u64]) {
        if self.remaining.is_empty() && self.completed < 2 && active.len() == self.capacity {
            self.remaining.extend_from_slice(active);
        }
    }

    fn complete(&mut self, train_id: u64) {
        if let Some(index) = self.remaining.iter().position(|&id| id == train_id) {
            self.remaining.remove(index);
            if self.remaining.is_empty() {
                self.completed += 1;
            }
        }
    }

    fn verified(&self) -> bool {
        self.completed >= 2
    }
}

fn eligible(piece: &Piece) -> bool {
    const GEOMETRY_SAMPLES: usize = 32;
    const ROLL_TOLERANCE: f64 = 1e-6;
    const MAXIMUM_GENTLE_TANGENT_Z: f64 = 0.447_214;
    matches!(
        piece.kind,
        PieceKind::Track | PieceKind::Station | PieceKind::Brakes
    ) && piece.inversion.is_none()
        && matches!(
            piece.entry.slope,
            Slope::Level | Slope::UpGentle | Slope::DownGentle
        )
        && matches!(
            piece.exit.slope,
            Slope::Level | Slope::UpGentle | Slope::DownGentle
        )
        && piece.entry.bank == Bank::Level
        && piece.exit.bank == Bank::Level
        && {
            let mut uphill = false;
            let mut downhill = false;
            (0..=GEOMETRY_SAMPLES).all(|index| {
                let sample =
                    piece.sample(piece.length() * (index as f64 / GEOMETRY_SAMPLES as f64));
                uphill |= sample.tangent.z > ROLL_TOLERANCE;
                downhill |= sample.tangent.z < -ROLL_TOLERANCE;
                sample.tangent.z.abs() <= MAXIMUM_GENTLE_TANGENT_Z
                    && sample.roll.abs() <= ROLL_TOLERANCE
                    && sample.curvature.is_finite()
                    && !(uphill && downhill)
            })
        }
}

fn validate_brake_forces(
    circuit: &Circuit,
    config: PhysicsConfig,
    runs: &[BrakeRun],
) -> Result<(), &'static str> {
    const GEOMETRY_SAMPLES: usize = 32;
    for run in runs {
        for &index in &run.piece_indices {
            let piece = &circuit.pieces()[index];
            for sample_index in 0..=GEOMETRY_SAMPLES {
                let sample =
                    piece.sample(piece.length() * (sample_index as f64 / GEOMETRY_SAMPLES as f64));
                let gravity = GRAVITY * sample.tangent.z;
                if gravity.abs() > config.braking_acceleration {
                    return Err("Brake holding force is insufficient on this slope.");
                }
                if gravity > 0.0
                    && gravity + config.rolling_resistance * GRAVITY >= config.drive_acceleration
                {
                    return Err("Brake drive force is insufficient to restart uphill.");
                }
            }
        }
    }
    Ok(())
}

fn validate_train(circuit: &Circuit, train: &TrainConfiguration) -> Result<(), &'static str> {
    if train.cars == 0
        || !train.length_metres.is_finite()
        || train.length_metres <= 0.0
        || train.length_metres >= circuit.length()
    {
        Err("train length and car count must fit the circuit")
    } else {
        Ok(())
    }
}

async fn yield_step() {
    let mut yielded = false;
    poll_fn(|_| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            Poll::Pending
        }
    })
    .await;
}

pub fn brake_runs(circuit: &Circuit, train_length: f64) -> Vec<BrakeRun> {
    assert!(train_length.is_finite() && train_length > 0.0);
    let pieces = circuit.pieces();
    let eligible_pieces: Vec<_> = pieces.iter().map(eligible).collect();
    let shared_indices: Arc<[usize]> = (0..pieces.len() * 2)
        .map(|index| index % pieces.len())
        .collect();
    let mut starts = Vec::with_capacity(pieces.len());
    let mut distance = 0.0;
    for piece in pieces {
        starts.push(distance);
        distance += piece.length();
    }
    let mut runs = Vec::new();
    let mut candidates_by_end = vec![0..0; pieces.len()];
    for (end, range) in candidates_by_end.iter_mut().enumerate() {
        if !eligible_pieces[end] {
            continue;
        }
        let first = runs.len();
        let mut length = 0.0;
        for offset in 0..pieces.len() {
            let index = (end + pieces.len() - offset) % pieces.len();
            if !eligible_pieces[index] {
                break;
            }
            length += pieces[index].length();
            if length > train_length && length < circuit.length() {
                runs.push(BrakeRun {
                    start_metres: starts[index],
                    boundary_metres: (starts[end] + pieces[end].length())
                        .rem_euclid(circuit.length()),
                    length_metres: length,
                    piece_indices: BrakePieces {
                        storage: Arc::clone(&shared_indices),
                        start: index,
                        length: offset + 1,
                    },
                });
            }
        }
        *range = first..runs.len();
    }
    for start in 0..pieces.len() {
        let kind = pieces[start].kind;
        let previous = (start + pieces.len() - 1) % pieces.len();
        if !matches!(kind, PieceKind::Lift | PieceKind::Launch) || pieces[previous].kind == kind {
            continue;
        }
        let mut end = start;
        while (end + 1) % pieces.len() != start && pieces[(end + 1) % pieces.len()].kind == kind {
            end = (end + 1) % pieces.len();
        }
        for candidate_index in candidates_by_end[previous].clone() {
            let mut powered = runs[candidate_index].clone();
            powered.boundary_metres =
                (starts[end] + pieces[end].length()).rem_euclid(circuit.length());
            runs.push(powered);
        }
    }
    runs.sort_by(|a, b| a.boundary_metres.total_cmp(&b.boundary_metres));
    runs
}

/// Native lift and launch controls can hold a train as it enters the powered
/// section. The first piece is the stopping zone; the section boundary is at
/// the end of the complete contiguous powered run.
fn powered_holds(circuit: &Circuit) -> Vec<BrakeRun> {
    let pieces = circuit.pieces();
    let mut starts = Vec::with_capacity(pieces.len());
    let mut distance = 0.0;
    for piece in pieces {
        starts.push(distance);
        distance += piece.length();
    }
    let mut holds = Vec::new();
    for start in 0..pieces.len() {
        let kind = pieces[start].kind;
        let previous = (start + pieces.len() - 1) % pieces.len();
        if !matches!(kind, PieceKind::Lift | PieceKind::Launch) || pieces[previous].kind == kind {
            continue;
        }
        let mut end = start;
        while (end + 1) % pieces.len() != start && pieces[(end + 1) % pieces.len()].kind == kind {
            end = (end + 1) % pieces.len();
        }
        holds.push(BrakeRun {
            start_metres: starts[start],
            boundary_metres: (starts[end] + pieces[end].length()).rem_euclid(circuit.length()),
            length_metres: pieces[start].length(),
            piece_indices: vec![start].into(),
        });
    }
    holds
}

fn is_powered_hold(circuit: &Circuit, run: &BrakeRun) -> bool {
    run.piece_indices.first().is_some_and(|&index| {
        matches!(
            circuit.pieces()[index].kind,
            PieceKind::Lift | PieceKind::Launch
        )
    })
}

fn maximum_nonoverlapping_runs(candidates: &[&BrakeRun]) -> usize {
    // Runs sharing an end are nested, so their shortest member can always
    // replace a longer one when computing a geometry-only upper bound.
    let mut shortest_by_end: HashMap<usize, &BrakeRun> = HashMap::new();
    for &run in candidates {
        let end = *run.piece_indices.last().expect("candidate run is nonempty");
        shortest_by_end
            .entry(end)
            .and_modify(|shortest| {
                if run.piece_indices.len() < shortest.piece_indices.len() {
                    *shortest = run;
                }
            })
            .or_insert(run);
    }
    let shortest: Vec<_> = shortest_by_end.into_values().collect();
    let mut linear: Vec<_> = shortest
        .iter()
        .copied()
        .filter(|run| run.piece_indices.first().unwrap() <= run.piece_indices.last().unwrap())
        .collect();
    linear.sort_by_key(|run| run.piece_indices.last().unwrap());
    let count_linear = |excluded: Option<&BrakeRun>| {
        let mut last_end = None;
        let mut count = 0;
        for run in &linear {
            let start = run.piece_indices[0];
            if last_end.is_some_and(|end| start <= end)
                || excluded.is_some_and(|other| brake_runs_overlap(run, other))
            {
                continue;
            }
            last_end = run.piece_indices.last().copied();
            count += 1;
        }
        count
    };
    let mut maximum = count_linear(None);
    for run in shortest
        .iter()
        .copied()
        .filter(|run| run.piece_indices.first().unwrap() > run.piece_indices.last().unwrap())
    {
        maximum = maximum.max(1 + count_linear(Some(run)));
    }
    maximum
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockLayout {
    boundaries: Vec<f64>,
    runs: Vec<BrakeRun>,
    train_length: f64,
    circuit_length: f64,
    circuit: Circuit,
    train: TrainConfiguration,
    config: PhysicsConfig,
    capacity: usize,
    capacity_search_complete: bool,
    placement_search_complete: bool,
}

type PlacementFuture = Pin<Box<dyn Future<Output = Result<BlockLayout, &'static str>>>>;

/// Incremental validation for a manually selected set of brake boundaries.
pub struct BlockManualSearch {
    circuit: Circuit,
    train: TrainConfiguration,
    config: PhysicsConfig,
    boundaries: Vec<f64>,
    choices: Vec<Vec<BrakeRun>>,
    indices: Option<Vec<usize>>,
    active: Option<PlacementFuture>,
    result: Option<Result<BlockLayout, &'static str>>,
    best: Option<BlockLayout>,
    first_error: Option<&'static str>,
    first_unverified_error: Option<&'static str>,
    unverified_upper_bound: usize,
    active_upper_bound: usize,
    maximum_capacity: usize,
    work_completed: usize,
}

#[derive(Serialize, Deserialize)]
struct ManualSearchSnapshot {
    circuit: Circuit,
    train: TrainConfiguration,
    config: PhysicsConfig,
    boundaries: Vec<f64>,
    work_completed: usize,
}

impl Serialize for BlockManualSearch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ManualSearchSnapshot {
            circuit: self.circuit.clone(),
            train: self.train.clone(),
            config: self.config,
            boundaries: self.boundaries.clone(),
            work_completed: self.work_completed,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BlockManualSearch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snapshot = ManualSearchSnapshot::deserialize(deserializer)?;
        let mut search = Self::new(
            snapshot.circuit,
            snapshot.train,
            snapshot.config,
            snapshot.boundaries,
        )
        .map_err(serde::de::Error::custom)?;
        search.advance(snapshot.work_completed);
        if search.work_completed != snapshot.work_completed {
            return Err(serde::de::Error::custom(
                "manual search work exceeds completion",
            ));
        }
        Ok(search)
    }
}

impl BlockManualSearch {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"BlockManualSearch-v1");
        self.circuit.hash_into(hash);
        self.train.hash_into(hash);
        self.config.hash_into(hash);
        hash.write_len(self.choices.len());
        for choices in &self.choices {
            hash.write_len(choices.len());
            for run in choices {
                write_brake_run(hash, run);
            }
        }
        hash.write_option(self.indices.as_ref(), |hash, indices| {
            write_indices(hash, indices);
        });
        // advance() polls once per work unit; the future is a deterministic replay of these inputs.
        hash.write_bool(self.active.is_some());
        hash.write_option(self.result.as_ref(), |hash, result| match result {
            Ok(layout) => {
                hash.write_u8(0);
                layout.hash_into(hash);
            }
            Err(error) => {
                hash.write_u8(1);
                hash.write_bytes(error.as_bytes());
            }
        });
        hash.write_option(self.best.as_ref(), |hash, layout| layout.hash_into(hash));
        hash.write_option(self.first_error, |hash, error| {
            hash.write_bytes(error.as_bytes());
        });
        hash.write_option(self.first_unverified_error, |hash, error| {
            hash.write_bytes(error.as_bytes());
        });
        hash.write_len(self.unverified_upper_bound);
        hash.write_len(self.active_upper_bound);
        hash.write_len(self.maximum_capacity);
        hash.write_len(self.work_completed);
    }

    pub fn new(
        circuit: Circuit,
        train: TrainConfiguration,
        config: PhysicsConfig,
        mut boundaries: Vec<f64>,
    ) -> Result<Self, &'static str> {
        validate_train(&circuit, &train)?;
        if boundaries.iter().any(|distance| {
            !distance.is_finite() || *distance < 0.0 || *distance >= circuit.length()
        }) {
            return Err("block boundary is outside the circuit");
        }
        boundaries.sort_by(f64::total_cmp);
        if boundaries.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("duplicate block boundary");
        }
        let candidates = brake_runs(&circuit, train.length_metres);
        let mut choices = Vec::new();
        for boundary in &boundaries {
            let matching: Vec<_> = candidates
                .iter()
                .filter(|run| (run.boundary_metres - boundary).abs() < 1e-7)
                .cloned()
                .collect();
            if matching.is_empty() {
                return Err("boundary must end an eligible brake run long enough for the train");
            }
            choices.push(matching);
        }
        for powered in powered_holds(&circuit) {
            if validate_brake_forces(&circuit, config, std::slice::from_ref(&powered)).is_err() {
                continue;
            }
            if boundaries
                .iter()
                .any(|boundary| (*boundary - powered.boundary_metres).abs() < 1e-7)
            {
                continue;
            }
            let pass_through = BrakeRun {
                start_metres: powered.boundary_metres,
                boundary_metres: powered.boundary_metres,
                length_metres: 0.0,
                piece_indices: Vec::new().into(),
            };
            choices.push(vec![powered, pass_through]);
        }
        let candidates: Vec<_> = choices
            .iter()
            .flatten()
            .filter(|run| !run.piece_indices.is_empty())
            .collect();
        let maximum_capacity = maximum_nonoverlapping_runs(&candidates).max(1);
        let active: Option<PlacementFuture> = if choices.is_empty() {
            let layout = BlockLayout::assemble_runs(&circuit, &train, config, Vec::new())?;
            Some(Box::pin(BlockLayout::verify_single_train(layout)))
        } else {
            None
        };
        let indices = (!choices.is_empty()).then(|| vec![0; choices.len()]);
        Ok(Self {
            circuit,
            train,
            config,
            boundaries,
            choices,
            indices,
            active,
            result: None,
            best: None,
            first_error: None,
            first_unverified_error: None,
            unverified_upper_bound: 0,
            active_upper_bound: 1,
            maximum_capacity,
            work_completed: 0,
        })
    }

    pub fn result(&self) -> Option<&Result<BlockLayout, &'static str>> {
        self.result.as_ref()
    }
    pub fn work_completed(&self) -> usize {
        self.work_completed
    }

    fn finish_with_work_limit(mut self, maximum_steps: usize) -> Result<BlockLayout, &'static str> {
        loop {
            if let Some(result) = self.result.take() {
                return result;
            }
            if self.work_completed >= maximum_steps {
                if let Some(mut best) = self.best.take() {
                    best.placement_search_complete = false;
                    return Ok(best);
                }
            }
            self.advance(1_000);
        }
    }

    pub fn advance(&mut self, work_steps: usize) {
        let mut context = Context::from_waker(Waker::noop());
        for _ in 0..work_steps {
            if self.result.is_some() {
                break;
            }
            self.work_completed += 1;
            if let Some(task) = &mut self.active {
                match task.as_mut().poll(&mut context) {
                    Poll::Pending => continue,
                    Poll::Ready(result) => {
                        self.active = None;
                        match result {
                            Ok(layout) => {
                                if !layout.capacity_search_complete() {
                                    self.unverified_upper_bound =
                                        self.unverified_upper_bound.max(self.active_upper_bound);
                                }
                                let improves = self.best.as_ref().is_none_or(|best| {
                                    layout.capacity() > best.capacity()
                                        || (layout.capacity() == best.capacity()
                                            && layout.capacity_search_complete()
                                            && !best.capacity_search_complete())
                                });
                                if improves {
                                    self.best = Some(layout);
                                }
                                if self
                                    .best
                                    .as_ref()
                                    .is_some_and(|best| best.capacity() >= self.maximum_capacity)
                                {
                                    self.result = self.best.take().map(Ok);
                                    break;
                                }
                            }
                            Err(
                                error @ ("train capacity could not be verified within 600 seconds"
                                | "train did not reach the next block brake"),
                            ) => {
                                self.unverified_upper_bound =
                                    self.unverified_upper_bound.max(self.active_upper_bound);
                                self.first_unverified_error.get_or_insert(error);
                            }
                            Err(error) => {
                                self.first_error.get_or_insert(error);
                            }
                        }
                        continue;
                    }
                }
            }
            let Some(indices) = self.indices.take() else {
                self.result = Some(if let Some(mut best) = self.best.take() {
                    if self.unverified_upper_bound > best.capacity() {
                        best.placement_search_complete = false;
                    }
                    Ok(best)
                } else {
                    Err(self
                        .first_unverified_error
                        .or(self.first_error)
                        .unwrap_or("block brake runs cannot overlap"))
                });
                break;
            };
            let mut next = indices.clone();
            let mut position = next.len();
            while position > 0 {
                position -= 1;
                next[position] += 1;
                if next[position] < self.choices[position].len() {
                    self.indices = Some(next);
                    break;
                }
                next[position] = 0;
            }
            let runs: Vec<_> = indices
                .iter()
                .enumerate()
                .map(|(position, &index)| self.choices[position][index].clone())
                .collect();
            if runs.iter().enumerate().any(|(index, run)| {
                runs[index + 1..]
                    .iter()
                    .any(|other| brake_runs_overlap(run, other))
            }) {
                self.first_error
                    .get_or_insert("block brake runs cannot overlap");
                continue;
            }
            self.active_upper_bound = runs
                .iter()
                .filter(|run| !run.piece_indices.is_empty())
                .count()
                .max(1);
            if self.best.as_ref().is_some_and(|best| {
                best.capacity_search_complete() && best.capacity() >= self.active_upper_bound
            }) {
                continue;
            }
            self.active = Some(Box::pin(BlockLayout::from_runs_async(
                self.circuit.clone(),
                self.train.clone(),
                self.config,
                runs,
            )));
        }
    }
}

/// Incremental automatic placement. Each advancement bounds the number of
/// candidate-enumeration and physics steps performed by the caller.
pub struct BlockPlacementSearch {
    circuit: Circuit,
    train: TrainConfiguration,
    config: PhysicsConfig,
    candidates: Vec<BrakeRun>,
    search_size: usize,
    selected: Vec<usize>,
    next_indices: Vec<usize>,
    bootstrap: Vec<Vec<usize>>,
    active: Option<PlacementFuture>,
    active_brake_count: usize,
    active_baseline: bool,
    best: BlockLayout,
    maximum_brakes: usize,
    work_completed: usize,
    complete: bool,
    unverified_upper_bound: usize,
}

#[derive(Serialize, Deserialize)]
struct PlacementSearchSnapshot {
    circuit: Circuit,
    train: TrainConfiguration,
    config: PhysicsConfig,
    work_completed: usize,
}

impl Serialize for BlockPlacementSearch {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PlacementSearchSnapshot {
            circuit: self.circuit.clone(),
            train: self.train.clone(),
            config: self.config,
            work_completed: self.work_completed,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BlockPlacementSearch {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snapshot = PlacementSearchSnapshot::deserialize(deserializer)?;
        let mut search = Self::new(snapshot.circuit, snapshot.train, snapshot.config)
            .map_err(serde::de::Error::custom)?;
        search.advance(snapshot.work_completed);
        if search.work_completed != snapshot.work_completed {
            return Err(serde::de::Error::custom(
                "placement search work exceeds completion",
            ));
        }
        Ok(search)
    }
}

impl BlockPlacementSearch {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"BlockPlacementSearch-v1");
        self.circuit.hash_into(hash);
        self.train.hash_into(hash);
        self.config.hash_into(hash);
        hash.write_len(self.candidates.len());
        for run in &self.candidates {
            write_brake_run(hash, run);
        }
        hash.write_len(self.search_size);
        write_indices(hash, &self.selected);
        write_indices(hash, &self.next_indices);
        hash.write_len(self.bootstrap.len());
        for indices in &self.bootstrap {
            write_indices(hash, indices);
        }
        // The active validation future is determined by these inputs and the poll count.
        hash.write_bool(self.active.is_some());
        hash.write_len(self.active_brake_count);
        hash.write_bool(self.active_baseline);
        self.best.hash_into(hash);
        hash.write_len(self.maximum_brakes);
        hash.write_len(self.work_completed);
        hash.write_bool(self.complete);
        hash.write_len(self.unverified_upper_bound);
    }

    pub fn new(
        circuit: Circuit,
        train: TrainConfiguration,
        config: PhysicsConfig,
    ) -> Result<Self, &'static str> {
        // Validate inputs without running physics before the incremental search starts.
        let mut best = BlockLayout::unverified_baseline(&circuit, &train, config)?;
        let candidates = if config.powered_sections
            && config.drive_acceleration > 0.0
            && config.braking_acceleration > 0.0
            && config.brake_speed > 0.0
        {
            let mut candidates = brake_runs(&circuit, train.length_metres);
            candidates.retain(|run| {
                validate_brake_forces(&circuit, config, std::slice::from_ref(run)).is_ok()
            });
            candidates.extend(powered_holds(&circuit));
            candidates
                .sort_by(|left, right| left.boundary_metres.total_cmp(&right.boundary_metres));
            candidates
        } else {
            Vec::new()
        };
        let maximum_brakes = maximum_nonoverlapping_runs(&candidates.iter().collect::<Vec<_>>());
        let mut bootstrap = Vec::new();
        for count in (2..=maximum_brakes.min(3)).rev() {
            let mut chosen: Vec<usize> = Vec::new();
            for position in 0..count {
                let target = circuit.length() * (position as f64 + 0.5) / count as f64;
                let nearest = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, run)| {
                        chosen
                            .iter()
                            .all(|&other| !brake_runs_overlap(run, &candidates[other]))
                    })
                    .min_by(|(_, left), (_, right)| {
                        let distance = |run: &BrakeRun| {
                            let gap = (run.boundary_metres - target).abs();
                            gap.min(circuit.length() - gap)
                        };
                        distance(left)
                            .total_cmp(&distance(right))
                            .then_with(|| left.piece_indices.len().cmp(&right.piece_indices.len()))
                    });
                if let Some((index, _)) = nearest {
                    chosen.push(index);
                }
            }
            if chosen.len() == count {
                bootstrap.push(chosen);
            }
        }
        let mut powered_bootstrap = Vec::new();
        for hold in powered_holds(&circuit) {
            let mut following: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter(|(_, run)| {
                    !is_powered_hold(&circuit, run) && run.boundary_metres != hold.boundary_metres
                })
                .collect();
            following.sort_by(|(_, left), (_, right)| {
                (left.boundary_metres - hold.boundary_metres)
                    .rem_euclid(circuit.length())
                    .total_cmp(
                        &(right.boundary_metres - hold.boundary_metres)
                            .rem_euclid(circuit.length()),
                    )
                    .then_with(|| left.piece_indices.len().cmp(&right.piece_indices.len()))
            });
            if let Some(&(first_index, first)) = following.first() {
                let mut pairs = Vec::new();
                for &(second_index, second) in &following[1..] {
                    if !brake_runs_overlap(first, second)
                        && pairs
                            .last()
                            .is_none_or(|(_, boundary)| *boundary != second.boundary_metres)
                    {
                        pairs.push((second_index, second.boundary_metres));
                    }
                    if pairs.len() == 3 {
                        break;
                    }
                }
                for (second_index, _) in pairs.into_iter().rev() {
                    powered_bootstrap.push(vec![first_index, second_index]);
                }
            }
        }
        powered_bootstrap.extend(bootstrap);
        let bootstrap = powered_bootstrap;
        let complete = false;
        best.placement_search_complete = false;
        let baseline = Box::pin(BlockLayout::verify_single_train(best.clone()));
        Ok(Self {
            circuit,
            train,
            config,
            candidates,
            search_size: maximum_brakes,
            selected: Vec::new(),
            next_indices: vec![0],
            bootstrap,
            active: Some(baseline),
            active_brake_count: 0,
            active_baseline: true,
            best,
            maximum_brakes,
            work_completed: 0,
            complete,
            unverified_upper_bound: 0,
        })
    }

    pub fn best(&self) -> &BlockLayout {
        &self.best
    }
    pub fn is_complete(&self) -> bool {
        self.complete
    }
    pub fn work_completed(&self) -> usize {
        self.work_completed
    }

    pub fn advance(&mut self, work_steps: usize) {
        let mut context = Context::from_waker(Waker::noop());
        for _ in 0..work_steps {
            if self.complete {
                break;
            }
            self.work_completed += 1;
            if let Some(task) = &mut self.active {
                match task.as_mut().poll(&mut context) {
                    Poll::Pending => continue,
                    Poll::Ready(result) => {
                        self.active = None;
                        match result {
                            Ok(mut layout) => {
                                if self.active_baseline {
                                    layout.placement_search_complete = false;
                                    self.best = layout;
                                    self.active_baseline = false;
                                    continue;
                                }
                                if !layout.capacity_search_complete() {
                                    self.unverified_upper_bound =
                                        self.unverified_upper_bound.max(self.active_brake_count);
                                }
                                let new_brakes = layout
                                    .runs
                                    .iter()
                                    .filter(|run| !run.piece_indices.is_empty())
                                    .count();
                                let current_brakes = self
                                    .best
                                    .runs
                                    .iter()
                                    .filter(|run| !run.piece_indices.is_empty())
                                    .count();
                                if layout.capacity() > self.best.capacity()
                                    || (layout.capacity() == self.best.capacity()
                                        && new_brakes > current_brakes)
                                    || (layout.capacity() == self.best.capacity()
                                        && new_brakes == current_brakes
                                        && layout.capacity_search_complete()
                                        && !self.best.capacity_search_complete())
                                {
                                    layout.placement_search_complete = false;
                                    self.best = layout;
                                }
                                if self.best.capacity() >= self.maximum_brakes {
                                    self.complete = true;
                                    self.best.placement_search_complete = true;
                                }
                            }
                            Err("train capacity could not be verified within 600 seconds")
                            | Err("train did not reach the next block brake") => {
                                self.unverified_upper_bound =
                                    self.unverified_upper_bound.max(self.active_brake_count)
                            }
                            Err(_) => {}
                        }
                        continue;
                    }
                }
            }
            let result = if let Some(indices) = self.bootstrap.pop() {
                Some(Some(indices))
            } else {
                self.next_search_node()
            };
            let Some(result) = result else {
                self.complete = true;
                self.best.placement_search_complete =
                    self.unverified_upper_bound <= self.best.capacity();
                break;
            };
            let Some(indices) = result else {
                continue;
            };
            self.active_brake_count = indices.len();
            let runs = indices
                .into_iter()
                .map(|index| self.candidates[index].clone())
                .collect();
            self.active = Some(Box::pin(BlockLayout::from_runs_async(
                self.circuit.clone(),
                self.train.clone(),
                self.config,
                runs,
            )));
        }
    }

    fn next_search_node(&mut self) -> Option<Option<Vec<usize>>> {
        if self.search_size < self.best.capacity() || self.search_size == 0 {
            return None;
        }
        let next = *self.next_indices.last().expect("search stack has a root");
        let needed = self.search_size - self.selected.len();
        let available: Vec<_> = self.candidates[next.min(self.candidates.len())..]
            .iter()
            .filter(|run| {
                self.selected
                    .iter()
                    .all(|&other| !brake_runs_overlap(run, &self.candidates[other]))
            })
            .collect();
        if maximum_nonoverlapping_runs(&available) < needed {
            self.next_indices.pop();
            if self.selected.pop().is_some() {
                *self.next_indices.last_mut().expect("parent search node") += 1;
            } else {
                self.search_size -= 1;
                self.next_indices.push(0);
            }
            return Some(None);
        }
        let run = &self.candidates[next];
        if self
            .selected
            .iter()
            .any(|&other| brake_runs_overlap(run, &self.candidates[other]))
        {
            *self.next_indices.last_mut().expect("search node") += 1;
            return Some(None);
        }
        self.selected.push(next);
        if self.selected.len() == self.search_size {
            let complete = self.selected.clone();
            self.selected.pop();
            *self.next_indices.last_mut().expect("search node") += 1;
            Some(Some(complete))
        } else {
            self.next_indices.push(next + 1);
            Some(None)
        }
    }
}

impl BlockLayout {
    pub(crate) fn hash_into(&self, hash: &mut StateHasher) {
        hash.write_tag(b"BlockLayout-v1");
        hash.write_len(self.boundaries.len());
        for &boundary in &self.boundaries {
            hash.write_f64(boundary);
        }
        hash.write_len(self.runs.len());
        for run in &self.runs {
            hash.write_f64(run.start_metres);
            hash.write_f64(run.boundary_metres);
            hash.write_f64(run.length_metres);
            hash.write_len(run.piece_indices.len());
            for &piece_index in &run.piece_indices {
                hash.write_len(piece_index);
            }
        }
        hash.write_f64(self.train_length);
        hash.write_f64(self.circuit_length);
        self.circuit.hash_into(hash);
        self.train.hash_into(hash);
        self.config.hash_into(hash);
        hash.write_len(self.capacity);
        hash.write_bool(self.capacity_search_complete);
        hash.write_bool(self.placement_search_complete);
    }

    /// Permit a single train while its first complete circuit is still unverified.
    pub fn unverified_baseline(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Result<Self, &'static str> {
        validate_train(circuit, train)?;
        let mut layout = Self::assemble_runs(circuit, train, config, Vec::new())?;
        layout.placement_search_complete = false;
        Ok(layout)
    }

    pub fn automatic(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Result<Self, &'static str> {
        let mut search = BlockPlacementSearch::new(circuit.clone(), train.clone(), config)?;
        while !search.is_complete() {
            search.advance(10_000);
        }
        Ok(search.best().clone())
    }

    #[cfg(test)]
    fn automatic_with_budget(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        maximum_visits: usize,
        maximum_simulations: usize,
        maximum_steps: usize,
    ) -> Result<Self, &'static str> {
        let mut search = BlockPlacementSearch::new(circuit.clone(), train.clone(), config)?;
        search.advance(maximum_visits.min(maximum_simulations).min(maximum_steps));
        Ok(search.best().clone())
    }

    pub fn new(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        boundaries: Vec<f64>,
    ) -> Result<Self, &'static str> {
        const MAXIMUM_SEARCH_STEPS: usize = 100_000;
        BlockManualSearch::new(circuit.clone(), train.clone(), config, boundaries)?
            .finish_with_work_limit(MAXIMUM_SEARCH_STEPS)
    }

    #[cfg(test)]
    fn from_runs(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        runs: Vec<BrakeRun>,
    ) -> Result<Self, &'static str> {
        let mut remaining_steps = usize::MAX;
        Self::from_runs_with_budget(circuit, train, config, runs, &mut remaining_steps)
    }

    #[cfg(test)]
    fn from_runs_with_budget(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        runs: Vec<BrakeRun>,
        remaining_steps: &mut usize,
    ) -> Result<Self, &'static str> {
        let mut task = Box::pin(Self::from_runs_async(
            circuit.clone(),
            train.clone(),
            config,
            runs,
        ));
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if *remaining_steps == 0 {
                return Err("block placement physics work budget exhausted");
            }
            *remaining_steps -= 1;
            if let Poll::Ready(result) = task.as_mut().poll(&mut context) {
                return result;
            }
        }
    }

    async fn from_runs_async(
        circuit: Circuit,
        train: TrainConfiguration,
        config: PhysicsConfig,
        runs: Vec<BrakeRun>,
    ) -> Result<Self, &'static str> {
        let mut layout = Self::assemble_runs(&circuit, &train, config, runs)?;
        if layout.runs.iter().all(|run| run.piece_indices.is_empty()) {
            return Self::verify_single_train(layout).await;
        }
        if layout.runs.iter().any(|run| !run.piece_indices.is_empty()) {
            layout.validate_traversal(&circuit, &train, config).await?;
            layout.validate_unblocked_lap().await?;
        }
        if layout.capacity > 1 {
            let (capacity, complete) = layout.measure_capacity().await?;
            layout.capacity = capacity;
            layout.capacity_search_complete = complete;
        }
        Ok(layout)
    }

    async fn validate_unblocked_lap(&self) -> Result<(), &'static str> {
        const MAXIMUM_SECONDS: f64 = 600.0;
        let mut operation = BlockOperation::new_without_reports(self.clone());
        operation.dispatch();
        for _ in 0..(MAXIMUM_SECONDS / TIME_STEP) as usize {
            yield_step().await;
            if let Some((_, outcome)) = operation.step(&[]).into_iter().next() {
                return if outcome.recovered {
                    Err("A train cannot complete the circuit without being recovered.")
                } else {
                    Ok(())
                };
            }
        }
        Err("The train could not complete an unblocked circuit within 600 seconds.")
    }

    async fn verify_single_train(mut layout: Self) -> Result<Self, &'static str> {
        const MAXIMUM_SECONDS: f64 = 600.0;
        let mut operation = BlockOperation::new_without_reports(layout.clone());
        operation.dispatch();
        for _ in 0..(MAXIMUM_SECONDS / TIME_STEP) as usize {
            yield_step().await;
            if let Some((_, outcome)) = operation.step(&[]).into_iter().next() {
                layout.capacity = usize::from(!outcome.recovered);
                layout.capacity_search_complete = true;
                return Ok(layout);
            }
        }
        layout.capacity_search_complete = false;
        Ok(layout)
    }

    fn assemble_runs(
        circuit: &Circuit,
        train: &TrainConfiguration,
        config: PhysicsConfig,
        mut runs: Vec<BrakeRun>,
    ) -> Result<Self, &'static str> {
        let pieces = circuit.pieces();
        let mut distance = 0.0;
        for (index, piece) in pieces.iter().enumerate() {
            distance += piece.length();
            let next = &pieces[(index + 1) % pieces.len()];
            if (matches!(
                piece.kind,
                PieceKind::Station | PieceKind::Lift | PieceKind::Launch | PieceKind::Brakes
            ) && piece.kind != next.kind)
                || (piece.kind != next.kind
                    && matches!(
                        next.kind,
                        PieceKind::Station | PieceKind::Lift | PieceKind::Launch
                    ))
            {
                let boundary = distance.rem_euclid(circuit.length());
                if !runs
                    .iter()
                    .any(|run| (run.boundary_metres - boundary).abs() < 1e-7)
                {
                    runs.push(BrakeRun {
                        start_metres: boundary,
                        boundary_metres: boundary,
                        length_metres: 0.0,
                        piece_indices: Vec::new().into(),
                    });
                }
            }
        }
        runs.sort_by(|a, b| a.boundary_metres.total_cmp(&b.boundary_metres));
        if runs
            .windows(2)
            .any(|pair| (pair[0].boundary_metres - pair[1].boundary_metres).abs() < 1e-7)
        {
            return Err("block boundaries cannot overlap");
        }
        let boundaries = runs.iter().map(|run| run.boundary_metres).collect();
        for (index, run) in runs.iter().enumerate() {
            if runs[index + 1..]
                .iter()
                .any(|other| brake_runs_overlap(run, other))
            {
                return Err("block brake runs cannot overlap");
            }
        }
        if runs.iter().any(|run| !run.piece_indices.is_empty())
            && (!config.powered_sections
                || config.drive_acceleration <= 0.0
                || config.braking_acceleration <= 0.0
                || config.brake_speed <= 0.0)
        {
            return Err("block brakes require powered drive and braking");
        }
        validate_brake_forces(circuit, config, &runs)?;
        let brake_count = runs
            .iter()
            .filter(|run| !run.piece_indices.is_empty())
            .count();
        let provisional_capacity = brake_count.max(1);
        Ok(Self {
            boundaries,
            runs,
            train_length: train.length_metres,
            circuit_length: circuit.length(),
            circuit: circuit.clone(),
            train: train.clone(),
            config,
            capacity: provisional_capacity,
            capacity_search_complete: brake_count > 0,
            placement_search_complete: true,
        })
    }

    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }
    pub fn train(&self) -> &TrainConfiguration {
        &self.train
    }
    pub fn config(&self) -> PhysicsConfig {
        self.config
    }

    pub fn boundaries(&self) -> &[f64] {
        &self.boundaries
    }
    pub fn brake_pieces(&self, index: usize) -> &[usize] {
        if is_powered_hold(&self.circuit, &self.runs[index]) {
            &[]
        } else {
            &self.runs[index].piece_indices
        }
    }
    /// Powered track that can stop a train at the start of this section.
    pub fn powered_hold_pieces(&self, index: usize) -> &[usize] {
        if is_powered_hold(&self.circuit, &self.runs[index]) {
            &self.runs[index].piece_indices
        } else {
            &[]
        }
    }
    pub(crate) fn hold_pieces(&self, index: usize) -> &[usize] {
        &self.runs[index].piece_indices
    }
    pub fn brake_start(&self, index: usize) -> f64 {
        self.runs[index].start_metres
    }
    pub fn should_hold(&self, index: usize, head_distance: f64) -> bool {
        let distance =
            (head_distance - self.runs[index].start_metres).rem_euclid(self.circuit_length);
        let minimum_distance = if is_powered_hold(&self.circuit, &self.runs[index]) {
            0.0
        } else {
            self.train_length
        };
        distance >= minimum_distance && distance < self.runs[index].length_metres
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn capacity_search_complete(&self) -> bool {
        self.capacity_search_complete
    }

    pub fn placement_search_complete(&self) -> bool {
        self.placement_search_complete
    }

    async fn measure_capacity(&self) -> Result<(usize, bool), &'static str> {
        const MAXIMUM_SECONDS: f64 = 600.0;
        let mut higher_capacity_unverified = false;
        for capacity in (2..=self.capacity).rev() {
            let mut trial = self.clone();
            trial.capacity = capacity;
            let mut operation = BlockOperation::new_without_reports(trial);
            let mut maximum_active = 0;
            let mut cohorts = SustainedCohorts::new(capacity);
            let mut deadlocked = false;
            let mut unsafe_trial = false;
            for _ in 0..(MAXIMUM_SECONDS / TIME_STEP) as usize {
                while operation.can_dispatch() {
                    operation.dispatch();
                }
                let active: Vec<_> = operation.trains().map(|(id, _)| id).collect();
                maximum_active = maximum_active.max(active.len());
                cohorts.observe_active(&active);
                yield_step().await;
                let outcomes = operation.step(&[]);
                if operation.interlock_triggered() {
                    higher_capacity_unverified = true;
                    unsafe_trial = true;
                    break;
                }
                for (id, outcome) in outcomes {
                    if outcome.recovered {
                        if maximum_active == 1 {
                            return Err("a train recovered during block capacity validation");
                        }
                        higher_capacity_unverified = true;
                        unsafe_trial = true;
                        break;
                    }
                    cohorts.complete(id);
                    if maximum_active == 1 {
                        // The only train returned before any other could leave the
                        // station. Its next departure has exactly the same state.
                        return Ok((1, !higher_capacity_unverified));
                    }
                }
                if unsafe_trial {
                    break;
                }
                if cohorts.verified() {
                    return Ok((maximum_active, !higher_capacity_unverified));
                }
                if operation.is_deadlocked() {
                    deadlocked = true;
                    break;
                }
            }
            if !deadlocked && !unsafe_trial {
                higher_capacity_unverified = true;
            }
        }
        Ok((1, !higher_capacity_unverified))
    }

    async fn validate_traversal(
        &self,
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> Result<(), &'static str> {
        let mut station_distance = 0.0;
        for piece in circuit.pieces() {
            if piece.kind == PieceKind::Station {
                station_distance += piece.length() / 2.0;
                break;
            }
            station_distance += piece.length();
        }
        let station_section = self
            .boundaries
            .partition_point(|boundary| *boundary <= station_distance);
        let station_section = if station_section == 0 {
            self.runs.len() - 1
        } else {
            station_section - 1
        };
        let stopping: Vec<usize> = (1..=self.runs.len())
            .map(|offset| (station_section + offset) % self.runs.len())
            .filter(|&index| !self.runs[index].piece_indices.is_empty())
            .collect();
        let station_train = Train::new(circuit, configuration, config, station_distance, 0.0);
        let mut train = station_train.clone();
        let mut stopped_trains = Vec::with_capacity(stopping.len());
        for leg in 0..=stopping.len() * 2 {
            let target_index = stopping[(leg + 1) % stopping.len()];
            self.validate_approach(circuit, configuration, &mut train, target_index)
                .await?;
            if leg >= stopping.len() && stopped_trains.len() < stopping.len() {
                stopped_trains.push((target_index, train.clone()));
            }
        }
        // A train may pass any number of clear brakes before it encounters an
        // occupied section. Check those approaches as well as stopping at each brake.
        for (start_index, start_train) in stopped_trains {
            for &target_index in &stopping {
                if target_index == start_index {
                    continue;
                }
                let mut train = start_train.clone();
                self.validate_approach(circuit, configuration, &mut train, target_index)
                    .await?;
            }
        }
        for &target_index in &stopping {
            let mut train = station_train.clone();
            self.validate_approach(circuit, configuration, &mut train, target_index)
                .await?;
        }
        Ok(())
    }

    async fn validate_approach(
        &self,
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        train: &mut Train,
        target_index: usize,
    ) -> Result<(), &'static str> {
        let target = &self.runs[target_index];
        let start = train.distance();
        let target_start = start + (target.start_metres - start).rem_euclid(circuit.length());
        let target_start = if target_start <= start + 1e-6 {
            target_start + circuit.length()
        } else {
            target_start
        };
        let target_end = target_start + target.length_metres;
        let mut reached_target = false;
        for _ in 0..(300.0 / TIME_STEP) as usize {
            let distance = train.distance();
            let holding_distance = if is_powered_hold(circuit, target) {
                0.0
            } else {
                configuration.length_metres
            };
            let holding = distance >= target_start + holding_distance;
            let position = distance.rem_euclid(circuit.length());
            let section = self
                .boundaries
                .partition_point(|boundary| *boundary <= position);
            let section = if section == 0 {
                self.runs.len() - 1
            } else {
                section - 1
            };
            let next = (1..=self.runs.len())
                .map(|offset| (section + offset) % self.runs.len())
                .find(|&index| !self.runs[index].piece_indices.is_empty())
                .expect("stopping brake exists");
            yield_step().await;
            train.step_block_brake_without_readings(
                circuit,
                &self.runs[next].piece_indices,
                holding,
            );
            if train.distance() < distance - 1e-6 || train.distance() > target_end {
                return Err("train cannot safely reach and stop on the next block brake");
            }
            if holding && train.speed() == 0.0 {
                if !is_powered_hold(circuit, target)
                    && train.distance() - target_start < configuration.length_metres
                {
                    return Err("train stops before it fits on the block brake");
                }
                reached_target = true;
                break;
            }
            if !train.notes().is_empty() {
                return Err("train stalled or rolled back between block brakes");
            }
        }
        if !reached_target {
            return Err("train did not reach the next block brake");
        }
        Ok(())
    }
}
