use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use super::Park;
use super::tick::{generate_supports, render_cached_supports};
use crate::facilities::{
    FacilityKind, PathOpening, PlacementError, coaster_access_openings,
    validate_facility_path_access,
};
use crate::physics::Circuit;
use crate::track::clearance::{ClearanceBox, ClearanceCollision, check_clearance};
use crate::track::construction::{Construction, track_support_obstacles};
use crate::track::supports::SupportObstacle;
use crate::track::{Piece, PieceId, TrainConfiguration, hybrid_model};
use crate::world::{
    Direction, PathBuildError, PathKind, PathPreviewTile, Position, Scenery, SceneryPlacementError,
    TerrainCommand, TerrainEditError, TerrainEditResult, TerrainTile, World,
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
        self.support_render_cache.clear();
        Ok(result)
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
