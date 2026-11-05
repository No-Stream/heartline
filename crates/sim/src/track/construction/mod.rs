use crate::physics::Circuit;
use crate::state_hash::StateHasher;
use crate::track::clearance::{
    ClearanceBox, ClearanceCollision, ClearanceIndex, TunnelPlan, check_clearance_indexed,
    check_reverse_track_clearance, tunnel_plan,
};
use crate::track::supports::{
    SUPPORT_INFLUENCE_RADIUS_METRES, Support, SupportError, SupportObstacle,
    generate_dense_supports, generate_supports, support_intersects_obstacle,
};
use crate::track::{
    CoasterColors, CoasterModel, LAUNCH_MAX_ACCELERATION_METRES_PER_SECOND_SQUARED,
    LAUNCH_MAX_SPEED_METRES_PER_SECOND, LaunchSpeedBounds, Piece, PieceId, PieceKind, Shape,
    SupportStyle, TrackState, TrainConfiguration, Vec3, catalog,
};
use crate::world::World;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};

#[cfg(test)]
mod tests;

mod completion;
mod elements;
#[cfg(test)]
mod flexibility_tests;
#[cfg(test)]
mod walk_tests;
pub use completion::{Completion, CompletionSearch, CompletionSearchProgress};
pub use elements::ElementChoice;

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

/// Select the rated candidate with the largest margin to either window edge.
/// If no window contains the speed, select the nearest window instead. Ties
/// preserve the order supplied by the caller.
pub fn select_speed_matched_variant(
    candidates: &[Piece],
    entry_speed: f64,
) -> Option<SpeedMatchedVariant> {
    assert!(entry_speed.is_finite() && entry_speed >= 0.0);
    let mut best: Option<(SpeedMatchedVariant, f64)> = None;
    for piece in candidates {
        let Some(window) = piece.rated_speeds else {
            continue;
        };
        assert!(window.minimum_entry.is_finite() && window.maximum_entry.is_finite());
        assert!(window.minimum_entry >= 0.0 && window.minimum_entry <= window.maximum_entry);
        let margin = (entry_speed - window.minimum_entry).min(window.maximum_entry - entry_speed);
        let choice = SpeedMatchedVariant {
            id: piece.id,
            fits: margin >= 0.0,
        };
        if best
            .as_ref()
            .is_none_or(|(_, best_margin)| margin > *best_margin)
        {
            best = Some((choice, margin));
        }
    }
    best.map(|(choice, _)| choice)
}

