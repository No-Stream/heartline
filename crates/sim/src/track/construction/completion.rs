//! Finish-the-circuit search: short piece sequences from the open end back into the station.
//!
//! A beam search over plain track pieces and brakes finds sequences that reach the station
//! entry with valid placement. Each closing sequence is then simulated with the builder's
//! construction-test physics, resuming from the preview snapshot at the open end, and kept
//! only when the train reaches the station, every new piece's worst-car forces stay
//! `FORCE_HEADROOM_G` inside the model's comfortable limits, and every car enters every rated
//! new piece with a non-negative rated-speed margin. See
//! docs/superpowers/plans/2026-11-20-finish-circuit.md.

use super::*;
use crate::physics::{
    ForcePeakValues, GRAVITY, PhysicsConfig, PreviewCache, PreviewStatus, TestSample, TrackPreview,
};
use crate::track::rated_speed_margin;

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;

const MAX_DISTANCE_METRES: f64 = 120.0;
const MAX_PIECES: usize = 6;
const BEAM_WIDTH: usize = 12;
const CANDIDATES_PER_NODE: usize = 16;
const MAX_COMPLETIONS: usize = 5;
/// Orders pieces whose rated window misses the estimated entry speed after the rest.
const OUTSIDE_RATED_WINDOW_ORDER_PENALTY: f64 = 50.0;
/// The same margin inside each model force limit that `tools/design check` requires.
pub const FORCE_HEADROOM_G: f64 = 0.25;

/// The physics the builder's force preview uses: default physics with a 5 m/s lift.
pub fn construction_preview_physics() -> PhysicsConfig {
    PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    }
}

