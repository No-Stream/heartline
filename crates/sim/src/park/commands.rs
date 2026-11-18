use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};

use super::Park;
use super::tick::{generate_supports, render_cached_supports};
use crate::facilities::{
    FacilityKind, PathOpening, PlacementError, coaster_access_openings,
    validate_facility_path_access,
};
use crate::physics::Circuit;
use crate::track::clearance::{ClearanceBox, ClearanceCollision, TunnelPlan, check_clearance};
use crate::track::construction::{Construction, track_support_obstacles};
use crate::track::supports::SupportObstacle;
use crate::track::{Piece, PieceId, TrainConfiguration, hybrid_model};
use crate::world::{
    CellEditError, Direction, Ground, PathBuildError, PathKind, PathPreviewTile, Position, Scenery,
    SceneryPlacementError, TerrainCommand, TerrainEditError, TerrainEditResult, TerrainTile, World,
};

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkCommandError {
    Facility(PlacementError),
    Path(PathBuildError),
    Terrain(TerrainEditError),
    Scenery(SceneryPlacementError),
    FacilityIntersectsTrack,
    PathMissing,
    SceneryMissing,
    SceneryIntersectsFacility,
    SceneryIntersectsTrack,
    ScenerySupportRoute,
    TerrainUnderBuilding,
    TerrainTrackClearance,
    TerrainSupportRoute,
}

impl Display for ParkCommandError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ParkCommandError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunnelCarveError {
    InvalidPlan,
    PathObstructed,
    SceneryUnsupported,
}

impl Display for TunnelCarveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidPlan => "The tunnel plan is no longer valid.",
            Self::PathObstructed => "The tunnel would cut through a path.",
            Self::SceneryUnsupported => "The tunnel would undermine scenery.",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TunnelCarveError {}

impl Park {
    pub fn place_facility(
        &mut self,
        kind: FacilityKind,
        position: Position,
        direction: Direction,
    ) -> Result<u32, ParkCommandError> {
        if self.demo_coaster.as_ref().is_some_and(|coaster| {
            facility_intersects_track(
                kind,
                position,
                direction,
                coaster.circuit.pieces(),
                &coaster.train_configuration,
            )
        }) || self.construction.as_ref().is_some_and(|construction| {
            construction_intersects_facility(kind, position, direction, construction)
        }) {
            return Err(ParkCommandError::FacilityIntersectsTrack);
        }
        let id = self
            .facilities
            .place(&self.world, kind, position, direction)
            .map_err(ParkCommandError::Facility)?;
        self.support_render_cache.clear();
        Ok(id)
    }

    pub fn remove_facility(&mut self, id: u32) -> Result<(), ParkCommandError> {
        if !self.facilities.remove(id) {
            return Err(ParkCommandError::Facility(PlacementError::UnknownBuilding));
        }
        self.support_render_cache.clear();
        Ok(())
    }

    pub fn set_facility_price(&mut self, id: u32, price: u16) -> Result<(), ParkCommandError> {
        self.facilities
            .set_price(id, price)
            .map_err(ParkCommandError::Facility)
    }

    pub fn facility_openings(&self) -> Vec<PathOpening> {
        let mut openings = self.facilities.access_openings(&self.world);
        if let Some(coaster_openings) = self.construction.as_ref().and_then(coaster_access_openings)
        {
            openings.extend(coaster_openings);
        }
        openings
    }

    pub fn preview_path_route(
        &self,
        start: Position,
        end: Position,
        kind: PathKind,
        style: u8,
    ) -> Result<Vec<PathPreviewTile>, ParkCommandError> {
        let mut preview = self
            .world
            .preview_path_tiles(start, end, kind, style)
            .map_err(ParkCommandError::Path)?;
        let mut blocked_by_facility = false;
        for tile in &mut preview {
            if self.facilities.occupies_path_position(tile.path.position) {
                tile.refusal = Some(PathBuildError::FacilityOccupied);
                blocked_by_facility = true;
            } else if blocked_by_facility && tile.refusal.is_none() {
                tile.refusal = Some(PathBuildError::Disconnected);
            }
        }
        if preview.iter().all(|tile| tile.refusal.is_none()) {
            let proposed = self
                .world
                .preview_path_world(start, end, kind)
                .map_err(ParkCommandError::Path)?;
            if validate_facility_path_access(
                &proposed,
                &self.facilities,
                self.construction.as_ref(),
            )
            .is_err()
            {
                for tile in &mut preview {
                    tile.refusal = Some(PathBuildError::FacilityAccess);
                }
            }
        }
        Ok(preview)
    }

