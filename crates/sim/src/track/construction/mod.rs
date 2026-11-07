use crate::physics::Circuit;
use crate::state_hash::StateHasher;
use crate::track::clearance::{
    ClearanceBox, ClearanceCollision, ClearanceEnvelopes, ClearanceIndex, TerrainCell, TunnelPlan,
    check_clearance_indexed, check_clearance_with_envelopes, check_reverse_track_clearance,
    tunnel_plan, tunnel_plan_with_envelopes,
};
use crate::track::supports::{
    SUPPORT_INFLUENCE_RADIUS_METRES, Support, SupportError, SupportObstacle,
    generate_dense_supports, generate_supports, support_intersects_obstacle,
};
use crate::track::{
    CoasterColors, CoasterModel, Heartlining, HillFeel,
    LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED, LAUNCH_MAX_SPEED_METRES_PER_SECOND,
    LaunchSpeedBounds, Piece, PieceFamily, PieceId, PieceKind, Shape, SupportStyle, TrackState,
    TrainConfiguration, Vec3, catalog,
};
use crate::world::World;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashSet};
use std::sync::{Arc, OnceLock};

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod tests;

mod completion;
mod elements;
#[cfg(test)]
mod flexibility_tests;
mod speed_estimate;
#[cfg(test)]
mod walk_tests;
pub use completion::{Completion, CompletionSearch, CompletionSearchProgress};
pub use elements::ElementChoice;
pub use speed_estimate::{EndSpeedEstimate, EndSpeedSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionError {
    Closed,
    Clearance(ClearanceCollision),
    Unavailable,
    ModelForbidden,
    ChainNotAllowed,
    AmbiguousConnection,
    NoSupportRoute,
    InvalidIndex,
    InvalidLaunchIntensity,
    NotClosed,
    InvalidCircuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceChoice {
    pub id: PieceId,
    pub result: Result<(), ConstructionError>,
    pub would_tunnel: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeedMatchedVariant {
    pub id: PieceId,
    pub fits: bool,
}

/// Select a fitting hill closest to its target force, or the rated candidate
/// with the largest margin. If no window fits, select the nearest window.
pub fn select_speed_matched_variant(
    candidates: &[Piece],
    entry_speed: f64,
) -> Option<SpeedMatchedVariant> {
    select_speed_matched_variant_with_windows(candidates, entry_speed, -1.5, |piece| {
        piece.rated_speeds
    })
}

pub fn select_speed_matched_variant_for_train(
    candidates: &[Piece],
    entry_speed: f64,
    train: &super::TrainConfiguration,
    limits: super::ComfortableForceLimits,
) -> Option<SpeedMatchedVariant> {
    select_speed_matched_variant_with_windows(
        candidates,
        entry_speed,
        limits.minimum_vertical_g,
        |piece| super::effective_rated_speeds(piece, train),
    )
}

fn select_speed_matched_variant_with_windows(
    candidates: &[Piece],
    entry_speed: f64,
    minimum_vertical_g: f64,
    rated_speeds: impl Fn(&Piece) -> Option<super::RatedSpeeds>,
) -> Option<SpeedMatchedVariant> {
    assert!(entry_speed.is_finite() && entry_speed >= 0.0);
    let mut best: Option<(SpeedMatchedVariant, f64)> = None;
    for piece in candidates {
        let Some(window) = rated_speeds(piece) else {
            continue;
        };
        assert!(window.minimum_entry.is_finite() && window.maximum_entry.is_finite());
        assert!(window.minimum_entry >= 0.0 && window.minimum_entry <= window.maximum_entry);
        let headroom = window.maximum_entry - entry_speed;
        let margin = (entry_speed - window.minimum_entry).min(headroom);
        let fits = margin >= 0.0;
        let target = piece.family.and_then(|family| match family {
            PieceFamily::Hill(family) => match family.feel {
                // Target −1.2 g on steel and −0.7 g on wood, leaving 0.3 g above the model limit.
                HillFeel::Ejector => Some((minimum_vertical_g + 0.3).max(-1.2)),
                HillFeel::Floater => Some(-0.3),
                HillFeel::StrongFloater => Some(-0.7),
                HillFeel::Speed => Some(0.3),
                _ => None,
            },
            PieceFamily::SpeedTurn { .. } => None,
        });
        // Fitting windows outrank every window that does not fit.
        let score = if fits {
            match target {
                Some(target) => super::predicted_hill_forces(piece, entry_speed)
                    .map_or(0.0, |forces| {
                        1.0 / (1.0 + (forces.minimum_vertical_g - target).abs())
                    }),
                None => margin,
            }
        } else {
            margin
        };
        let choice = SpeedMatchedVariant { id: piece.id, fits };
        if best
            .as_ref()
            .is_none_or(|(_, best_score)| score > *best_score)
        {
            best = Some((choice, score));
        }
    }
    best.map(|(choice, _)| choice)
}

#[derive(Clone)]
struct PieceValidation {
    supports: Vec<Vec<Support>>,
    tunnel_plan: TunnelPlan,
    track_obstacles: Vec<SupportObstacle>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TrackSegment {
    pub ids: Vec<usize>,
    pub pieces: Vec<Piece>,
}

#[derive(Clone)]
struct AvailableChoice {
    piece: Piece,
    choice: PieceChoice,
    validation: Option<PieceValidation>,
}

#[derive(Clone)]
struct ChoicesCache {
    state_hash: u64,
    world_id: usize,
    world_revision: u64,
    obstacles: Vec<SupportObstacle>,
    choices: Vec<PieceChoice>,
    available_choices: Vec<AvailableChoice>,
}

#[derive(Clone)]
struct ValidationCache {
    world_id: usize,
    world_revision: u64,
    obstacles: Vec<SupportObstacle>,
    existing: Vec<Piece>,
    existing_tunnel_plans: Vec<TunnelPlan>,
    clearance_index: ClearanceIndex,
    track_obstacles: Vec<Vec<SupportObstacle>>,
    supports: Vec<Option<Result<Vec<Support>, SupportError>>>,
    support_regions: Vec<SupportSearchRegion>,
    /// Union of the existing pieces' tunnel carve cells.
    existing_carve_cells: BTreeSet<TerrainCell>,
    /// The world with `existing_carve_cells` carved, built once and shared by copies of the cache.
    existing_carved_world: Arc<OnceLock<Result<World, ConstructionError>>>,
    /// Supports routed for each existing piece on the existing carved world, among the other
    /// existing pieces. Unlike `supports`, an entry is dropped whenever a change could reach it,
    /// so it always equals a fresh routing and tunnelling validations can reuse it.
    exact_supports: Vec<Option<Result<Vec<Support>, SupportError>>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Construction {
    start: TrackState,
    model: CoasterModel,
    pieces: Vec<Piece>,
    ids: Vec<usize>,
    disconnected: Vec<TrackSegment>,
    next_id: usize,
    closed: bool,
    chain: bool,
    #[serde(default = "default_hill_variant")]
    selected_hill_variant: PieceId,
    colors: CoasterColors,
    #[serde(skip)]
    validation_cache: RefCell<Option<ValidationCache>>,
    #[serde(skip)]
    choices_cache: RefCell<Option<ChoicesCache>>,
    #[serde(skip)]
    element_choices_cache: RefCell<Option<elements::ElementChoicesCache>>,
    /// `state_hash` of the current editable state; every mutation of a hashed field clears it.
    #[serde(skip)]
    state_hash_memo: Cell<Option<u64>>,
    #[cfg(test)]
    support_cache_misses: Cell<usize>,
    #[cfg(test)]
    validation_cache_rebuilds: Cell<usize>,
}

fn default_hill_variant() -> PieceId {
    PieceId::FloaterHill8x64
}

impl Construction {
    pub fn new(start: TrackState, model: CoasterModel) -> Self {
        let colors = model.default_colors;
        Self {
            start,
            model,
            pieces: Vec::new(),
            ids: Vec::new(),
            disconnected: Vec::new(),
            next_id: 0,
            closed: false,
            chain: false,
            selected_hill_variant: default_hill_variant(),
            colors,
            validation_cache: RefCell::new(None),
            choices_cache: RefCell::new(None),
            element_choices_cache: RefCell::new(None),
            state_hash_memo: Cell::new(None),
            #[cfg(test)]
            support_cache_misses: Cell::new(0),
            #[cfg(test)]
            validation_cache_rebuilds: Cell::new(0),
        }
    }

    pub fn start(&self) -> TrackState {
        self.start
    }
    pub fn end(&self) -> TrackState {
        self.pieces.last().map_or(self.start, |piece| piece.exit)
    }
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn piece_ids(&self) -> &[usize] {
        &self.ids
    }
    pub fn disconnected_segments(&self) -> &[TrackSegment] {
        &self.disconnected
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn chain(&self) -> bool {
        self.chain
    }
    pub fn colors(&self) -> CoasterColors {
        self.colors
    }
    pub fn set_colors(&mut self, colors: CoasterColors) {
        self.invalidate_state_hash();
        self.colors = colors;
    }
    pub fn set_chain(&mut self, enabled: bool) {
        if self.chain != enabled {
            self.invalidate_state_hash();
            self.chain = enabled;
            *self.choices_cache.get_mut() = None;
        }
    }
    pub fn model(&self) -> &CoasterModel {
        &self.model
    }

    pub fn set_cars_per_train(&mut self, cars: u8) {
        assert!(self.model.cars_per_train.contains(&cars));
        let train = &mut self.model.train;
        if train.cars == cars {
            return;
        }
        train.length_metres = train.length_metres / f64::from(train.cars) * f64::from(cars);
        train.cars = cars;
        self.invalidate_state_hash();
        *self.validation_cache.get_mut() = None;
        *self.choices_cache.get_mut() = None;
        *self.element_choices_cache.get_mut() = None;
    }

    pub fn next_piece_id(&self) -> usize {
        self.next_id
    }

    /// Stable fingerprint of the editable track state, excluding derived validation caches.
    pub fn state_hash(&self) -> u64 {
        if let Some(memo) = self.state_hash_memo.get() {
            #[cfg(test)]
            assert_eq!(
                memo,
                self.compute_state_hash(),
                "a mutation did not clear the state hash memo"
            );
            return memo;
        }
        let hash = self.compute_state_hash();
        self.state_hash_memo.set(Some(hash));
        hash
    }

    fn invalidate_state_hash(&mut self) {
        *self.state_hash_memo.get_mut() = None;
    }

    fn compute_state_hash(&self) -> u64 {
        let mut hash = StateHasher::new();
        hash.write_tag(b"Construction-v1");
        self.start.hash_into(&mut hash);
        self.model.hash_into(&mut hash);
        hash.write_len(self.pieces.len());
        for piece in &self.pieces {
            piece.hash_into(&mut hash);
        }
        hash.write_len(self.ids.len());
        for &id in &self.ids {
            hash.write_len(id);
        }
        hash.write_len(self.disconnected.len());
        for segment in &self.disconnected {
            hash.write_len(segment.ids.len());
            for &id in &segment.ids {
                hash.write_len(id);
            }
            hash.write_len(segment.pieces.len());
            for piece in &segment.pieces {
                piece.hash_into(&mut hash);
            }
        }
        hash.write_len(self.next_id);
        hash.write_bool(self.closed);
        hash.write_bool(self.chain);
        self.colors.hash_into(&mut hash);
        hash.finish()
    }

    pub fn choices(&self, world: &World, obstacles: &[SupportObstacle]) -> Vec<PieceChoice> {
        if let Some(cache) = self.choices_cache.borrow().as_ref() {
            if cache.state_hash == self.state_hash()
                && cache.world_id == world.instance_id()
                && cache.world_revision == world.revision()
                && same_obstacles(&cache.obstacles, obstacles)
                && cache
                    .choices
                    .iter()
                    .any(|choice| choice.id == self.selected_hill_variant)
            {
                return self.visible_choices(&cache.choices);
            }
        }
        let previous_cache = self
            .choices_cache
            .borrow()
            .as_ref()
            .filter(|cache| {
                cache.state_hash == self.state_hash()
                    && cache.world_id == world.instance_id()
                    && cache.world_revision == world.revision()
                    && same_obstacles(&cache.obstacles, obstacles)
            })
            .cloned();
        // Choices already validated for this state are reused in catalog order, so the
        // list is the same as a fresh computation whichever hill variants were selected.
        let (previous_choices, previous_available_choices) = previous_cache
            .map_or_else(Default::default, |cache| {
                (cache.choices, cache.available_choices)
            });
        let mut choices = Vec::new();
        let mut available_choices: Vec<AvailableChoice> = Vec::new();
        let mut seen_ids = HashSet::new();
        for base in templates().iter().filter(|piece| seen_ids.insert(piece.id)) {
            if let Some(previous) = previous_choices.iter().find(|choice| choice.id == base.id) {
                choices.push(*previous);
                available_choices.extend(
                    previous_available_choices
                        .iter()
                        .find(|candidate| candidate.piece.id == base.id)
                        .cloned(),
                );
                continue;
            }
            if matches!(base.family, Some(PieceFamily::Hill(_)))
                && base.id != self.selected_hill_variant
            {
                continue;
            }
            let template = template_for(base.id, self.end());
            let placed = self.model_placeable_geometry(template);
            let mut would_tunnel = false;
            let (result, validation) = if self.closed {
                (Err(ConstructionError::Closed), None)
            } else {
                match placed.as_ref().map_err(|error| *error).and_then(|piece| {
                    if self.chain
                        && piece.kind == PieceKind::Track
                        && !self.model.allows_chain(piece)
                    {
                        return Err(ConstructionError::ChainNotAllowed);
                    }
                    Ok(piece)
                }) {
                    Err(error) => (Err(error), None),
                    Ok(piece) => {
                        if let Some(previous) = available_choices
                            .iter()
                            .chain(&previous_available_choices)
                            .find(|previous| same_geometry(&previous.piece, piece))
                        {
                            would_tunnel = previous.choice.would_tunnel;
                            (previous.choice.result, previous.validation.clone())
                        } else {
                            match self.validate_piece(piece, world, obstacles) {
                                Ok(validation) => {
                                    would_tunnel = validation.tunnel_plan.would_tunnel();
                                    (Ok(()), Some(validation))
                                }
                                Err(error) => (Err(error), None),
                            }
                        }
                    }
                }
            };
            let choice = PieceChoice {
                id: template.id,
                result,
                would_tunnel,
            };
            if let Ok(mut piece) = placed {
                if self.chain && self.model.allows_chain(&piece) {
                    piece.kind = PieceKind::Lift;
                }
                available_choices.push(AvailableChoice {
                    piece,
                    choice,
                    validation,
                });
            }
            choices.push(choice);
        }
        *self.choices_cache.borrow_mut() = Some(ChoicesCache {
            state_hash: self.state_hash(),
            world_id: world.instance_id(),
            world_revision: world.revision(),
            obstacles: obstacles.to_vec(),
            choices: choices.clone(),
            available_choices,
        });
        self.visible_choices(&choices)
    }

    fn visible_choices(&self, choices: &[PieceChoice]) -> Vec<PieceChoice> {
        choices
            .iter()
            .copied()
            .filter(|choice| {
                choice.id == self.selected_hill_variant
                    || !templates().iter().any(|piece| {
                        piece.id == choice.id && matches!(piece.family, Some(PieceFamily::Hill(_)))
                    })
            })
            .collect()
    }

    pub fn selected_hill_variant(&self) -> PieceId {
        self.selected_hill_variant
    }

    pub fn select_hill_variant(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        if !templates()
            .iter()
            .any(|piece| piece.id == id && matches!(piece.family, Some(PieceFamily::Hill(_))))
        {
            return Err(ConstructionError::Unavailable);
        }
        self.selected_hill_variant = id;
        self.choices(world, obstacles);
        Ok(())
    }

    /// Returns model-compatible placed pieces with their construction validation results.
    pub fn available_choices(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Vec<(Piece, PieceChoice)> {
        self.choices(world, obstacles);
        self.choices_cache
            .borrow()
            .as_ref()
            .expect("choices cache initialized")
            .available_choices
            .iter()
            .filter(|candidate| {
                !matches!(candidate.piece.family, Some(PieceFamily::Hill(_)))
                    || candidate.piece.id == self.selected_hill_variant
            })
            .map(|candidate| (candidate.piece.clone(), candidate.choice))
            .collect()
    }

    /// Returns a candidate piece when its connector and model rules permit it.
    pub fn candidate_piece(&self, id: PieceId) -> Option<Piece> {
        let mut piece = self
            .model_placeable_geometry(template_for(id, self.end()))
            .ok()?;
        if self.chain && self.model.allows_chain(&piece) {
            piece.kind = PieceKind::Lift;
        }
        Some(piece)
    }

    /// Finds the first valid candidate preferred by `prefer`, falling back to
    /// the first valid candidate in catalog order.
    pub fn first_valid_piece(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
        mut prefer: impl FnMut(&Piece) -> bool,
    ) -> Option<PieceId> {
        let mut seen_ids = HashSet::new();
        let mut attempted_preferred = Vec::new();
        for template in templates().iter().filter(|piece| seen_ids.insert(piece.id)) {
            let Some(piece) = self.candidate_piece(template.id) else {
                continue;
            };
            if prefer(&piece) {
                let valid = self.validate(piece.id, world, obstacles).is_ok();
                attempted_preferred.push((piece.id, valid));
                if valid {
                    return Some(piece.id);
                }
            }
        }

        let mut seen_ids = HashSet::new();
        for template in templates().iter().filter(|piece| seen_ids.insert(piece.id)) {
            let Some(piece) = self.candidate_piece(template.id) else {
                continue;
            };
            let valid = attempted_preferred
                .iter()
                .find(|(id, _)| *id == piece.id)
                .map_or_else(
                    || self.validate(piece.id, world, obstacles).is_ok(),
                    |(_, ok)| *ok,
                );
            if valid {
                return Some(piece.id);
            }
        }
        None
    }

    /// Appends a piece, reusing its validation from the choices cache when the cache is
    /// current, and returns whether the placed piece tunnels.
    pub(super) fn append_cached_choice(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ConstructionError> {
        let cached = {
            let cache = self.choices_cache.borrow();
            cache.as_ref().and_then(|cache| {
                (cache.state_hash == self.state_hash()
                    && cache.world_id == world.instance_id()
                    && cache.world_revision == world.revision()
                    && same_obstacles(&cache.obstacles, obstacles))
                .then(|| {
                    let result = cache.choices.iter().find(|choice| choice.id == id)?.result;
                    let candidate = cache
                        .available_choices
                        .iter()
                        .find(|candidate| candidate.piece.id == id)
                        .map(|candidate| (candidate.piece.clone(), candidate.validation.clone()));
                    Some((result, candidate))
                })
                .flatten()
            })
        };
        if let Some((result, candidate)) = cached {
            result?;
            if let Some((piece, Some(validation))) = candidate {
                let would_tunnel = validation.tunnel_plan.would_tunnel();
                self.append_validated_piece(piece, validation);
                return Ok(would_tunnel);
            }
            return Err(ConstructionError::Unavailable);
        }
        self.append_reporting_tunnel(id, world, obstacles)
    }

    pub fn validate(
        &self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let template = template_for(id, self.end());
        let piece = self.placeable(template)?;
        self.validate_piece(&piece, world, obstacles).map(|_| ())
    }

    pub fn tunnel_plan(
        &self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<TunnelPlan, ConstructionError> {
        let template = template_for(id, self.end());
        let piece = self.placeable(template)?;
        self.validate_piece(&piece, world, obstacles)
            .map(|validation| validation.tunnel_plan)
    }

    pub fn tunnel_plans_for_geometry(
        &self,
        pieces: &[Piece],
        world: &World,
    ) -> Result<Vec<TunnelPlan>, ConstructionError> {
        pieces
            .iter()
            .map(|piece| {
                tunnel_plan(world, piece, &self.model.train).map_err(ConstructionError::Clearance)
            })
            .collect()
    }

    fn placeable(&self, template: &Piece) -> Result<Piece, ConstructionError> {
        if self.closed {
            return Err(ConstructionError::Closed);
        }
        self.model_placeable(template)
    }

    fn model_placeable(&self, template: &Piece) -> Result<Piece, ConstructionError> {
        let mut piece = self.model_placeable_geometry(template)?;
        if self.chain && piece.kind == PieceKind::Track {
            if !self.model.allows_chain(&piece) {
                return Err(ConstructionError::ChainNotAllowed);
            }
            piece.kind = PieceKind::Lift;
        }
        Ok(piece)
    }

    fn model_placeable_geometry(&self, template: &Piece) -> Result<Piece, ConstructionError> {
        if !self.model.allows_piece(template) {
            return Err(ConstructionError::ModelForbidden);
        }
        let piece = template
            .at(self.end())
            .ok_or(ConstructionError::Unavailable)?;
        if piece.maximum_curvature() > 1.0 / self.model.minimum_radius_metres {
            return Err(ConstructionError::ModelForbidden);
        }
        Ok(piece)
    }

    pub fn set_launch_intensity(
        &mut self,
        id: usize,
        target_speed: f64,
    ) -> Result<(), ConstructionError> {
        let index = self
            .ids
            .iter()
            .position(|piece_id| *piece_id == id)
            .ok_or(ConstructionError::InvalidIndex)?;
        if self.pieces[index].kind != PieceKind::Launch {
            return Err(ConstructionError::ModelForbidden);
        }
        let bounds = self.launch_speed_bounds(id).expect("launch bounds exist");
        if !target_speed.is_finite()
            || target_speed < bounds.minimum
            || target_speed > bounds.maximum
        {
            return Err(ConstructionError::InvalidLaunchIntensity);
        }
        self.invalidate_state_hash();
        self.pieces[index].launch_target_speed = Some(target_speed);
        *self.validation_cache.get_mut() = None;
        *self.choices_cache.get_mut() = None;
        *self.element_choices_cache.get_mut() = None;
        Ok(())
    }

    pub fn launch_speed_bounds(&self, id: usize) -> Option<LaunchSpeedBounds> {
        let index = self.ids.iter().position(|piece_id| *piece_id == id)?;
        let piece = &self.pieces[index];
        if piece.kind != PieceKind::Launch {
            return None;
        }
        let entry_speed = self.predicted_speed_at(index);
        let maximum = (entry_speed.powi(2)
            + 2.0 * LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED * piece.length())
        .sqrt()
        .min(LAUNCH_MAX_SPEED_METRES_PER_SECOND);
        Some(LaunchSpeedBounds {
            minimum: (entry_speed + 3.0).min(maximum),
            maximum,
        })
    }

    fn predicted_speed_at(&self, piece_index: usize) -> f64 {
        let mut speed: f64 = 0.0;
        let mut previous_height = self.start.position().z;
        for piece in &self.pieces[..piece_index] {
            speed = (speed.powi(2) - 2.0 * 9.81 * (piece.exit.position().z - previous_height))
                .max(0.0)
                .sqrt();
            match piece.kind {
                PieceKind::Launch => {
                    if let Some(target_speed) = piece.launch_target_speed {
                        speed = target_speed;
                    } else {
                        let launch = self
                            .model
                            .train
                            .launch
                            .expect("launch pieces require model launch parameters");
                        speed = (speed.powi(2) + 2.0 * launch.acceleration * piece.length())
                            .sqrt()
                            .min(launch.target_speed);
                    }
                }
                PieceKind::Lift => speed = 3.0,
                _ => {}
            }
            previous_height = piece.exit.position().z;
        }
        speed
    }

    fn validate_piece(
        &self,
        piece: &Piece,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<PieceValidation, ConstructionError> {
        let scenery: Vec<_> = obstacles
            .iter()
            .map(|obstacle| ClearanceBox {
                min: obstacle.min,
                max: obstacle.max,
            })
            .collect();
        let mut cache = self.validation_cache.borrow_mut();
        let matches_inputs = cache.as_ref().is_some_and(|cache| {
            cache.world_id == world.instance_id()
                && cache.world_revision == world.revision()
                && same_obstacles(&cache.obstacles, obstacles)
        });
        if !matches_inputs {
            #[cfg(test)]
            self.validation_cache_rebuilds
                .set(self.validation_cache_rebuilds.get() + 1);
            let existing: Vec<_> = self
                .pieces
                .iter()
                .chain(self.disconnected.iter().flat_map(|segment| &segment.pieces))
                .cloned()
                .collect();
            let existing_tunnel_plans = existing
                .iter()
                .map(|piece| {
                    tunnel_plan(world, piece, &self.model.train)
                        .map_err(ConstructionError::Clearance)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let track_obstacles: Vec<_> = existing
                .iter()
                .map(|piece| {
                    track_support_obstacles(std::slice::from_ref(piece), &self.model.train)
                })
                .collect();
            let supports = vec![None; existing.len()];
            *cache = Some(ValidationCache {
                world_id: world.instance_id(),
                world_revision: world.revision(),
                obstacles: obstacles.to_vec(),
                clearance_index: ClearanceIndex::new(&existing),
                support_regions: existing.iter().map(support_search_region).collect(),
                existing_carve_cells: existing_tunnel_plans
                    .iter()
                    .flat_map(|plan| plan.carve_cells.iter().copied())
                    .collect(),
                existing_carved_world: Arc::new(OnceLock::new()),
                exact_supports: supports.clone(),
                existing,
                existing_tunnel_plans,
                track_obstacles,
                supports,
            });
        }
        let cache = cache.as_mut().expect("validation cache initialized");
        let envelopes = ClearanceEnvelopes::new(piece, &self.model.train);
        let piece_tunnel_plan =
            tunnel_plan_with_envelopes(world, &envelopes).map_err(ConstructionError::Clearance)?;
        let tunnelled = !cache.existing_carve_cells.is_empty() || piece_tunnel_plan.would_tunnel();
        let new_carve_cells: BTreeSet<_> = piece_tunnel_plan
            .carve_cells
            .iter()
            .filter(|cell| !cache.existing_carve_cells.contains(cell))
            .copied()
            .collect();
        let existing_carved_world = Arc::clone(&cache.existing_carved_world);
        let existing_world = |cache: &ValidationCache| -> Result<&World, ConstructionError> {
            if cache.existing_carve_cells.is_empty() {
                return Ok(world);
            }
            existing_carved_world
                .get_or_init(|| carved_world(world, &cache.existing_carve_cells))
                .as_ref()
                .map_err(|error| *error)
        };
        let candidate_carved_world;
        let candidate_world = if !tunnelled {
            world
        } else if new_carve_cells.is_empty() {
            existing_world(cache)?
        } else {
            let mut carve_cells = cache.existing_carve_cells.clone();
            carve_cells.extend(new_carve_cells.iter().copied());
            candidate_carved_world = carved_world(world, &carve_cells)?;
            &candidate_carved_world
        };
        check_clearance_with_envelopes(
            candidate_world,
            piece,
            &envelopes,
            &self.model.train,
            &cache.clearance_index,
            None,
            &scenery,
        )
        .map_err(ConstructionError::Clearance)?;
        let mut end = piece.exit;
        let mut connected = Vec::new();
        loop {
            let matches: Vec<_> = self
                .disconnected
                .iter()
                .enumerate()
                .filter(|(index, segment)| {
                    !connected.contains(index) && segment.pieces[0].entry == end
                })
                .collect();
            if matches.len() > 1 {
                return Err(ConstructionError::AmbiguousConnection);
            }
            let Some((index, segment)) = matches.first() else {
                break;
            };
            connected.push(*index);
            end = segment.pieces.last().expect("nonempty segment").exit;
        }
        if connected.is_empty() {
            check_reverse_track_clearance(&cache.clearance_index, piece, &self.model.train)
                .map_err(ConstructionError::Clearance)?;
        } else {
            let mut all_pieces = cache.existing.clone();
            all_pieces.push(piece.clone());
            let index_for_recheck = ClearanceIndex::new(&all_pieces);
            for (index, retained_piece) in all_pieces.iter().enumerate() {
                check_clearance_indexed(
                    candidate_world,
                    retained_piece,
                    &self.model.train,
                    &index_for_recheck,
                    Some(index),
                    &scenery,
                )
                .map_err(ConstructionError::Clearance)?;
            }
        }
        let candidate_bounds =
            track_support_obstacles(std::slice::from_ref(piece), &self.model.train);
        let candidate_region = support_search_region(piece);
        let existing_count = cache.existing.len();
        let mut validated_supports = Vec::with_capacity(existing_count + 1);
        for index in 0..=existing_count {
            let (supported_piece, region) = if index < existing_count {
                (&cache.existing[index], cache.support_regions[index])
            } else {
                (piece, candidate_region)
            };
            let route_supports = if self.model.support_style_for_piece(supported_piece)
                == SupportStyle::DenseWoodenLattice
            {
                generate_dense_supports
            } else {
                generate_supports
            };
            // Obstacles from the other existing pieces near this piece's support search region.
            let existing_support_obstacles = || {
                let mut support_obstacles = obstacles.to_vec();
                for (other_index, other_bounds) in cache.track_obstacles.iter().enumerate() {
                    if index != other_index {
                        support_obstacles.extend(
                            other_bounds
                                .iter()
                                .copied()
                                .filter(|bounds| region.overlaps(bounds)),
                        );
                    }
                }
                support_obstacles
            };
            if tunnelled {
                // Supports read the world and obstacles only inside their search region, so
                // a piece the candidate's bounds and new excavation cannot reach keeps the
                // supports routed without the candidate.
                let candidate_reaches_piece = candidate_bounds
                    .iter()
                    .any(|bounds| region.overlaps(bounds))
                    || new_carve_cells
                        .iter()
                        .any(|cell| region.overlaps_tile(cell));
                if index < existing_count && !candidate_reaches_piece {
                    if cache.exact_supports[index].is_none() {
                        let routed = route_supports(
                            supported_piece,
                            existing_world(cache)?,
                            &existing_support_obstacles(),
                        );
                        cache.exact_supports[index] = Some(routed);
                    }
                    let exact = cache.exact_supports[index]
                        .clone()
                        .expect("exact supports routed above");
                    validated_supports.push(exact.map_err(construction_support_error)?);
                    continue;
                }
                let mut support_obstacles = existing_support_obstacles();
                if index < existing_count {
                    support_obstacles.extend(
                        candidate_bounds
                            .iter()
                            .copied()
                            .filter(|bounds| region.overlaps(bounds)),
                    );
                }
                validated_supports.push(
                    route_supports(supported_piece, candidate_world, &support_obstacles)
                        .map_err(construction_support_error)?,
                );
                continue;
            }
            if index < existing_count && cache.supports[index].is_none() {
                #[cfg(test)]
                self.support_cache_misses
                    .set(self.support_cache_misses.get() + 1);
                if cache.exact_supports[index].is_none() {
                    cache.exact_supports[index] = Some(route_supports(
                        supported_piece,
                        world,
                        &existing_support_obstacles(),
                    ));
                }
                cache.supports[index] = cache.exact_supports[index].clone();
            }
            if let Some(Some(Ok(supports))) = cache.supports.get(index) {
                if !supports.iter().any(|support| {
                    candidate_bounds.iter().any(|bounds| {
                        support_bounds_overlap(support, bounds)
                            && support_intersects_obstacle(support, bounds)
                    })
                }) {
                    validated_supports.push(supports.clone());
                    continue;
                }
            }
            if let Some(Some(Err(error))) = cache.supports.get(index) {
                if !candidate_bounds
                    .iter()
                    .any(|bounds| region.overlaps(bounds))
                {
                    return Err(construction_support_error(*error));
                }
            }
            let mut support_obstacles = existing_support_obstacles();
            if index < existing_count {
                support_obstacles.extend(
                    candidate_bounds
                        .iter()
                        .copied()
                        .filter(|bounds| region.overlaps(bounds)),
                );
            }
            let supports = route_supports(supported_piece, world, &support_obstacles)
                .map_err(construction_support_error)?;
            validated_supports.push(supports);
        }
        Ok(PieceValidation {
            supports: validated_supports,
            tunnel_plan: piece_tunnel_plan,
            track_obstacles: candidate_bounds,
        })
    }

    pub fn append(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        self.append_reporting_tunnel(id, world, obstacles)
            .map(|_| ())
    }

    fn append_reporting_tunnel(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<bool, ConstructionError> {
        let template = template_for(id, self.end());
        let piece = self.placeable(template)?;
        let validation = self.validate_piece(&piece, world, obstacles)?;
        let would_tunnel = validation.tunnel_plan.would_tunnel();
        self.append_validated_piece(piece, validation);
        Ok(would_tunnel)
    }

    fn append_validated_piece(&mut self, piece: Piece, validation: PieceValidation) {
        if self.disconnected.is_empty() {
            let cache = self
                .validation_cache
                .get_mut()
                .as_mut()
                .expect("validation cache initialized");
            let piece_bounds = validation.track_obstacles;
            let tunnelled =
                !cache.existing_carve_cells.is_empty() || validation.tunnel_plan.would_tunnel();
            if tunnelled {
                // A tunnelling validation routed every piece exactly on the world carved for
                // the new track, or reused routes the new piece cannot reach.
                cache.exact_supports = validation
                    .supports
                    .iter()
                    .map(|supports| Some(Ok(supports.clone())))
                    .collect();
            } else {
                for (exact, region) in cache.exact_supports.iter_mut().zip(&cache.support_regions) {
                    if piece_bounds.iter().any(|bounds| region.overlaps(bounds)) {
                        *exact = None;
                    }
                }
                let piece_supports = validation
                    .supports
                    .last()
                    .expect("validation routes the new piece")
                    .clone();
                cache.exact_supports.push(Some(Ok(piece_supports)));
            }
            let carve_cells_before = cache.existing_carve_cells.len();
            cache
                .existing_carve_cells
                .extend(validation.tunnel_plan.carve_cells.iter().copied());
            if cache.existing_carve_cells.len() != carve_cells_before {
                cache.existing_carved_world = Arc::new(OnceLock::new());
            }
            cache.support_regions.push(support_search_region(&piece));
            cache.existing_tunnel_plans.push(validation.tunnel_plan);
            cache.track_obstacles.push(piece_bounds);
            cache.existing.push(piece.clone());
            cache.clearance_index.push(&piece);
            cache.supports = validation
                .supports
                .into_iter()
                .map(|support| Some(Ok(support)))
                .collect();
        } else {
            *self.validation_cache.get_mut() = None;
        }
        *self.choices_cache.get_mut() = None;
        self.invalidate_state_hash();
        self.pieces.push(piece);
        self.ids.push(self.next_id);
        self.next_id += 1;
        while let Some(index) = self
            .disconnected
            .iter()
            .position(|segment| segment.pieces[0].entry == self.end())
        {
            let segment = self.disconnected.remove(index);
            self.ids.extend(segment.ids);
            self.pieces.extend(segment.pieces);
        }
    }

    pub fn delete_from(&mut self, id: usize) -> Result<Vec<Piece>, ConstructionError> {
        self.invalidate_state_hash();
        let old_ids: Vec<_> = self
            .ids
            .iter()
            .chain(self.disconnected.iter().flat_map(|segment| &segment.ids))
            .copied()
            .collect();
        if let Some(index) = self.ids.iter().position(|&piece_id| piece_id == id) {
            let suffix_pieces = self.pieces.split_off(index + 1);
            let suffix_ids = self.ids.split_off(index + 1);
            let removed = self.pieces.pop().expect("piece matching id");
            self.ids.pop();
            if !suffix_pieces.is_empty() {
                self.disconnected.push(TrackSegment {
                    ids: suffix_ids,
                    pieces: suffix_pieces,
                });
            }
            self.closed = false;
            *self.choices_cache.get_mut() = None;
            self.reorder_cache_after_deletion(&old_ids);
            return Ok(vec![removed]);
        }
        let segment_index = self
            .disconnected
            .iter()
            .position(|segment| segment.ids.contains(&id))
            .ok_or(ConstructionError::InvalidIndex)?;
        let segment = &mut self.disconnected[segment_index];
        let index = segment
            .ids
            .iter()
            .position(|&piece_id| piece_id == id)
            .expect("piece id exists");
        let suffix_pieces = segment.pieces.split_off(index + 1);
        let suffix_ids = segment.ids.split_off(index + 1);
        let removed = segment.pieces.pop().expect("piece matching id");
        segment.ids.pop();
        if segment.pieces.is_empty() {
            self.disconnected.remove(segment_index);
        }
        if !suffix_pieces.is_empty() {
            self.disconnected.push(TrackSegment {
                ids: suffix_ids,
                pieces: suffix_pieces,
            });
        }
        *self.choices_cache.get_mut() = None;
        self.reorder_cache_after_deletion(&old_ids);
        Ok(vec![removed])
    }

    fn reorder_cache_after_deletion(&mut self, old_ids: &[usize]) {
        let Some(cache) = self.validation_cache.get_mut().as_mut() else {
            return;
        };
        let removed_index = old_ids
            .iter()
            .position(|id| {
                !self.ids.contains(id)
                    && !self
                        .disconnected
                        .iter()
                        .any(|segment| segment.ids.contains(id))
            })
            .expect("deleted piece has previous id");
        let removed_bounds = &cache.track_obstacles[removed_index];
        let remaining_carve_cells: BTreeSet<_> = cache
            .existing_tunnel_plans
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed_index)
            .flat_map(|(_, plan)| plan.carve_cells.iter().copied())
            .collect();
        if remaining_carve_cells != cache.existing_carve_cells {
            cache.existing_carve_cells = remaining_carve_cells;
            cache.existing_carved_world = Arc::new(OnceLock::new());
            cache.exact_supports.fill(None);
        }
        for (exact, region) in cache.exact_supports.iter_mut().zip(&cache.support_regions) {
            if removed_bounds.iter().any(|bounds| region.overlaps(bounds)) {
                *exact = None;
            }
        }
        for (index, result) in cache.supports.iter_mut().enumerate() {
            let affected = removed_bounds
                .iter()
                .any(|bounds| support_search_may_intersect(&cache.existing[index], bounds));
            if affected
                && result.as_ref().is_some_and(|result| match result {
                    Err(_) => true,
                    Ok(supports) => supports.iter().any(|support| {
                        let dx = support.footing.x - support.anchor.x;
                        let dy = support.footing.y - support.anchor.y;
                        (dx - dx.round()).abs() > 1e-9 || (dy - dy.round()).abs() > 1e-9
                    }),
                })
            {
                *result = None;
            }
        }
        let indices: Vec<_> = self
            .ids
            .iter()
            .chain(self.disconnected.iter().flat_map(|segment| &segment.ids))
            .map(|id| {
                old_ids
                    .iter()
                    .position(|old_id| old_id == id)
                    .expect("retained piece has previous id")
            })
            .collect();
        cache.existing = indices
            .iter()
            .map(|&index| cache.existing[index].clone())
            .collect();
        cache.clearance_index.reorder(&indices);
        cache.track_obstacles = indices
            .iter()
            .map(|&index| cache.track_obstacles[index].clone())
            .collect();
        cache.existing_tunnel_plans = indices
            .iter()
            .map(|&index| cache.existing_tunnel_plans[index].clone())
            .collect();
        cache.supports = indices
            .iter()
            .map(|&index| cache.supports[index].clone())
            .collect();
        cache.exact_supports = indices
            .iter()
            .map(|&index| cache.exact_supports[index].clone())
            .collect();
        cache.support_regions = indices
            .iter()
            .map(|&index| cache.support_regions[index])
            .collect();
    }

    pub fn close(&mut self) -> Result<(), ConstructionError> {
        Circuit::new(self.pieces.clone()).map_err(|_| ConstructionError::InvalidCircuit)?;
        *self.choices_cache.get_mut() = None;
        self.invalidate_state_hash();
        self.closed = true;
        Ok(())
    }

    pub fn circuit(&self) -> Result<Circuit, ConstructionError> {
        if !self.closed {
            return Err(ConstructionError::NotClosed);
        }
        Circuit::new(self.pieces.clone()).map_err(|_| ConstructionError::InvalidCircuit)
    }
}

fn same_geometry(first: &Piece, second: &Piece) -> bool {
    first.entry == second.entry
        && first.exit == second.exit
        && first.shape == second.shape
        && first.tangent_scale == second.tangent_scale
        && first.mirrored == second.mirrored
}

fn construction_support_error(error: SupportError) -> ConstructionError {
    match error {
        SupportError::NoRoute => ConstructionError::NoSupportRoute,
    }
}

#[derive(Clone, Copy)]
struct SupportSearchRegion {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl SupportSearchRegion {
    fn overlaps(&self, obstacle: &SupportObstacle) -> bool {
        obstacle.max.x >= self.min_x
            && obstacle.min.x <= self.max_x
            && obstacle.max.y >= self.min_y
            && obstacle.min.y <= self.max_y
    }

    fn overlaps_tile(&self, cell: &TerrainCell) -> bool {
        let west = f64::from(cell.x) * 4.0;
        let north = f64::from(cell.y) * 4.0;
        west + 4.0 >= self.min_x
            && west <= self.max_x
            && north + 4.0 >= self.min_y
            && north <= self.max_y
    }
}

/// Returns `world` with `cells` carved, failing as a placement would if carving them is refused.
fn carved_world(world: &World, cells: &BTreeSet<TerrainCell>) -> Result<World, ConstructionError> {
    let cells: Vec<_> = cells
        .iter()
        .map(|cell| (cell.x, cell.y, cell.height))
        .collect();
    let (carved, invalidated) = world
        .with_cells_carved(&cells)
        .map_err(|_| ConstructionError::Clearance(ClearanceCollision::Scenery))?;
    if !invalidated.is_empty() {
        return Err(ConstructionError::Clearance(ClearanceCollision::Path));
    }
    Ok(carved)
}

fn support_search_region(piece: &Piece) -> SupportSearchRegion {
    let mut region = SupportSearchRegion {
        min_x: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        min_y: f64::INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for spacing in [4.0, 8.0] {
        let count = (piece.length() / spacing).ceil() as usize;
        for index in 0..count {
            let anchor = piece
                .sample(piece.length() * (index as f64 + 0.5) / count as f64)
                .position;
            region.min_x = region.min_x.min(anchor.x - SUPPORT_INFLUENCE_RADIUS_METRES);
            region.max_x = region.max_x.max(anchor.x + SUPPORT_INFLUENCE_RADIUS_METRES);
            region.min_y = region.min_y.min(anchor.y - SUPPORT_INFLUENCE_RADIUS_METRES);
            region.max_y = region.max_y.max(anchor.y + SUPPORT_INFLUENCE_RADIUS_METRES);
        }
    }
    region
}

fn support_search_may_intersect(piece: &Piece, obstacle: &SupportObstacle) -> bool {
    support_search_region(piece).overlaps(obstacle)
}

fn same_obstacles(first: &[SupportObstacle], second: &[SupportObstacle]) -> bool {
    first.len() == second.len()
        && first
            .iter()
            .zip(second)
            .all(|(a, b)| a.min == b.min && a.max == b.max)
}

fn support_bounds_overlap(support: &Support, obstacle: &SupportObstacle) -> bool {
    let padding = 0.4;
    obstacle.max.x >= support.anchor.x.min(support.footing.x) - padding
        && obstacle.min.x <= support.anchor.x.max(support.footing.x) + padding
        && obstacle.max.y >= support.anchor.y.min(support.footing.y) - padding
        && obstacle.min.y <= support.anchor.y.max(support.footing.y) + padding
        && obstacle.max.z >= support.anchor.z.min(support.footing.z) - padding
        && obstacle.min.z <= support.anchor.z.max(support.footing.z) + padding
}

/// Returns conservative track and train bounds for support routing.
pub fn track_support_obstacles(
    pieces: &[Piece],
    train: &TrainConfiguration,
) -> Vec<SupportObstacle> {
    let mut obstacles = Vec::new();
    for piece in pieces {
        let segments = (piece.length() / 2.0).ceil() as usize;
        for segment in 0..segments {
            let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut max = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for step in 0..=4 {
                let fraction = (segment as f64 + f64::from(step) / 4.0) / segments as f64;
                let distance = piece.length() * fraction;
                let sample = piece.sample(distance);
                let left = sample.up.cross(sample.tangent);
                for side in [-1.0, 1.0] {
                    for height in [0.0, train.height_metres] {
                        let point = sample.position
                            + left * (side * train.width_metres / 2.0)
                            + sample.up * height;
                        min = Vec3::new(min.x.min(point.x), min.y.min(point.y), min.z.min(point.z));
                        max = Vec3::new(max.x.max(point.x), max.y.max(point.y), max.z.max(point.z));
                    }
                }
            }
            let padding = Vec3::new(0.2, 0.2, 0.2);
            obstacles.push(SupportObstacle {
                min: min - padding,
                max: max + padding,
            });
        }
    }
    obstacles
}

fn templates() -> &'static [Piece] {
    catalog()
}

fn template_for(id: PieceId, end: TrackState) -> &'static Piece {
    let mut matching = templates().iter().filter(|piece| piece.id == id);
    let first = matching.next().expect("catalog contains every piece id");
    if first.entry.slope == end.slope
        && first.entry.bank == end.bank
        && first.entry.horizontal_curvature == end.horizontal_curvature
        && first.entry.vertical_curvature == end.vertical_curvature
    {
        return first;
    }
    matching
        .find(|piece| {
            piece.entry.slope == end.slope
                && piece.entry.bank == end.bank
                && piece.entry.horizontal_curvature == end.horizontal_curvature
                && piece.entry.vertical_curvature == end.vertical_curvature
        })
        .unwrap_or(first)
}