/// A validated way back to the station. Its serialized layout predates the physics report,
/// which is skipped so park saves stay compatible; loading a park restarts the search.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Completion {
    pub pieces: Vec<PieceId>,
    pub geometry: Vec<Piece>,
    /// Rank among the completions found by one search, starting at 1.
    pub score: f64,
    construction_hash: u64,
    #[serde(skip)]
    pub physics: CompletionPhysics,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionPhysics {
    /// Front-car speed where the track meets the station.
    pub arrival_speed_mps: f64,
    /// Kinetic energy the new brake pieces remove from the loaded train.
    pub brake_energy_kj: f64,
    /// Smallest remaining margin to `limit - FORCE_HEADROOM_G` across new pieces and axes.
    pub minimum_force_headroom_g: f64,
    /// Smallest rated-speed margin across new rated pieces and cars, if any are rated.
    pub minimum_rated_margin_mps: Option<f64>,
    pub pieces: Vec<CompletionPieceReport>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionPieceReport {
    pub id: PieceId,
    pub entry_speed_mps: f64,
    pub exit_speed_mps: f64,
    pub worst_car_peaks: ForcePeakValues,
    pub force_headroom_g: f64,
    pub rated_window_mps: Option<(f64, f64)>,
    pub rated_margin_mps: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SearchNode {
    construction: Construction,
    pieces: Vec<PieceId>,
    rank: f64,
    brake_count: usize,
    rated_piece_count: usize,
    /// Energy estimate of the squared front-car speed at the node's end.
    speed_squared_estimate: f64,
}

impl Construction {
    pub(super) fn copy_for_completion(&self) -> Self {
        Self {
            start: self.start,
            model: self.model.clone(),
            pieces: self.pieces.clone(),
            ids: self.ids.clone(),
            disconnected: self
                .disconnected
                .iter()
                .map(|segment| TrackSegment {
                    ids: segment.ids.clone(),
                    pieces: segment.pieces.clone(),
                })
                .collect(),
            next_id: self.next_id,
            closed: self.closed,
            chain: self.chain,
            selected_hill_variant: self.selected_hill_variant,
            colors: self.colors,
            validation_cache: RefCell::new(None),
            choices_cache: RefCell::new(None),
            element_choices_cache: RefCell::new(None),
            force_preview_cache: RefCell::new(crate::physics::PreviewCache::default()),
            shaped_solve_cache: RefCell::new(None),
            state_hash_memo: Cell::new(self.state_hash_memo.get()),
            #[cfg(test)]
            support_cache_misses: Cell::new(0),
            #[cfg(test)]
            validation_cache_rebuilds: Cell::new(0),
        }
    }

    pub(super) fn copy_for_element_choice(&self) -> Self {
        #[cfg(test)]
        super::ELEMENT_CHOICE_CONSTRUCTION_COPY_COUNT.with(|count| count.set(count.get() + 1));
        let mut copy = self.copy_for_completion();
        copy.validation_cache = RefCell::new(self.validation_cache.borrow().clone());
        copy.choices_cache = RefCell::new(self.choices_cache.borrow().clone());
        copy
    }

    /// Search copies keep the validation cache so each append validates only the new piece.
    fn copy_for_completion_search(&self) -> Self {
        let copy = self.copy_for_completion();
        *copy.validation_cache.borrow_mut() = self.validation_cache.borrow().clone();
        copy
    }

    pub(super) fn append_sequence(
        &self,
        pieces: &[PieceId],
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<Self, ConstructionError> {
        let mut updated = self.copy_for_completion();
        for &id in pieces {
            updated.append(id, world, obstacles)?;
        }
        Ok(updated)
    }

    /// Starts a finish search, or explains why none can start. The builder's force preview of
    /// the open track is computed (or reused from the cache) here.
    pub fn completion_search(&self) -> Result<CompletionSearch, String> {
        if self.closed {
            return Err("The coaster circuit is already closed.".into());
        }
        if self
            .pieces
            .first()
            .is_none_or(|piece| piece.kind != PieceKind::Station)
        {
            return Err("Place a station before finishing the coaster.".into());
        }
        if !self.disconnected.is_empty() {
            return Err("Reconnect the disconnected track before finishing the coaster.".into());
        }
        if self.end() == self.start {
            return Err(
                "The track already reaches the station entry; close the circuit instead.".into(),
            );
        }
        let remaining = distance(self.end(), self.start);
        if remaining > MAX_DISTANCE_METRES {
            return Err(format!(
                "The track end is {remaining:.0} m from the station entry; bring it within {MAX_DISTANCE_METRES:.0} m to search for a finish."
            ));
        }
        let config = construction_preview_physics();
        let preview = self
            .force_preview(&self.pieces, config)
            .expect("a track with a station has a preview");
        let end_speed_mps = match preview.status {
            PreviewStatus::ReachedEnd => preview
                .end_speed_mps
                .expect("a preview that reached the end has an end speed"),
            PreviewStatus::Stalled => {
                return Err(
                    "The train stops before the end of the track, so no finish can bring it back to the station. Add height or a lift before the end."
                        .into(),
                );
            }
            PreviewStatus::TimedOut => {
                return Err(
                    "The train does not reach the end of the track within the preview time, so no finish can be checked."
                        .into(),
                );
            }
        };
        let preview_cache = self
            .force_preview_cache
            .borrow()
            .fork_at_end(self.pieces.len())
            .expect("a preview that reached the end has a snapshot there");
        Ok(self.search_from(end_speed_mps, preview_cache, true))
    }

    /// Geometry coverage for unpowered test layouts: every closing route counts, unsimulated.
    #[cfg(test)]
    pub(super) fn completion_search_ignoring_physics(&self) -> CompletionSearch {
        self.search_from(f64::INFINITY, PreviewCache::default(), false)
    }

    fn search_from(
        &self,
        end_speed_mps: f64,
        preview_cache: PreviewCache,
        physics_checked: bool,
    ) -> CompletionSearch {
        let mut construction = self.copy_for_completion_search();
        construction.set_chain(false);
        CompletionSearch {
            physics_checked,
            target: self.start,
            construction_hash: self.state_hash(),
            original_length: self.pieces.len(),
            end_state: self.end(),
            end_speed_mps,
            preview_cache,
            depth: 0,
            beam: vec![SearchNode {
                construction,
                pieces: Vec::new(),
                rank: 0.0,
                brake_count: 0,
                rated_piece_count: 0,
                speed_squared_estimate: end_speed_mps.powi(2),
            }],
            next: Vec::new(),
            current: None,
            candidates: Vec::new(),
            closing: Vec::new(),
            found: Vec::new(),
            rejections: Rejections::default(),
        }
    }

    /// Runs the whole finish search; returns ranked completions or why there are none.
    pub fn find_completions(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<Vec<Completion>, String> {
        let mut search = self.completion_search()?;
        loop {
            if let CompletionSearchProgress::Finished(result) = search.advance(world, obstacles) {
                return result;
            }
        }
    }

    /// Revalidate and apply the preview as a single construction edit.
    pub fn apply_completion(
        &mut self,
        completion: &Completion,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        if self.state_hash() != completion.construction_hash {
            return Err(ConstructionError::InvalidCircuit);
        }
        let mut unchained = self.copy_for_completion();
        unchained.set_chain(false);
        let mut updated = unchained.append_sequence(&completion.pieces, world, obstacles)?;
        updated.set_chain(self.chain);
        updated.close()?;
        *self = updated;
        Ok(())
    }
}

pub enum CompletionSearchProgress {
    Pending,
    Finished(Result<Vec<Completion>, String>),
}

/// Why closing sequences were rejected, with the worst example of each kind.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Rejections {
    closing_routes: usize,
    energy_pruned: usize,
    stalled: usize,
    too_fast: usize,
    too_slow: usize,
    over_force: usize,
    worst_too_fast: Option<(f64, String)>,
    worst_too_slow: Option<(f64, String)>,
    worst_over_force: Option<(f64, String)>,
}

fn record_worst(worst: &mut Option<(f64, String)>, shortfall: f64, description: String) {
    if worst
        .as_ref()
        .is_none_or(|(current, _)| shortfall > *current)
    {
        *worst = Some((shortfall, description));
    }
}

enum PhysicsRejection {
    Stalled,
    TooFast { shortfall: f64, description: String },
    TooSlow { shortfall: f64, description: String },
    OverForce { shortfall: f64, description: String },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CompletionSearch {
    /// False only for test searches that cover geometry on unpowered layouts.
    physics_checked: bool,
    target: TrackState,
    construction_hash: u64,
    original_length: usize,
    end_state: TrackState,
    end_speed_mps: f64,
    preview_cache: PreviewCache,
    depth: usize,
    beam: Vec<SearchNode>,
    next: Vec<SearchNode>,
    current: Option<SearchNode>,
    candidates: Vec<Piece>,
    closing: Vec<SearchNode>,
    found: Vec<Completion>,
    rejections: Rejections,
}

impl CompletionSearch {
    /// Advance by at most one placement validation or one physics check, so callers can yield
    /// between calls.
    pub fn advance(
        &mut self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> CompletionSearchProgress {
        loop {
            if let Some(piece) = self.candidates.pop() {
                self.try_candidate(piece, world, obstacles);
                return CompletionSearchProgress::Pending;
            }
            if let Some(node) = self.beam.pop() {
                self.candidates = self.candidate_pieces(&node);
                self.current = Some(node);
                continue;
            }
            if let Some(node) = self.closing.pop() {
                let evaluation = if self.physics_checked {
                    self.evaluate(&node)
                } else {
                    Ok(CompletionPhysics::default())
                };
                match evaluation {
                    Ok(physics) => self.found.push(Completion {
                        construction_hash: self.construction_hash,
                        geometry: node.construction.pieces[self.original_length..].to_vec(),
                        pieces: node.pieces,
                        score: 0.0,
                        physics,
                    }),
                    Err(rejection) => self.record_rejection(rejection),
                }
                return CompletionSearchProgress::Pending;
            }
            self.depth += 1;
            if !self.found.is_empty() || self.depth == MAX_PIECES || self.next.is_empty() {
                return CompletionSearchProgress::Finished(self.finish());
            }
            self.next
                .sort_by(|first, second| first.rank.total_cmp(&second.rank));
            let mut unique: Vec<SearchNode> = Vec::new();
            for node in self.next.drain(..) {
                // Routes to the same end with different brakes or rated pieces ride differently.
                if !unique.iter().any(|other| {
                    other.construction.end() == node.construction.end()
                        && other.brake_count == node.brake_count
                        && other.rated_piece_count == node.rated_piece_count
                }) {
                    unique.push(node);
                }
            }
            unique.truncate(BEAM_WIDTH);
            unique.reverse();
            self.beam = unique;
        }
    }

    fn candidate_pieces(&self, node: &SearchNode) -> Vec<Piece> {
        let end = node.construction.end();
        let mut seen = HashSet::new();
        let mut candidates: Vec<(Piece, f64)> = templates()
            .iter()
            .filter_map(|base| {
                if !seen.insert(base.id) {
                    return None;
                }
                let template = template_for(base.id, end);
                let searchable = match template.kind {
                    // Shaped elements need a solve per entry speed and inversions do not
                    // belong in a station approach.
                    PieceKind::Track => {
                        matches!(
                            template.shape,
                            Shape::Plain
                                | Shape::SpeedBank { .. }
                                | Shape::Profile(_)
                                | Shape::AirtimeHill
                                | Shape::Overbank
                                | Shape::WaveTurn { .. }
                        ) && template.inversion.is_none()
                    }
                    PieceKind::Brakes => true,
                    PieceKind::Station | PieceKind::Lift | PieceKind::Launch => false,
                };
                if !searchable {
                    return None;
                }
                let piece = node.construction.model_placeable_geometry(template).ok()?;
                let estimated_speed = node.speed_squared_estimate.max(0.0).sqrt();
                let outside_rated_window = self.physics_checked
                    && rated_speeds(node.construction.model(), &piece)
                        .is_some_and(|window| rated_speed_margin(window, estimated_speed) < 0.0);
                let order = heuristic(piece.exit, self.target)
                    + piece_score(&piece)
                    + if outside_rated_window {
                        OUTSIDE_RATED_WINDOW_ORDER_PENALTY
                    } else {
                        0.0
                    };
                Some((piece, order))
            })
            .collect();
        candidates.sort_by(|first, second| first.1.total_cmp(&second.1));
        let mut unique: Vec<Piece> = Vec::new();
        for (piece, _) in candidates {
            // Banked speed turns share their end states with flat turns; keep both.
            if !unique.iter().any(|other| {
                other.kind == piece.kind
                    && same_geometry(other, &piece)
                    && rated_window_key(other) == rated_window_key(&piece)
            }) {
                unique.push(piece);
            }
        }
        unique.truncate(CANDIDATES_PER_NODE);
        unique.reverse();
        unique
    }

    fn try_candidate(&mut self, piece: Piece, world: &World, obstacles: &[SupportObstacle]) {
        let node = self.current.as_ref().expect("candidate has a node");
        let mut construction = node.construction.copy_for_completion_search();
        if construction.append(piece.id, world, obstacles).is_err() {
            return;
        }
        let speed_squared_estimate =
            estimated_exit_speed_squared(node.speed_squared_estimate, &piece);
        if speed_squared_estimate <= 0.0 {
            self.rejections.energy_pruned += 1;
            return;
        }
        let mut pieces = node.pieces.clone();
        pieces.push(piece.id);
        let brake_count = node.brake_count + usize::from(piece.kind == PieceKind::Brakes);
        let rated_piece_count = node.rated_piece_count + usize::from(piece.design_speeds.is_some());
        let rank = pieces.len() as f64 + heuristic(construction.end(), self.target);
        let closes = construction.end() == self.target && {
            let mut closed = construction.copy_for_completion();
            closed.close().is_ok()
        };
        let node = SearchNode {
            construction,
            pieces,
            rank,
            brake_count,
            rated_piece_count,
            speed_squared_estimate,
        };
        if closes {
            self.rejections.closing_routes += 1;
            self.closing.push(node);
        } else if node.pieces.len() < MAX_PIECES {
            self.next.push(node);
        }
    }

    fn evaluate(&mut self, node: &SearchNode) -> Result<CompletionPhysics, PhysicsRejection> {
        let construction = &node.construction;
        let model = construction.model();
        let pieces = construction.pieces();
        let mut cache = self.preview_cache.clone();
        let preview = cache
            .preview_open_continuation(pieces, model, construction_preview_physics())
            .expect("a track with a station has a preview");
        if preview.status != PreviewStatus::ReachedEnd {
            return Err(PhysicsRejection::Stalled);
        }
        let arrival_speed_mps = preview
            .end_speed_mps
            .expect("a preview that reached the end has an end speed");
        let samples = &preview.report.samples;
        let car_spacing = model.train.length_metres / f64::from(model.train.cars);
        let train_mass_kg =
            f64::from(model.train.cars) * construction_preview_physics().car_mass_kg;
        let limits = model.comfortable_force_limits;
        let mut start_distance: f64 = pieces[..self.original_length]
            .iter()
            .map(Piece::length)
            .sum();
        let mut reports = Vec::new();
        let mut brake_energy_kj = 0.0;
        for (piece_index, piece) in pieces.iter().enumerate().skip(self.original_length) {
            let end_distance = start_distance + piece.length();
            let entry_speed_mps = speed_at(samples, start_distance, arrival_speed_mps);
            let exit_speed_mps = speed_at(samples, end_distance, arrival_speed_mps);
            let worst_car_peaks = worst_car_peaks(&preview, piece_index);
            let (force_headroom_g, axis) = force_headroom(worst_car_peaks, limits);
            if force_headroom_g < 0.0 {
                return Err(PhysicsRejection::OverForce {
                    shortfall: -force_headroom_g,
                    description: format!(
                        "{:?} reaches {axis} force {:.2} g past the {FORCE_HEADROOM_G:.2} g headroom at {entry_speed_mps:.1} m/s",
                        piece.id, -force_headroom_g
                    ),
                });
            }
            let rated_window = rated_speeds(model, piece);
            let mut rated_margin_mps = None;
            if let Some(window) = rated_window {
                for car_index in 0..model.train.cars {
                    let car_entry_speed = speed_at(
                        samples,
                        start_distance + f64::from(car_index) * car_spacing,
                        arrival_speed_mps,
                    );
                    let margin = rated_speed_margin(window, car_entry_speed);
                    if margin < 0.0 {
                        let centre = (window.minimum_entry + window.maximum_entry) / 2.0;
                        let description = format!(
                            "{:?} entered at {car_entry_speed:.1} m/s by car {} against its rated window {:.1}-{:.1} m/s, {:.1} m/s short of the required margin",
                            piece.id,
                            car_index + 1,
                            window.minimum_entry,
                            window.maximum_entry,
                            -margin
                        );
                        return Err(if car_entry_speed > centre {
                            PhysicsRejection::TooFast {
                                shortfall: -margin,
                                description,
                            }
                        } else {
                            PhysicsRejection::TooSlow {
                                shortfall: -margin,
                                description,
                            }
                        });
                    }
                    rated_margin_mps =
                        Some(rated_margin_mps.map_or(margin, |current: f64| current.min(margin)));
                }
            }
            if piece.kind == PieceKind::Brakes {
                brake_energy_kj += 0.5
                    * train_mass_kg
                    * (entry_speed_mps.powi(2) - exit_speed_mps.powi(2)).max(0.0)
                    / 1000.0;
            }
            reports.push(CompletionPieceReport {
                id: piece.id,
                entry_speed_mps,
                exit_speed_mps,
                worst_car_peaks,
                force_headroom_g,
                rated_window_mps: rated_window
                    .map(|window| (window.minimum_entry, window.maximum_entry)),
                rated_margin_mps,
            });
            start_distance = end_distance;
        }
        Ok(CompletionPhysics {
            arrival_speed_mps,
            brake_energy_kj,
            minimum_force_headroom_g: reports
                .iter()
                .map(|report| report.force_headroom_g)
                .fold(f64::INFINITY, f64::min),
            minimum_rated_margin_mps: reports
                .iter()
                .filter_map(|report| report.rated_margin_mps)
                .reduce(f64::min),
            pieces: reports,
        })
    }

    fn record_rejection(&mut self, rejection: PhysicsRejection) {
        let rejections = &mut self.rejections;
        match rejection {
            PhysicsRejection::Stalled => rejections.stalled += 1,
            PhysicsRejection::TooFast {
                shortfall,
                description,
            } => {
                rejections.too_fast += 1;
                record_worst(&mut rejections.worst_too_fast, shortfall, description);
            }
            PhysicsRejection::TooSlow {
                shortfall,
                description,
            } => {
                rejections.too_slow += 1;
                record_worst(&mut rejections.worst_too_slow, shortfall, description);
            }
            PhysicsRejection::OverForce {
                shortfall,
                description,
            } => {
                rejections.over_force += 1;
                record_worst(&mut rejections.worst_over_force, shortfall, description);
            }
        }
    }

    fn finish(&mut self) -> Result<Vec<Completion>, String> {
        let mut found = std::mem::take(&mut self.found);
        if found.is_empty() {
            return Err(self.explain_failure());
        }
        found.sort_by(|first, second| {
            first
                .pieces
                .len()
                .cmp(&second.pieces.len())
                .then(
                    first
                        .physics
                        .brake_energy_kj
                        .total_cmp(&second.physics.brake_energy_kj),
                )
                .then(worst_margin(second).total_cmp(&worst_margin(first)))
        });
        found.truncate(MAX_COMPLETIONS);
        for (index, completion) in found.iter_mut().enumerate() {
            completion.score = (index + 1) as f64;
        }
        Ok(found)
    }

    fn explain_failure(&self) -> String {
        let rejections = &self.rejections;
        let offset = self.target.position() - self.end_state.position();
        let heading_steps =
            (self.target.heading.index() - self.end_state.heading.index()).rem_euclid(8);
        let heading_mismatch = heading_steps.min(8 - heading_steps) * 45;
        let geometry = format!(
            "the end is {:.0} m from the station entry (x {:+.1}, y {:+.1} tiles, height {:+.1} m) with a {heading_mismatch} degree heading mismatch, and the train reaches it at {:.1} m/s",
            offset.length(),
            offset.x / 4.0,
            offset.y / 4.0,
            offset.z,
            self.end_speed_mps
        );
        if rejections.closing_routes == 0 {
            if rejections.energy_pruned > 0 && offset.z > 0.0 {
                let needed = (2.0 * GRAVITY * offset.z).sqrt();
                return format!(
                    "No finish found: {geometry}. The train is too slow to climb {:.1} m back to the station (it needs at least {needed:.1} m/s). End the track higher or faster.",
                    offset.z
                );
            }
            return format!(
                "No finish found: no route of up to {MAX_PIECES} pieces reaches the station entry without a collision; {geometry}. Bring the end closer to the station entry, line it up with the station's heading, or clear the way."
            );
        }
        let mut parts = Vec::new();
        if rejections.too_fast > 0 {
            parts.push(format!(
                "{} too fast for a rated piece (worst: {})",
                rejections.too_fast,
                rejections.worst_too_fast.as_ref().expect("counted").1
            ));
        }
        if rejections.over_force > 0 {
            parts.push(format!(
                "{} over a force limit (worst: {})",
                rejections.over_force,
                rejections.worst_over_force.as_ref().expect("counted").1
            ));
        }
        if rejections.too_slow > 0 {
            parts.push(format!(
                "{} too slow for a rated piece (worst: {})",
                rejections.too_slow,
                rejections.worst_too_slow.as_ref().expect("counted").1
            ));
        }
        if rejections.stalled > 0 {
            parts.push(format!("{} stop before the station", rejections.stalled));
        }
        let advice = if rejections.too_fast + rejections.over_force
            >= rejections.too_slow + rejections.stalled
        {
            "The train is too fast here: lose speed before the return with brakes or a climb, or end the track further from the station so the finish has room for brakes."
        } else {
            "The train is too slow here: end the track higher or faster."
        };
        format!(
            "No safe finish found: {} route(s) reach the station but none is safe: {}; {geometry}. {advice}",
            rejections.closing_routes,
            parts.join("; ")
        )
    }
}

fn rated_window_key(piece: &Piece) -> Option<(u64, u64)> {
    piece.design_speeds.map(|window| {
        (
            window.minimum_entry.to_bits(),
            window.maximum_entry.to_bits(),
        )
    })
}

fn worst_margin(completion: &Completion) -> f64 {
    let rated = completion
        .physics
        .minimum_rated_margin_mps
        .unwrap_or(f64::INFINITY);
    completion.physics.minimum_force_headroom_g.min(rated)
}

fn worst_car_peaks(preview: &TrackPreview, piece_index: usize) -> ForcePeakValues {
    preview
        .piece_force_peaks
        .iter()
        .find(|peaks| peaks.piece_index == piece_index)
        .and_then(|peaks| peaks.worst_car)
        .expect("every piece the train crossed has worst-car peaks")
}

/// Remaining g inside `limit - FORCE_HEADROOM_G` on the tightest axis, and that axis.
fn force_headroom(
    peaks: ForcePeakValues,
    limits: crate::track::ComfortableForceLimits,
) -> (f64, &'static str) {
    [
        (
            limits.maximum_vertical_g - FORCE_HEADROOM_G - peaks.maximum_vertical_g,
            "positive vertical",
        ),
        (
            peaks.minimum_vertical_g - (limits.minimum_vertical_g + FORCE_HEADROOM_G),
            "negative vertical",
        ),
        (
            limits.maximum_lateral_g - FORCE_HEADROOM_G - peaks.maximum_lateral_g.abs(),
            "lateral",
        ),
        (
            limits.maximum_longitudinal_g - FORCE_HEADROOM_G - peaks.maximum_longitudinal_g.abs(),
            "longitudinal",
        ),
    ]
    .into_iter()
    .min_by(|first, second| first.0.total_cmp(&second.0))
    .expect("four axes")
}

/// Front-car speed at `distance`; past the last sample the train is on the runout, where the
/// speed at the open end is used.
fn speed_at(samples: &[TestSample], distance: f64, end_speed: f64) -> f64 {
    let after = samples.partition_point(|sample| sample.distance < distance);
    if after == samples.len() {
        return end_speed;
    }
    if after == 0 {
        return samples[0].speed;
    }
    let (before, after) = (&samples[after - 1], &samples[after]);
    let fraction = (distance - before.distance) / (after.distance - before.distance);
    before.speed + (after.speed - before.speed) * fraction
}

/// Energy estimate used only to drop routes early: height change, rolling resistance and the
/// brakes' deceleration while the front car is on them.
fn estimated_exit_speed_squared(entry_speed_squared: f64, piece: &Piece) -> f64 {
    let config = construction_preview_physics();
    let mut speed_squared = entry_speed_squared
        - 2.0 * GRAVITY * (piece.exit.position().z - piece.entry.position().z)
        - 2.0 * config.rolling_resistance * GRAVITY * piece.length();
    if piece.kind == PieceKind::Brakes && speed_squared > config.brake_speed.powi(2) {
        speed_squared = (speed_squared - 2.0 * config.braking_acceleration * piece.length())
            .max(config.brake_speed.powi(2));
    }
    speed_squared
}

fn distance(from: TrackState, to: TrackState) -> f64 {
    (from.position() - to.position()).length()
}

/// Estimated remaining route length: the best over up to two classic turns of the cost to line
/// up with the station entry.
fn heuristic(from: TrackState, target: TrackState) -> f64 {
    const TURN_COST_METRES: f64 = 4.0;
    let mut frontier = vec![(from, 0.0)];
    let mut best = f64::INFINITY;
    for depth in 0..=2 {
        let mut next = Vec::new();
        for &(state, cost) in &frontier {
            best = best.min(cost + alignment_cost(state, target));
            if depth == 2 {
                continue;
            }
            for id in [
                PieceId::TurnSmallStandalone,
                PieceId::TurnSmallRightStandalone,
                PieceId::TurnLargeStandalone,
                PieceId::TurnLargeRightStandalone,
            ] {
                if let Some(piece) = template_for(id, state).at(state) {
                    next.push((piece.exit, cost + TURN_COST_METRES));
                }
            }
        }
        frontier = next;
    }
    best
}

/// With the heading already right, the station must lie ahead: straight distance plus the
/// sideways and height offsets the remaining pieces must still remove. Otherwise distance plus a
/// penalty per 45 degrees of heading error.
fn alignment_cost(state: TrackState, target: TrackState) -> f64 {
    const HEADING_STEP_PENALTY_METRES: f64 = 16.0;
    const OFFSET_COST_PER_METRE: f64 = 1.5;
    const LOOP_PENALTY_METRES: f64 = 200.0;
    let heading_steps = (target.heading.index() - state.heading.index()).rem_euclid(8);
    let mismatch = heading_steps.min(8 - heading_steps);
    let offset = target.position() - state.position();
    if mismatch != 0 {
        return offset.length() + f64::from(mismatch) * HEADING_STEP_PENALTY_METRES;
    }
    let tangent = target.tangent();
    let horizontal = Vec3::new(tangent.x, tangent.y, 0.0).unit();
    let along = offset.x * horizontal.x + offset.y * horizontal.y;
    let sideways = (offset.x * horizontal.y - offset.y * horizontal.x).abs();
    let overshoot_penalty = if along < -0.1 {
        LOOP_PENALTY_METRES
    } else {
        0.0
    };
    along.abs() + overshoot_penalty + OFFSET_COST_PER_METRE * (sideways + offset.z.abs())
}

fn piece_score(piece: &Piece) -> f64 {
    1.0 + piece.length() * piece.maximum_curvature()
        + piece.entry.slope.gradient().abs()
        + piece.exit.slope.gradient().abs()
        + (piece.exit.slope.gradient() - piece.entry.slope.gradient()).abs()
        + (piece.exit.bank.radians() - piece.entry.bank.radians()).abs()
}