    pub fn build_path_route(
        &mut self,
        start: Position,
        end: Position,
        kind: PathKind,
        style: u8,
    ) -> Result<(), ParkCommandError> {
        let preview = self.preview_path_route(start, end, kind, style)?;
        if let Some(error) = preview.iter().find_map(|tile| tile.refusal) {
            return Err(ParkCommandError::Path(error));
        }
        self.world
            .build_path_route(start, end, kind, style)
            .map_err(ParkCommandError::Path)?;
        Ok(())
    }

    pub fn remove_path(&mut self, position: Position) -> Result<(), ParkCommandError> {
        if self.world.path(position).is_none() {
            return Err(ParkCommandError::PathMissing);
        }
        self.world.remove_path(position);
        Ok(())
    }

    pub fn place_scenery(&mut self, scenery: Scenery) -> Result<(), ParkCommandError> {
        let (width, height) = self.world.dimensions();
        if scenery.position.x >= width || scenery.position.y >= height {
            return Err(ParkCommandError::Scenery(
                SceneryPlacementError::OutsideWorld,
            ));
        }
        if scenery.quarter > 3 || scenery.rotation > 3 {
            return Err(ParkCommandError::Scenery(
                SceneryPlacementError::InvalidOrientation,
            ));
        }
        let bounds = scenery.clearance_box();
        if self
            .facility_obstacles()
            .iter()
            .any(|obstacle| boxes_overlap(bounds, obstacle_box(*obstacle)))
        {
            return Err(ParkCommandError::SceneryIntersectsFacility);
        }
        if self.demo_coaster.as_ref().is_some_and(|coaster| {
            coaster
                .circuit
                .pieces()
                .iter()
                .any(|piece| scenery_intersects_track(bounds, piece, &coaster.train_configuration))
        }) || self.construction.as_ref().is_some_and(|construction| {
            construction
                .pieces()
                .iter()
                .any(|piece| scenery_intersects_track(bounds, piece, &construction.model().train))
                || construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| &segment.pieces)
                    .any(|piece| {
                        scenery_intersects_track(bounds, piece, &construction.model().train)
                    })
        }) {
            return Err(ParkCommandError::SceneryIntersectsTrack);
        }

