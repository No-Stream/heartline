//! Finish-the-circuit search: short piece sequences from the open end back into the station.
//!
//! A beam search over plain track pieces, brakes and, for models that support them, shaped
//! fully banked turns finds sequences that reach the station entry with valid placement. Each
//! shaped turn is solved for the simulated speed at its entry, as the builder solves it. Each closing sequence is then simulated with the builder's
//! construction-test physics, resuming from the preview snapshot at the open end, and kept
//! only when the train reaches the station, every new piece's worst-car forces stay
//! `FORCE_HEADROOM_G` inside the model's comfortable limits, and every car enters every rated
//! new piece with a non-negative rated-speed margin. See
//! docs/superpowers/plans/2026-11-20-finish-circuit.md.

use super::*;
use crate::physics::{
    ForcePeakValues, GRAVITY, PhysicsConfig, PreviewCache, PreviewStatus, TestSample, TrackPreview,
};
use crate::track::shaped::{
    ElementIntent, ElementSpec, ShapedConstructionError, SolveContext, SolveRefusal, TurnSizeHint,
    automatic_turn_radius_m, model_supports_shaped_elements, solve, turn_exit,
};
use crate::track::{
    Bank, ENVELOPE_DISPLAY_LIMITS, Hand, HorizontalCurvature, RatedSpeeds, Slope,
    VerticalCurvature, feel_rated_speeds, rated_speed_margin, rated_speeds,
};

/// Force and rated-window judge used by the finish search.
///
/// In-game player construction uses `Envelope` (global ASTM force envelope and hard rated
/// windows). Design tools (`tools/design finish`) use `DesignTargets` (the model's design force
/// targets with `FORCE_HEADROOM_G` margin, shaped-turn targets, and feel rated windows) so
/// sample-ride finishes remain vetted against model design targets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FinishForceJudge {
    #[default]
    Envelope,
    DesignTargets,
}

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
/// Shaped turns solved per search depth across the beam, chosen by the rank their routes would
/// have; each solve costs one to three short simulations.
const SHAPED_TURNS_PER_DEPTH: usize = 2;
/// Straight track a shaped turn leaves before the station entry. A train waiting in the
/// station rests its last cars on the track behind the entry, and on a steeply banked turn they
/// feel lateral force at dispatch: on Horizon Hawk, 0.62 g with one tile of straight and 0.44 g
/// with two (the owner's ceiling is 0.6 g). Ending at the entry itself also leaves the builder
/// with no piece selected, which the ride check's builder snapshots cannot record.
const SHAPED_TURN_STATION_CLEARANCE_METRES: f64 = 8.0;
/// Heuristic cost of one turn in the remaining route.
const TURN_COST_METRES: f64 = 4.0;
/// Fixed shaped-turn radii step by 2 m, as the builder's radius stepper does.
const SHAPED_TURN_RADIUS_STEP_METRES: f64 = 2.0;

/// The physics the builder's force preview uses: default physics with a 5 m/s lift.
pub fn construction_preview_physics() -> PhysicsConfig {
    PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    }
}

/// A validated way back to the station. Its serialized layout predates the physics report,
/// which is skipped so park saves stay compatible; loading a park restarts the search. Shaped
/// turns in `geometry` carry their solved spec; `shaped_turn_intent` gives their intent.
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
    /// Not saved, like the other shaped-turn state below: loading a park discards the search.
    #[serde(skip)]
    shaped_piece_count: usize,
    /// Shaped turns to solve at this node, best last.
    #[serde(skip)]
    shaped_turns: Vec<ShapedTurnCandidate>,
}

/// A route one piece longer than an expanded node, ranked before its placement is validated:
/// only children that make the next beam, or reach the station, are validated.
struct PendingChild {
    /// Index of the parent in `CompletionSearch::expanded`.
    parent: usize,
    piece: Piece,
    pieces: Vec<PieceId>,
    rank: f64,
    brake_count: usize,
    rated_piece_count: usize,
    shaped_piece_count: usize,
    speed_squared_estimate: f64,
}

