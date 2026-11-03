//! Placement and validation of powered block-brake runs.
use crate::physics::{BlockOperation, Circuit, PhysicsConfig, TIME_STEP, Train};
use crate::track::{
    Bank, HorizontalCurvature, Piece, PieceId, PieceKind, Slope, TrainConfiguration,
    VerticalCurvature,
};
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct BrakeRun {
    pub start_metres: f64,
    pub boundary_metres: f64,
    pub length_metres: f64,
    pub piece_indices: Vec<usize>,
}

fn eligible(piece: &Piece) -> bool {
    (piece.id == PieceId::Straight || matches!(piece.kind, PieceKind::Station | PieceKind::Brakes))
        && piece.entry.slope == Slope::Level
        && piece.exit.slope == Slope::Level
        && piece.entry.bank == Bank::Level
        && piece.exit.bank == Bank::Level
        && piece.entry.horizontal_curvature == HorizontalCurvature::Straight
        && piece.exit.horizontal_curvature == HorizontalCurvature::Straight
        && piece.entry.vertical_curvature == VerticalCurvature::Straight
        && piece.exit.vertical_curvature == VerticalCurvature::Straight
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
    let mut starts = Vec::with_capacity(pieces.len());
    let mut distance = 0.0;
    for piece in pieces {
        starts.push(distance);
        distance += piece.length();
    }
    let mut runs = Vec::new();
    for end in 0..pieces.len() {
        if !eligible(&pieces[end]) {
            continue;
        }
        let mut indices = Vec::new();
        let mut length = 0.0;
        for offset in 0..pieces.len() {
            let index = (end + pieces.len() - offset) % pieces.len();
            if !eligible(&pieces[index]) {
                break;
            }
            indices.push(index);
            length += pieces[index].length();
            if length > train_length && length < circuit.length() {
                let mut ordered_indices = indices.clone();
                ordered_indices.reverse();
                runs.push(BrakeRun {
                    start_metres: starts[ordered_indices[0]],
                    boundary_metres: (starts[end] + pieces[end].length())
                        .rem_euclid(circuit.length()),
                    length_metres: length,
                    piece_indices: ordered_indices,
                });
            }
        }
    }
    let plain_runs = runs.clone();
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
        for brake in plain_runs
            .iter()
            .filter(|run| run.piece_indices.last() == Some(&previous))
        {
            let mut powered = brake.clone();
            powered.boundary_metres =
                (starts[end] + pieces[end].length()).rem_euclid(circuit.length());
            runs.push(powered);
        }
    }
    runs.sort_by(|a, b| a.boundary_metres.total_cmp(&b.boundary_metres));
    runs
}

fn maximum_nonoverlapping_runs(candidates: &[&BrakeRun]) -> usize {
    let mut linear: Vec<_> = candidates
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
                || excluded.is_some_and(|other| {
                    run.piece_indices
                        .iter()
                        .any(|piece| other.piece_indices.contains(piece))
                })
            {
                continue;
            }
            last_end = run.piece_indices.last().copied();
            count += 1;
        }
        count
    };
    let mut maximum = count_linear(None);
    for run in candidates
        .iter()
        .copied()
        .filter(|run| run.piece_indices.first().unwrap() > run.piece_indices.last().unwrap())
    {
        maximum = maximum.max(1 + count_linear(Some(run)));
    }
    maximum
}

#[derive(Clone, Debug)]
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
    choices: Vec<Vec<BrakeRun>>,
    indices: Option<Vec<usize>>,
    active: Option<PlacementFuture>,
    result: Option<Result<BlockLayout, &'static str>>,
    first_error: Option<&'static str>,
    capacity_unverified: bool,
    work_completed: usize,
}

impl BlockManualSearch {
    pub fn new(
        circuit: Circuit,
        train: TrainConfiguration,
        config: PhysicsConfig,
        mut boundaries: Vec<f64>,
    ) -> Result<Self, &'static str> {
        if train.cars == 0
            || !train.length_metres.is_finite()
            || train.length_metres <= 0.0
            || train.length_metres >= circuit.length()
        {
            return Err("train length and car count must fit the circuit");
        }
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
        let result = if choices.is_empty() {
            Some(BlockLayout::assemble_runs(
                &circuit,
                &train,
                config,
                Vec::new(),
            ))
        } else {
            None
        };
        let indices = (!choices.is_empty()).then(|| vec![0; choices.len()]);
        Ok(Self {
            circuit,
            train,
            config,
            choices,
            indices,
            active: None,
            result,
            first_error: None,
            capacity_unverified: false,
            work_completed: 0,
        })
    }

    pub fn result(&self) -> Option<&Result<BlockLayout, &'static str>> {
        self.result.as_ref()
    }
    pub fn work_completed(&self) -> usize {
        self.work_completed
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
                                self.result = Some(Ok(layout));
                                break;
                            }
                            Err("train capacity could not be verified within 600 seconds") => {
                                self.capacity_unverified = true
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
                self.result = Some(Err(if self.capacity_unverified {
                    "train capacity could not be verified within 600 seconds"
                } else {
                    self.first_error
                        .unwrap_or("block brake runs cannot overlap")
                }));
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
                runs[index + 1..].iter().any(|other| {
                    run.piece_indices
                        .iter()
                        .any(|piece| other.piece_indices.contains(piece))
                })
            }) {
                self.first_error
                    .get_or_insert("block brake runs cannot overlap");
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
    best: BlockLayout,
    maximum_brakes: usize,
    work_completed: usize,
    complete: bool,
    unverified_upper_bound: usize,
}