        let mut candidate = self.world.clone();
        candidate
            .place_scenery(scenery)
            .map_err(ParkCommandError::Scenery)?;
        if self.support_route_worsened_by_scenery(&candidate) {
            return Err(ParkCommandError::ScenerySupportRoute);
        }
        self.world
            .place_scenery(scenery)
            .expect("validated scenery placement remains valid");
        self.support_render_cache.clear();
        Ok(())
    }

    pub fn remove_scenery(
        &mut self,
        position: Position,
        quarter: u8,
    ) -> Result<(), ParkCommandError> {
        if !self.world.remove_scenery(position, quarter) {
            return Err(ParkCommandError::SceneryMissing);
        }
        self.support_render_cache.clear();
        Ok(())
    }

    pub fn apply_terrain(
        &mut self,
        command: TerrainCommand,
        selected_piece: Option<PieceId>,
    ) -> Result<TerrainEditResult, ParkCommandError> {
        let mut candidate = self.world.clone();
        let candidate_result = candidate
            .apply_terrain(command)
            .map_err(ParkCommandError::Terrain)?;
        if candidate_result.changed_tiles.iter().any(|tile| {
            self.facilities
                .buildings()
                .iter()
                .any(|building| tile_in_building_footprint(*tile, building))
        }) {
            return Err(ParkCommandError::TerrainUnderBuilding);
        }
        self.reject_terrain_that_blocks_track(&candidate, selected_piece)?;
        let result = self
            .world
            .apply_terrain(command)
            .expect("validated terrain command remains valid");
        assert_eq!(result, candidate_result, "terrain preflight result changed");
        self.note_manual_tunnel_edit(command);
        self.support_render_cache.clear();
        Ok(result)
    }

    pub fn construction_tunnel_generation(&self) -> u64 {
        self.construction_tunnel_generation
    }

    pub fn next_construction_tunnel_generation(&self) -> u64 {
        self.construction_tunnel_generation
            .checked_add(1)
            .expect("construction tunnel generation overflow")
    }

    pub fn carve_for_construction(
        &mut self,
        plans: &[(usize, TunnelPlan)],
    ) -> Result<(), TunnelCarveError> {
        self.carve_for_construction_at(self.construction_tunnel_generation, plans)
    }

    pub fn carve_for_construction_at(
        &mut self,
        generation: u64,
        plans: &[(usize, TunnelPlan)],
    ) -> Result<(), TunnelCarveError> {
        let carve_cells: std::collections::BTreeSet<_> = plans
            .iter()
            .flat_map(|(_, plan)| plan.carve_cells.iter().copied())
            .collect();
        let cells: Vec<_> = carve_cells
            .iter()
            .map(|cell| (cell.x, cell.y, cell.height))
            .collect();
        if !cells.is_empty() {
            let (candidate, invalidated_paths) =
                self.world
                    .with_cells_carved(&cells)
                    .map_err(|error| match error {
                        CellEditError::InvalidCell => TunnelCarveError::InvalidPlan,
                        CellEditError::UnsupportedScenery => TunnelCarveError::SceneryUnsupported,
                    })?;
            if !invalidated_paths.is_empty() {
                return Err(TunnelCarveError::PathObstructed);
            }
            self.world.commit_candidate(candidate);
        }
        for (piece_id, plan) in plans {
            let owner = super::TunnelOwner {
                generation,
                piece_id: *piece_id,
            };
            for cell in &plan.swept_cells {
                if carve_cells.contains(cell) || self.automatic_tunnel_owners.contains_key(cell) {
                    self.automatic_tunnel_owners
                        .entry(*cell)
                        .or_default()
                        .insert(owner);
                }
            }
        }
        self.support_render_cache.clear();
        Ok(())
    }

    pub fn activate_construction_tunnel_generation(&mut self, generation: u64) {
        assert!(
            generation >= self.construction_tunnel_generation,
            "construction tunnel generations must increase"
        );
        self.construction_tunnel_generation = generation;
        self.restore_tunnel_owners(|owner| owner.generation < generation);
    }

    pub fn restore_tunnel_piece(&mut self, piece_id: usize) {
        let generation = self.construction_tunnel_generation;
        self.restore_tunnel_owners(|owner| {
            owner.generation == generation && owner.piece_id == piece_id
        });
    }

    pub fn restore_tunnel_piece_at(&mut self, generation: u64, piece_id: usize) {
        self.restore_tunnel_owners(|owner| {
            owner.generation == generation && owner.piece_id == piece_id
        });
    }

    fn restore_tunnel_owners(&mut self, mut remove_owner: impl FnMut(super::TunnelOwner) -> bool) {
        let mut cells_to_restore = Vec::new();
        let mut cells_without_owners = Vec::new();
        for (cell, owners) in &mut self.automatic_tunnel_owners {
            owners.retain(|owner| !remove_owner(*owner));
            if owners.is_empty() {
                cells_without_owners.push(*cell);
                if !self.manual_tunnel_cells.contains(cell)
                    && self.world.ground(cell.x, cell.y, cell.height) == Ground::Carved
                {
                    cells_to_restore.push(*cell);
                }
            }
        }
        for cell in cells_without_owners {
            self.automatic_tunnel_owners.remove(&cell);
        }
        let cells: Vec<_> = cells_to_restore
            .iter()
            .map(|cell| (cell.x, cell.y, cell.height))
            .collect();
        if !cells.is_empty() {
            let mut candidate = self.world.clone();
            match candidate.fill_cells(&cells) {
                Ok(invalidated_paths) if invalidated_paths.is_empty() => {
                    self.world.commit_candidate(candidate);
                }
                Ok(_) | Err(CellEditError::UnsupportedScenery) => {
                    self.manual_tunnel_cells.extend(cells_to_restore);
                }
                Err(CellEditError::InvalidCell) => {
                    panic!("stored tunnel ownership contains an invalid terrain cell")
                }
            }
        }
        self.support_render_cache.clear();
    }

    fn note_manual_tunnel_edit(&mut self, command: TerrainCommand) {
        let (area, bottom, top, carve) = match command {
            TerrainCommand::Carve { area, bottom, top } => (area, bottom, top, true),
            TerrainCommand::Fill { area, bottom, top } => (area, bottom, top, false),
            _ => return,
        };
        let candidate_cells: BTreeSet<_> = self
            .automatic_tunnel_owners
            .keys()
            .chain(self.manual_tunnel_cells.iter())
            .copied()
            .collect();
        for cell in candidate_cells {
            let in_edit = cell.x >= area.x
                && cell.x < area.x + u16::from(area.size)
                && cell.y >= area.y
                && cell.y < area.y + u16::from(area.size)
                && bottom <= cell.height
                && cell.height < top;
            if !in_edit {
                continue;
            }
            if carve {
                if self.automatic_tunnel_owners.contains_key(&cell) {
                    self.manual_tunnel_cells.insert(cell);
                }
            } else {
                self.manual_tunnel_cells.remove(&cell);
            }
        }
    }

    fn reject_terrain_that_blocks_track(
        &self,
        candidate: &World,
        selected_piece: Option<PieceId>,
    ) -> Result<(), ParkCommandError> {
        let clearance_worsened = |piece: &Piece, train: &TrainConfiguration| {
            !matches!(
                check_clearance(&self.world, piece, train, &[], &[]),
                Err(ClearanceCollision::Terrain)
            ) && matches!(
                check_clearance(candidate, piece, train, &[], &[]),
                Err(ClearanceCollision::Terrain)
            )
        };
        if self.demo_coaster.as_ref().is_some_and(|coaster| {
            coaster
                .circuit
                .pieces()
                .iter()
                .any(|piece| clearance_worsened(piece, &coaster.train_configuration))
        }) {
            return Err(ParkCommandError::TerrainTrackClearance);
        }
        if self.demo_coaster.as_ref().is_some_and(|coaster| {
            circuit_support_route(&coaster.circuit, &self.world, &coaster.train_configuration)
                .is_ok()
                && circuit_support_route(&coaster.circuit, candidate, &coaster.train_configuration)
                    .is_err()
        }) {
            return Err(ParkCommandError::TerrainSupportRoute);
        }

        let Some(construction) = &self.construction else {
            return Ok(());
        };
        let construction_train = &construction.model().train;
        let construction_pieces = construction
            .pieces()
            .iter()
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| &segment.pieces),
            )
            .collect::<Vec<_>>();
        if construction_pieces
            .iter()
            .any(|piece| clearance_worsened(piece, construction_train))
        {
            return Err(ParkCommandError::TerrainTrackClearance);
        }

        let entries = construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .collect::<Vec<_>>();
        let facility_obstacles = self.facility_obstacles();
        let original_supports =
            render_construction_supports(&entries, &self.world, construction, &facility_obstacles);
        let candidate_supports =
            render_construction_supports(&entries, candidate, construction, &facility_obstacles);
        if original_supports.is_ok() && candidate_supports.is_err() {
            return Err(ParkCommandError::TerrainSupportRoute);
        }

        let preview_piece = selected_piece.and_then(|selected| {
            construction
                .available_choices(&self.world, &facility_obstacles)
                .into_iter()
                .find_map(|(piece, choice)| {
                    (choice.id == selected && choice.result.is_ok()).then_some(piece)
                })
        });
        if let Some(piece) = preview_piece {
            if clearance_worsened(&piece, construction_train) {
                return Err(ParkCommandError::TerrainTrackClearance);
            }
            let mut obstacles = facility_obstacles;
            obstacles.extend(entries.iter().flat_map(|(_, placed)| {
                track_support_obstacles(std::slice::from_ref(*placed), construction_train)
            }));
            let style = construction.model().support_style.for_piece(&piece);
            if generate_supports(&piece, &self.world, &obstacles, style).is_ok()
                && generate_supports(&piece, candidate, &obstacles, style).is_err()
            {
                return Err(ParkCommandError::TerrainSupportRoute);
            }
        }
        Ok(())
    }

    fn support_route_worsened_by_scenery(&self, candidate: &World) -> bool {
        if self.demo_coaster.as_ref().is_some_and(|coaster| {
            circuit_support_route(&coaster.circuit, &self.world, &coaster.train_configuration)
                .is_ok()
                && circuit_support_route(&coaster.circuit, candidate, &coaster.train_configuration)
                    .is_err()
        }) {
            return true;
        }
        let Some(construction) = &self.construction else {
            return false;
        };
        let entries = construction
            .piece_ids()
            .iter()
            .zip(construction.pieces())
            .chain(
                construction
                    .disconnected_segments()
                    .iter()
                    .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
            )
            .collect::<Vec<_>>();
        let facility_obstacles = self.facility_obstacles();
        render_construction_supports(&entries, &self.world, construction, &facility_obstacles)
            .is_ok()
            && render_construction_supports(&entries, candidate, construction, &facility_obstacles)
                .is_err()
    }
}