#[derive(Clone)]
struct PieceValidation {
    supports: Vec<Vec<Support>>,
    tunnel_plan: TunnelPlan,
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
    colors: CoasterColors,
    #[serde(skip)]
    validation_cache: RefCell<Option<ValidationCache>>,
    #[serde(skip)]
    choices_cache: RefCell<Option<ChoicesCache>>,
    #[serde(skip)]
    element_choices_cache: RefCell<Option<elements::ElementChoicesCache>>,
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
            colors,
            validation_cache: RefCell::new(None),
            choices_cache: RefCell::new(None),
            element_choices_cache: RefCell::new(None),
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
        self.colors = colors;
    }
    pub fn set_chain(&mut self, enabled: bool) {
        if self.chain != enabled {
            self.chain = enabled;
            *self.choices_cache.get_mut() = None;
        }
    }
    pub fn model(&self) -> &CoasterModel {
        &self.model
    }

    pub fn next_piece_id(&self) -> usize {
        self.next_id
    }

    /// Stable fingerprint of the editable track state, excluding derived validation caches.
    pub fn state_hash(&self) -> u64 {
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
            if cache.world_id == world.instance_id()
                && cache.world_revision == world.revision()
                && same_obstacles(&cache.obstacles, obstacles)
            {
                return cache.choices.clone();
            }
        }
        let mut choices = Vec::new();
        let mut available_choices: Vec<AvailableChoice> = Vec::new();
        let mut seen_ids = HashSet::new();
        for base in templates().iter().filter(|piece| seen_ids.insert(piece.id)) {
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
            world_id: world.instance_id(),
            world_revision: world.revision(),
            obstacles: obstacles.to_vec(),
            choices: choices.clone(),
            available_choices,
        });
        choices
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

    pub(super) fn append_cached_choice(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let cached = {
            let cache = self.choices_cache.borrow();
            cache.as_ref().and_then(|cache| {
                (cache.world_id == world.instance_id()
                    && cache.world_revision == world.revision()
                    && same_obstacles(&cache.obstacles, obstacles))
                .then(|| {
                    let result = cache
                        .choices
                        .iter()
                        .find(|choice| choice.id == id)
                        .expect("catalog contains every piece id")
                        .result;
                    let candidate = cache
                        .available_choices
                        .iter()
                        .find(|candidate| candidate.piece.id == id)
                        .map(|candidate| (candidate.piece.clone(), candidate.validation.clone()));
                    (result, candidate)
                })
            })
        };
        if let Some((result, candidate)) = cached {
            result?;
            if let Some((piece, Some(validation))) = candidate {
                self.append_validated_piece(piece, validation);
                return Ok(());
            }
            return Err(ConstructionError::Unavailable);
        }
        self.append(id, world, obstacles)
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
                existing,
                existing_tunnel_plans,
                track_obstacles,
                supports,
            });
        }
        let cache = cache.as_mut().expect("validation cache initialized");
        let mut all_pieces = cache.existing.clone();
        all_pieces.push(piece.clone());
        let piece_tunnel_plan =
            tunnel_plan(world, piece, &self.model.train).map_err(ConstructionError::Clearance)?;
        let mut carve_cells: BTreeSet<_> = cache
            .existing_tunnel_plans
            .iter()
            .flat_map(|plan| plan.carve_cells.iter().copied())
            .collect();
        carve_cells.extend(piece_tunnel_plan.carve_cells.iter().copied());
        let mut carved_world = None;
        if !carve_cells.is_empty() {
            let mut candidate_world = world.clone();
            let cells = carve_cells
                .iter()
                .map(|cell| (cell.x, cell.y, cell.height))
                .collect::<Vec<_>>();
            let invalidated = candidate_world
                .carve_cells(&cells)
                .map_err(|_| ConstructionError::Clearance(ClearanceCollision::Scenery))?;
            if !invalidated.is_empty() {
                return Err(ConstructionError::Clearance(ClearanceCollision::Path));
            }
            carved_world = Some(candidate_world);
        }
        let candidate_world = carved_world.as_ref().unwrap_or(world);
        check_clearance_indexed(
            candidate_world,
            piece,
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
        let mut validated_supports = Vec::with_capacity(all_pieces.len());
        for (index, supported_piece) in all_pieces.iter().enumerate() {
            let route_supports = if self.model.support_style_for_piece(supported_piece)
                == SupportStyle::DenseWoodenLattice
            {
                generate_dense_supports
            } else {
                generate_supports
            };
            if !carve_cells.is_empty() {
                let mut support_obstacles = obstacles.to_vec();
                let region = support_search_region(supported_piece);
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
                if index < cache.existing.len() {
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
            if index < cache.existing.len() && cache.supports[index].is_none() {
                let mut bounds = obstacles.to_vec();
                let region = support_search_region(supported_piece);
                for (other_index, other_bounds) in cache.track_obstacles.iter().enumerate() {
                    if index != other_index {
                        bounds.extend(
                            other_bounds
                                .iter()
                                .copied()
                                .filter(|bounds| region.overlaps(bounds)),
                        );
                    }
                }
                cache.supports[index] = Some(route_supports(supported_piece, world, &bounds));
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
                    .any(|bounds| support_search_may_intersect(supported_piece, bounds))
                {
                    return Err(construction_support_error(*error));
                }
            }
            let mut support_obstacles = obstacles.to_vec();
            let region = support_search_region(supported_piece);
            for (other_index, bounds) in cache.track_obstacles.iter().enumerate() {
                if index != other_index {
                    support_obstacles.extend(
                        bounds
                            .iter()
                            .copied()
                            .filter(|bounds| region.overlaps(bounds)),
                    );
                }
            }
            if index < cache.existing.len() {
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
        })
    }

    pub fn append(
        &mut self,
        id: PieceId,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        let template = template_for(id, self.end());
        let piece = self.placeable(template)?;
        let validation = self.validate_piece(&piece, world, obstacles)?;
        self.append_validated_piece(piece, validation);
        Ok(())
    }

    fn append_validated_piece(&mut self, piece: Piece, validation: PieceValidation) {
        if self.disconnected.is_empty() {
            let cache = self
                .validation_cache
                .get_mut()
                .as_mut()
                .expect("validation cache initialized");
            cache.existing_tunnel_plans.push(validation.tunnel_plan);
            cache.track_obstacles.push(track_support_obstacles(
                std::slice::from_ref(&piece),
                &self.model.train,
            ));
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
    }

    pub fn close(&mut self) -> Result<(), ConstructionError> {
        Circuit::new(self.pieces.clone()).map_err(|_| ConstructionError::InvalidCircuit)?;
        *self.choices_cache.get_mut() = None;
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