impl BlockPlacementSearch {
    pub fn new(
        circuit: Circuit,
        train: TrainConfiguration,
        config: PhysicsConfig,
    ) -> Result<Self, &'static str> {
        let mut best = BlockLayout::new(&circuit, &train, config, Vec::new())?;
        let candidates = if config.powered_sections
            && config.drive_acceleration > 0.0
            && config.braking_acceleration > 0.0
            && config.brake_speed > 0.0
        {
            brake_runs(&circuit, train.length_metres)
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
                        chosen.iter().all(|&other| {
                            run.piece_indices
                                .iter()
                                .all(|piece| !candidates[other].piece_indices.contains(piece))
                        })
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
        let complete = maximum_brakes == 0;
        best.placement_search_complete = complete;
        Ok(Self {
            circuit,
            train,
            config,
            candidates,
            search_size: maximum_brakes,
            selected: Vec::new(),
            next_indices: vec![0],
            bootstrap,
            active: None,
            active_brake_count: 0,
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
                self.selected.iter().all(|&other| {
                    run.piece_indices
                        .iter()
                        .all(|piece| !self.candidates[other].piece_indices.contains(piece))
                })
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
        if self.selected.iter().any(|&other| {
            run.piece_indices
                .iter()
                .any(|piece| self.candidates[other].piece_indices.contains(piece))
        }) {
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
        let mut search =
            BlockManualSearch::new(circuit.clone(), train.clone(), config, boundaries)?;
        while search.result().is_none() {
            search.advance(10_000);
        }
        search.result().expect("completed manual search").clone()
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
        if layout.runs.iter().any(|run| !run.piece_indices.is_empty()) {
            layout.validate_traversal(&circuit, &train, config).await?;
        }
        if layout.capacity > 1 {
            let (capacity, complete) = layout.measure_capacity().await?;
            layout.capacity = capacity;
            layout.capacity_search_complete = complete;
        }
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
                || (piece.kind != PieceKind::Station && next.kind == PieceKind::Station)
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
                        piece_indices: Vec::new(),
                    });
                }
            }
        }
        runs.sort_by(|a, b| a.boundary_metres.total_cmp(&b.boundary_metres));
        let boundaries = runs.iter().map(|run| run.boundary_metres).collect();
        for (index, run) in runs.iter().enumerate() {
            if runs[index + 1..].iter().any(|other| {
                run.piece_indices
                    .iter()
                    .any(|piece| other.piece_indices.contains(piece))
            }) {
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
            capacity_search_complete: true,
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
        &self.runs[index].piece_indices
    }
    pub fn brake_start(&self, index: usize) -> f64 {
        self.runs[index].start_metres
    }
    pub fn should_hold(&self, index: usize, head_distance: f64) -> bool {
        let distance =
            (head_distance - self.runs[index].start_metres).rem_euclid(self.circuit_length);
        distance >= self.train_length && distance < self.runs[index].length_metres
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
            let mut peak_cohort = Vec::new();
            let mut deadlocked = false;
            for _ in 0..(MAXIMUM_SECONDS / TIME_STEP) as usize {
                while operation.can_dispatch() {
                    operation.dispatch();
                }
                let active: Vec<_> = operation.trains().map(|(id, _)| id).collect();
                if active.len() > maximum_active {
                    maximum_active = active.len();
                    peak_cohort = active;
                }
                yield_step().await;
                for (id, outcome) in operation.step(&[]) {
                    if outcome.recovered {
                        return Err("a train recovered during block capacity validation");
                    }
                    peak_cohort.retain(|&train_id| train_id != id);
                    if maximum_active == 1 {
                        // The only train returned before any other could leave the
                        // station. Its next departure has exactly the same state.
                        return Ok((1, !higher_capacity_unverified));
                    }
                }
                if operation.interlock_triggered() {
                    return Err(
                        "a train needed an emergency stop during block capacity validation",
                    );
                }
                if maximum_active == capacity && peak_cohort.is_empty() {
                    return Ok((maximum_active, !higher_capacity_unverified));
                }
                if operation.is_deadlocked() {
                    deadlocked = true;
                    break;
                }
            }
            if !deadlocked {
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
            let holding = distance >= target_start + configuration.length_metres;
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
                if train.distance() - target_start < configuration.length_metres {
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