fn render_construction_supports(
    entries: &[(&usize, &Piece)],
    world: &World,
    construction: &Construction,
    facility_obstacles: &[SupportObstacle],
) -> Result<(), crate::track::supports::SupportError> {
    let mut cache = BTreeMap::new();
    render_cached_supports(
        entries,
        world,
        &construction.model().train,
        construction.model().support_style,
        facility_obstacles,
        &mut cache,
    )
    .map(|_| ())
}

fn circuit_support_route(
    circuit: &Circuit,
    world: &World,
    train: &TrainConfiguration,
) -> Result<(), crate::track::supports::SupportError> {
    let model = hybrid_model();
    for (index, piece) in circuit.pieces().iter().enumerate() {
        let obstacles = circuit
            .pieces()
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .flat_map(|(_, other)| track_support_obstacles(std::slice::from_ref(other), train))
            .collect::<Vec<_>>();
        generate_supports(
            piece,
            world,
            &obstacles,
            model.support_style.for_piece(piece),
        )?;
    }
    Ok(())
}

fn construction_intersects_facility(
    kind: FacilityKind,
    position: Position,
    direction: Direction,
    construction: &Construction,
) -> bool {
    construction
        .pieces()
        .iter()
        .chain(
            construction
                .disconnected_segments()
                .iter()
                .flat_map(|segment| &segment.pieces),
        )
        .any(|piece| {
            facility_intersects_track(
                kind,
                position,
                direction,
                std::slice::from_ref(piece),
                &construction.model().train,
            )
        })
}