impl PendingChild {
    /// Routes to the same end with different brakes, rated or shaped pieces ride differently.
    fn same_route_class(&self, node: &SearchNode) -> bool {
        node.construction.end() == self.piece.exit
            && node.brake_count == self.brake_count
            && node.rated_piece_count == self.rated_piece_count
            && node.shaped_piece_count == self.shaped_piece_count
    }
}

/// A shaped fully banked turn to solve at the current search node.
#[derive(Clone, Copy)]
struct ShapedTurnCandidate {
    hand: Hand,
    radius_m: f64,
}

/// The simulated end of the current search node, shared by its shaped-turn solves.
#[derive(Clone)]
struct NodeEndPreview {
    end_speed_mps: Option<f64>,
    cache: PreviewCache,
}

/// The intent a finish search solves a shaped turn with: its hand and its fixed radius. Ride
/// files store it with the solved spec, as the builder does for a placed shaped turn.
pub fn shaped_turn_intent(spec: &ElementSpec) -> ElementIntent {
    match spec {
        ElementSpec::Turn { hand, radius_m, .. } => ElementIntent::Turn {
            hand: *hand,
            size: TurnSizeHint::Radius(*radius_m),
        },
        ElementSpec::Hill { .. }
        | ElementSpec::Loop { .. }
        | ElementSpec::ZeroGRoll { .. }
        | ElementSpec::RavenTurn { .. } => {
            panic!("the finish search places only shaped turns")
        }
    }
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

    /// Starts a finish search using the global ASTM envelope, or explains why none can start.
    /// The builder's force preview of the open track is computed (or reused from the cache) here.
    pub fn completion_search(&self) -> Result<CompletionSearch, String> {
        self.completion_search_with_judge(FinishForceJudge::Envelope)
    }

    /// Starts a finish search using the supplied force judge, or explains why none can start.
    pub fn completion_search_with_judge(
        &self,
        judge: FinishForceJudge,
    ) -> Result<CompletionSearch, String> {
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
        Ok(self.search_from(end_speed_mps, preview_cache, true, judge))
    }

    /// Geometry coverage for unpowered test layouts: every closing route counts, unsimulated.
    #[cfg(test)]
    pub(super) fn completion_search_ignoring_physics(&self) -> CompletionSearch {
        self.search_from(
            f64::INFINITY,
            PreviewCache::default(),
            false,
            FinishForceJudge::Envelope,
        )
    }

    fn search_from(
        &self,
        end_speed_mps: f64,
        preview_cache: PreviewCache,
        physics_checked: bool,
        judge: FinishForceJudge,
    ) -> CompletionSearch {
        let mut construction = self.copy_for_completion_search();
        construction.set_chain(false);
        let mut search = CompletionSearch {
            physics_checked,
            judge,
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
                speed_squared_estimate: crate::math::powi(end_speed_mps, 2),
                shaped_piece_count: 0,
                shaped_turns: Vec::new(),
            }],
            next: Vec::new(),
            current: None,
            candidates: Vec::new(),
            shaped_candidates: Vec::new(),
            current_end_preview: None,
            expanded: Vec::new(),
            pending: Vec::new(),
            selecting: Vec::new(),
            closing: Vec::new(),
            found: Vec::new(),
            rejections: Rejections::default(),
        };
        search.assign_shaped_turns();
        search
    }

    /// Runs the whole finish search with the global ASTM envelope; returns ranked completions or
    /// why there are none.
    pub fn find_completions(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<Vec<Completion>, String> {
        self.find_completions_with_judge(world, obstacles, FinishForceJudge::Envelope)
    }

    /// Runs the whole finish search with the supplied force judge; returns ranked completions or
    /// why there are none.
    pub fn find_completions_with_judge(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
        judge: FinishForceJudge,
    ) -> Result<Vec<Completion>, String> {
        let mut search = self.completion_search_with_judge(judge)?;
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
        if self.state_hash() != completion.construction_hash
            || completion.pieces.len() != completion.geometry.len()
            || completion
                .pieces
                .iter()
                .zip(&completion.geometry)
                .any(|(id, piece)| *id != piece.id)
        {
            return Err(ConstructionError::InvalidCircuit);
        }
        let mut updated = self.copy_for_completion();
        updated.set_chain(false);
        for piece in &completion.geometry {
            match piece.element_spec() {
                Some(spec) => {
                    updated
                        .append_resolved(&shaped_turn_intent(spec), spec, world, obstacles)
                        .map_err(|error| match error {
                            ShapedConstructionError::Construction(error) => error,
                            invalid => panic!(
                                "finish geometry holds a shaped turn its own intent refuses: {invalid:?}"
                            ),
                        })?;
                }
                None => updated.append(piece.id, world, obstacles)?,
            }
        }
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
    #[serde(skip)]
    shaped_turns_refused: usize,
    #[serde(skip)]
    first_shaped_turn_refusal: Option<String>,
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
    /// Not saved: loading a park discards any in-flight finish search.
    #[serde(skip)]
    judge: FinishForceJudge,
    target: TrackState,
    construction_hash: u64,
    original_length: usize,
    end_state: TrackState,
    end_speed_mps: f64,
    preview_cache: PreviewCache,
    depth: usize,
    beam: Vec<SearchNode>,
    /// The next beam, best first, filled from `selecting`.
    next: Vec<SearchNode>,
    current: Option<SearchNode>,
    candidates: Vec<Piece>,
    #[serde(skip)]
    shaped_candidates: Vec<ShapedTurnCandidate>,
    #[serde(skip)]
    current_end_preview: Option<NodeEndPreview>,
    /// Nodes of the current depth whose candidates have been tried, parents of `pending`.
    #[serde(skip)]
    expanded: Vec<SearchNode>,
    #[serde(skip)]
    pending: Vec<PendingChild>,
    /// Pending children in reverse rank order while the next beam is filled.
    #[serde(skip)]
    selecting: Vec<PendingChild>,
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
            if let Some(turn) = self.shaped_candidates.pop() {
                self.try_shaped_turn(turn, world, obstacles);
                return CompletionSearchProgress::Pending;
            }
            if let Some(expanded) = self.current.take() {
                self.expanded.push(expanded);
            }
            if let Some(mut node) = self.beam.pop() {
                self.candidates = self.candidate_pieces(&node);
                self.shaped_candidates = std::mem::take(&mut node.shaped_turns);
                self.current_end_preview = None;
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
            if !self.found.is_empty() {
                return CompletionSearchProgress::Finished(self.finish());
            }
            if !self.pending.is_empty() {
                // Stable, so equal ranks keep the order the candidates were tried in.
                self.pending
                    .sort_by(|first, second| first.rank.total_cmp(&second.rank));
                self.selecting = std::mem::take(&mut self.pending);
                self.selecting.reverse();
            }
            if self.next.len() < BEAM_WIDTH
                && let Some(child) = self.selecting.pop()
            {
                if !self.next.iter().any(|node| child.same_route_class(node)) {
                    self.select(child, world, obstacles);
                    return CompletionSearchProgress::Pending;
                }
                continue;
            }
            self.selecting.clear();
            self.expanded.clear();
            self.depth += 1;
            if self.depth == MAX_PIECES || self.next.is_empty() {
                return CompletionSearchProgress::Finished(self.finish());
            }
            self.beam = std::mem::take(&mut self.next);
            self.beam.reverse();
            self.assign_shaped_turns();
        }
    }

    /// Validates a pending child's placement and adds it to the next beam when it is valid.
    fn select(&mut self, child: PendingChild, world: &World, obstacles: &[SupportObstacle]) {
        let Some(construction) = placed(
            &self.expanded[child.parent].construction,
            &child.piece,
            world,
            obstacles,
        ) else {
            return;
        };
        self.next.push(SearchNode {
            construction,
            pieces: child.pieces,
            rank: child.rank,
            brake_count: child.brake_count,
            rated_piece_count: child.rated_piece_count,
            speed_squared_estimate: child.speed_squared_estimate,
            shaped_piece_count: child.shaped_piece_count,
            shaped_turns: Vec::new(),
        });
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
                    && self
                        .rated_window_for(node.construction.model(), &piece)
                        .is_some_and(|window| self.rated_margin_for(window, estimated_speed) < 0.0);
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

    /// Hands the best `SHAPED_TURNS_PER_DEPTH` shaped turns across the beam to their nodes,
    /// ranked as the routes they would extend are ranked. The classic heuristic looks ahead only
    /// through classic turns, so a turn's exit may also be judged by a second shaped turn of the
    /// same radii right after it.
    fn assign_shaped_turns(&mut self) {
        let target = self.target;
        let mut options: Vec<(usize, ShapedTurnCandidate, f64)> = Vec::new();
        for (node_index, node) in self.beam.iter().enumerate() {
            let route_length = (node.pieces.len() + 1) as f64;
            let turns = self.shaped_turn_options(node);
            for &(turn, exit) in &turns {
                let second_turn_cost = turns
                    .iter()
                    .filter_map(|(second, _)| turn_exit(exit, second.hand, second.radius_m).ok())
                    .filter(|second_exit| {
                        distance(*second_exit, target) >= SHAPED_TURN_STATION_CLEARANCE_METRES
                    })
                    .map(|second_exit| TURN_COST_METRES + alignment_cost(second_exit, target))
                    .fold(f64::INFINITY, f64::min);
                let remaining = heuristic(exit, target).min(second_turn_cost);
                options.push((node_index, turn, route_length + remaining));
            }
        }
        options.sort_by(|first, second| first.2.total_cmp(&second.2));
        options.truncate(SHAPED_TURNS_PER_DEPTH);
        for (node_index, turn, _) in options.into_iter().rev() {
            self.beam[node_index].shaped_turns.push(turn);
        }
    }

    /// Fully banked turns of fixed even radii with their exits, for models that support shaped
    /// elements, at level straight ends on the grid axes (the builder's radius stepper reaches
    /// only these radii there) within the solver's speeds. The radii run from the automatically
    /// sized turn's (plus one step) down to where its positive force would reach the owner's
    /// ceiling, and stop at the distance to the station. A turn must end at least
    /// `SHAPED_TURN_STATION_CLEARANCE_METRES` from the station entry. Shaped hills are not
    /// offered: they end at their entry height and are longer than a station approach.
    fn shaped_turn_options(&self, node: &SearchNode) -> Vec<(ShapedTurnCandidate, TrackState)> {
        let model = node.construction.model();
        let end = node.construction.end();
        let estimated_speed = node.speed_squared_estimate.max(0.0).sqrt();
        let Some(profile) = model.shaped_profile else {
            return Vec::new();
        };
        if !self.physics_checked
            || !model_supports_shaped_elements(model, &model.train)
            || end.heading.index() % 2 != 0
            || end.slope != Slope::Level
            || end.bank != Bank::Level
            || end.horizontal_curvature != HorizontalCurvature::Straight
            || end.vertical_curvature != VerticalCurvature::Straight
            || !(profile.minimum_speed_mps..=profile.maximum_speed_mps).contains(&estimated_speed)
        {
            return Vec::new();
        }
        let automatic_radius = automatic_turn_radius_m(
            model,
            &model.train,
            end.heading,
            estimated_speed,
            construction_preview_physics(),
        );
        let max_vertical_g = match self.judge {
            FinishForceJudge::DesignTargets => profile.finish_turn_max_vertical_g,
            FinishForceJudge::Envelope => ENVELOPE_DISPLAY_LIMITS.maximum_vertical_g,
        };
        // Positive force grows roughly with the inverse radius at the automatic turn's bank.
        let force_limited_radius = automatic_radius
            * (crate::math::powi(profile.turn_vertical_g, 2) - 1.0).sqrt()
            / (crate::math::powi(max_vertical_g, 2) - 1.0).sqrt();
        let smallest_step = (force_limited_radius.max(profile.minimum_turn_radius_m)
            / SHAPED_TURN_RADIUS_STEP_METRES)
            .ceil() as u32;
        // Calibration banks a turn one grid step wider than the automatic size within the
        // lateral limit, but not two (measured at 22.8 and 23.0 m/s).
        let largest_step = ((automatic_radius + SHAPED_TURN_RADIUS_STEP_METRES)
            .min(distance(end, self.target))
            .min(profile.maximum_turn_radius_m)
            / SHAPED_TURN_RADIUS_STEP_METRES)
            .floor() as u32;
        [Hand::Left, Hand::Right]
            .into_iter()
            .flat_map(|hand| {
                (smallest_step..=largest_step).map(move |step| {
                    let radius_m = f64::from(step) * SHAPED_TURN_RADIUS_STEP_METRES;
                    let exit = turn_exit(end, hand, radius_m)
                        .expect("even radii lie on the grid of an axis-aligned turn");
                    (ShapedTurnCandidate { hand, radius_m }, exit)
                })
            })
            .filter(|(_, exit)| {
                distance(*exit, self.target) >= SHAPED_TURN_STATION_CLEARANCE_METRES
            })
            .collect()
    }

    /// Solves a shaped turn for the simulated speed at the current node's end, as the builder
    /// would for a player placing it there, and extends the route with it.
    fn try_shaped_turn(
        &mut self,
        turn: ShapedTurnCandidate,
        world: &World,
        obstacles: &[SupportObstacle],
    ) {
        let node = self.current.as_ref().expect("shaped candidate has a node");
        let model = node.construction.model();
        let physics = construction_preview_physics();
        let end_preview = self.current_end_preview.get_or_insert_with(|| {
            let mut cache = self.preview_cache.clone();
            let preview = cache
                .preview(node.construction.pieces(), model, physics)
                .expect("a track with a station has a preview");
            NodeEndPreview {
                end_speed_mps: (preview.status == PreviewStatus::ReachedEnd).then(|| {
                    preview
                        .end_speed_mps
                        .expect("a preview that reached the end has an end speed")
                }),
                cache,
            }
        });
        // A route whose train stops is rejected when it is evaluated.
        let Some(entry_speed_mps) = end_preview.end_speed_mps else {
            return;
        };
        let intent = ElementIntent::Turn {
            hand: turn.hand,
            size: TurnSizeHint::Radius(turn.radius_m),
        };
        let mut cache = end_preview.cache.clone();
        let mut context = SolveContext {
            entry: node.construction.end(),
            entry_speed_mps,
            model,
            train: &model.train,
            prefix: node.construction.pieces(),
            preview_cache: &mut cache,
            physics,
        };
        let Some(profile) = model.shaped_profile else {
            return;
        };
        let max_vertical_g = match self.judge {
            FinishForceJudge::DesignTargets => profile.finish_turn_max_vertical_g,
            FinishForceJudge::Envelope => ENVELOPE_DISPLAY_LIMITS.maximum_vertical_g,
        };
        let refusal = match solve(&intent, &mut context) {
            Ok(solved) => {
                let ElementSpec::Turn { vertical_g, .. } = solved.spec else {
                    unreachable!("a turn intent solves to a turn")
                };
                assert_eq!(shaped_turn_intent(&solved.spec), intent);
                if vertical_g <= max_vertical_g {
                    self.try_candidate(solved.piece, world, obstacles);
                    return;
                }
                format!(
                    "{:?} turn of radius {:.0} m at {entry_speed_mps:.1} m/s reaches {vertical_g:.2} g, over the {max_vertical_g:.1} g positive-force target",
                    turn.hand, turn.radius_m
                )
            }
            Err(refusal) => format!(
                "{:?} turn of radius {:.0} m at {entry_speed_mps:.1} m/s: {}",
                turn.hand,
                turn.radius_m,
                match refusal {
                    SolveRefusal::InfeasibleTarget { .. } =>
                        "no bank keeps it inside the lateral and force-headroom limits".to_owned(),
                    SolveRefusal::BelowMinimumSpeed {
                        minimum_speed_mps, ..
                    } =>
                        format!("slower than the {minimum_speed_mps:.0} m/s shaped-element minimum"),
                    other => format!("{other:?}"),
                }
            ),
        };
        let rejections = &mut self.rejections;
        rejections.shaped_turns_refused += 1;
        rejections.first_shaped_turn_refusal.get_or_insert(refusal);
    }

    /// Extends the current node by `piece`. Routes that reach the station entry are validated
    /// now and kept for evaluation; the others wait in `pending`, so only those that make the
    /// next beam are validated. A route the energy estimate stops counts as pruned only when
    /// its placement is valid.
    fn try_candidate(&mut self, piece: Piece, world: &World, obstacles: &[SupportObstacle]) {
        let node = self.current.as_ref().expect("candidate has a node");
        let speed_squared_estimate =
            estimated_exit_speed_squared(node.speed_squared_estimate, &piece);
        let mut pieces = node.pieces.clone();
        pieces.push(piece.id);
        let child = PendingChild {
            parent: self.expanded.len(),
            rank: pieces.len() as f64 + heuristic(piece.exit, self.target),
            pieces,
            brake_count: node.brake_count + usize::from(piece.kind == PieceKind::Brakes),
            rated_piece_count: node.rated_piece_count + usize::from(piece.design_speeds.is_some()),
            shaped_piece_count: node.shaped_piece_count
                + usize::from(piece.element_spec().is_some()),
            speed_squared_estimate,
            piece,
        };
        if speed_squared_estimate > 0.0 && child.piece.exit != self.target {
            if child.pieces.len() < MAX_PIECES {
                self.pending.push(child);
            }
            return;
        }
        let Some(construction) = placed(&node.construction, &child.piece, world, obstacles) else {
            return;
        };
        if speed_squared_estimate <= 0.0 {
            self.rejections.energy_pruned += 1;
            return;
        }
        let mut closed = construction.copy_for_completion();
        if closed.close().is_err() {
            if child.pieces.len() < MAX_PIECES {
                self.pending.push(child);
            }
            return;
        }
        self.rejections.closing_routes += 1;
        self.closing.push(SearchNode {
            construction,
            pieces: child.pieces,
            rank: child.rank,
            brake_count: child.brake_count,
            rated_piece_count: child.rated_piece_count,
            speed_squared_estimate,
            shaped_piece_count: child.shaped_piece_count,
            shaped_turns: Vec::new(),
        });
    }

    fn rated_window_for(&self, model: &CoasterModel, piece: &Piece) -> Option<RatedSpeeds> {
        match self.judge {
            FinishForceJudge::Envelope => rated_speeds(model, piece),
            FinishForceJudge::DesignTargets => {
                feel_rated_speeds(model, piece).or_else(|| rated_speeds(model, piece))
            }
        }
    }

    fn rated_margin_for(&self, window: RatedSpeeds, speed: f64) -> f64 {
        match self.judge {
            FinishForceJudge::Envelope => {
                (speed - window.minimum_entry).min(window.maximum_entry - speed)
            }
            FinishForceJudge::DesignTargets => rated_speed_margin(window, speed),
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
        let limits = model.design_force_targets;
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
            let force_headroom_g = match self.judge {
                FinishForceJudge::DesignTargets => {
                    let (headroom_g, axis) = force_headroom(worst_car_peaks, limits);
                    if headroom_g < 0.0 {
                        return Err(PhysicsRejection::OverForce {
                            shortfall: -headroom_g,
                            description: format!(
                                "{:?} reaches {axis} force {:.2} g past the {FORCE_HEADROOM_G:.2} g headroom at {entry_speed_mps:.1} m/s",
                                piece.id, -headroom_g
                            ),
                        });
                    }
                    if piece.element_spec().is_some()
                        && let Some(profile) = model.shaped_profile
                    {
                        for (observed_g, target_g, axis) in [
                            (
                                worst_car_peaks.maximum_lateral_g.abs(),
                                profile.finish_turn_max_lateral_g,
                                "lateral",
                            ),
                            (
                                worst_car_peaks.maximum_vertical_g,
                                profile.finish_turn_max_vertical_g,
                                "positive vertical",
                            ),
                        ] {
                            if observed_g > target_g {
                                return Err(PhysicsRejection::OverForce {
                                    shortfall: observed_g - target_g,
                                    description: format!(
                                        "{:?} reaches {axis} force {observed_g:.2} g, over the owner's {target_g:.1} g target, at {entry_speed_mps:.1} m/s",
                                        piece.id
                                    ),
                                });
                            }
                        }
                    }
                    headroom_g
                }
                FinishForceJudge::Envelope => {
                    if let Some((axis, value_g)) = preview
                        .force_envelope_samples
                        .iter()
                        .filter(|sample| {
                            sample.distance >= start_distance && sample.distance <= end_distance
                        })
                        .find_map(|sample| {
                            if sample.force_limit_exceeded[0] {
                                let vertical = if sample.minimum_vertical_g.abs()
                                    > sample.maximum_vertical_g.abs()
                                {
                                    sample.minimum_vertical_g
                                } else {
                                    sample.maximum_vertical_g
                                };
                                Some(("vertical", vertical))
                            } else if sample.force_limit_exceeded[1] {
                                Some(("lateral", sample.lateral_g))
                            } else if sample.force_limit_exceeded[2] {
                                Some(("longitudinal", sample.longitudinal_g))
                            } else {
                                None
                            }
                        })
                    {
                        return Err(PhysicsRejection::OverForce {
                            shortfall: value_g.abs(),
                            description: format!(
                                "{:?} reaches {axis} force {value_g:+.2} g past the comfort envelope at {entry_speed_mps:.1} m/s",
                                piece.id
                            ),
                        });
                    }
                    envelope_headroom(worst_car_peaks)
                }
            };
            let rated_window = self.rated_window_for(model, piece);
            let mut rated_margin_mps = None;
            if let Some(window) = rated_window {
                for car_index in 0..model.train.cars {
                    let car_entry_speed = speed_at(
                        samples,
                        start_distance + f64::from(car_index) * car_spacing,
                        arrival_speed_mps,
                    );
                    let margin = self.rated_margin_for(window, car_entry_speed);
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
                    * (crate::math::powi(entry_speed_mps, 2)
                        - crate::math::powi(exit_speed_mps, 2))
                    .max(0.0)
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
        let reason = self.explain_route_failure();
        match &self.rejections.first_shaped_turn_refusal {
            Some(refusal) => format!(
                "{reason} {} shaped turn(s) could not be solved within the force targets (first: {refusal}).",
                self.rejections.shaped_turns_refused
            ),
            None => reason,
        }
    }

    fn explain_route_failure(&self) -> String {
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

/// `construction` with `piece` appended, or `None` when the placement is invalid there.
fn placed(
    construction: &Construction,
    piece: &Piece,
    world: &World,
    obstacles: &[SupportObstacle],
) -> Option<Construction> {
    let mut extended = construction.copy_for_completion_search();
    let result = match piece.element_spec() {
        Some(spec) => extended
            .append_resolved(&shaped_turn_intent(spec), spec, world, obstacles)
            .map(|_| ())
            .map_err(|error| match error {
                ShapedConstructionError::Construction(error) => error,
                invalid => panic!("a solved shaped turn refuses its own intent: {invalid:?}"),
            }),
        None => extended.append(piece.id, world, obstacles),
    };
    result.ok().map(|()| extended)
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

/// Remaining g inside the global ASTM display limits for ranking envelope-judged completions.
fn envelope_headroom(peaks: ForcePeakValues) -> f64 {
    let limits = ENVELOPE_DISPLAY_LIMITS;
    [
        limits.maximum_vertical_g - peaks.maximum_vertical_g,
        peaks.minimum_vertical_g - limits.minimum_vertical_g,
        limits.maximum_lateral_g - peaks.maximum_lateral_g.abs(),
        limits.maximum_longitudinal_g - peaks.maximum_longitudinal_g.abs(),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

/// Remaining g inside `limit - FORCE_HEADROOM_G` on the tightest axis, and that axis.
fn force_headroom(
    peaks: ForcePeakValues,
    limits: crate::track::DesignForceTargets,
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
    if piece.kind == PieceKind::Brakes && speed_squared > crate::math::powi(config.brake_speed, 2) {
        speed_squared = (speed_squared - 2.0 * config.braking_acceleration * piece.length())
            .max(crate::math::powi(config.brake_speed, 2));
    }
    speed_squared
}

fn distance(from: TrackState, to: TrackState) -> f64 {
    (from.position() - to.position()).length()
}

/// Estimated remaining route length: the best over up to two classic turns of the cost to line
/// up with the station entry.
fn heuristic(from: TrackState, target: TrackState) -> f64 {
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