fn facility_intersects_track(
    kind: FacilityKind,
    position: Position,
    direction: Direction,
    pieces: &[Piece],
    train: &TrainConfiguration,
) -> bool {
    let (width, depth) = kind.footprint(direction);
    let min_x = f64::from(position.x) * 4.0;
    let min_y = f64::from(position.y) * 4.0;
    let min_z = f64::from(position.height);
    let height = match kind {
        FacilityKind::Carousel | FacilityKind::FerrisWheel => 12.0,
        _ => 4.0,
    };
    track_support_obstacles(pieces, train)
        .iter()
        .any(|obstacle| {
            obstacle.max.x > min_x
                && obstacle.min.x < min_x + f64::from(width) * 4.0
                && obstacle.max.y > min_y
                && obstacle.min.y < min_y + f64::from(depth) * 4.0
                && obstacle.max.z > min_z
                && obstacle.min.z < min_z + height
        })
}

fn scenery_intersects_track(
    bounds: ClearanceBox,
    piece: &Piece,
    train: &TrainConfiguration,
) -> bool {
    track_support_obstacles(std::slice::from_ref(piece), train)
        .iter()
        .any(|obstacle| boxes_overlap(bounds, obstacle_box(*obstacle)))
}

fn boxes_overlap(first: ClearanceBox, second: ClearanceBox) -> bool {
    first.min.x < second.max.x
        && second.min.x < first.max.x
        && first.min.y < second.max.y
        && second.min.y < first.max.y
        && first.min.z < second.max.z
        && second.min.z < first.max.z
}

fn obstacle_box(obstacle: SupportObstacle) -> ClearanceBox {
    ClearanceBox {
        min: obstacle.min,
        max: obstacle.max,
    }
}

fn tile_in_building_footprint(tile: TerrainTile, building: &crate::facilities::Building) -> bool {
    let (width, depth) = building.kind.footprint(building.direction);
    tile.x >= building.position.x
        && u32::from(tile.x) < u32::from(building.position.x) + u32::from(width)
        && tile.y >= building.position.y
        && u32::from(tile.y) < u32::from(building.position.y) + u32::from(depth)
}
