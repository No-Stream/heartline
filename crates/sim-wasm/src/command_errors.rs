use theme_park_sim::facilities::PlacementError;
use theme_park_sim::park::block_analysis::BlockCommandError;
use theme_park_sim::park::commands::ParkCommandError;
use theme_park_sim::world::{PathBuildError, SceneryPlacementError, TerrainEditError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandError {
    Park(ParkCommandError),
    Block(BlockCommandError),
    UnknownFacility,
    InvalidFacilityRotation,
    FacilityOutsideWorld,
    UnknownScenery,
    SceneryOutsideWorld,
    InvalidSceneryOrientation,
    UnknownPathKind,
    PathRouteNeedsSixCoordinates,
    PathCoordinatesOutsideSupportedRange,
    TerrainNeedsEightParameters,
    TerrainCoordinatesOutsideSupportedRange,
    TerrainBrushSizeOutsideSupportedRange,
    TerrainCornerOutsideSupportedRange,
    TerrainStepsOutsideSupportedRange,
    TerrainHeightOutsideSupportedRange,
    ExcavationBottomOutsideSupportedRange,
    ExcavationTopOutsideSupportedRange,
    UnknownTerrainCorner,
    UnknownTerrainCommand,
}

pub(crate) fn record_command_result<T>(
    result: Result<T, CommandError>,
    message: &mut String,
    clear_on_success: bool,
) -> Option<T> {
    match result {
        Ok(value) => {
            if clear_on_success {
                message.clear();
            }
            Some(value)
        }
        Err(error) => {
            *message = command_error_message(error).into();
            None
        }
    }
}

pub(crate) fn command_error_message(error: CommandError) -> &'static str {
    match error {
        CommandError::Park(error) => park_error_message(error),
        CommandError::Block(error) => block_error_message(error),
        CommandError::UnknownFacility => "Unknown facility",
        CommandError::InvalidFacilityRotation => "Invalid rotation",
        CommandError::FacilityOutsideWorld => "Outside world",
        CommandError::UnknownScenery => "Unknown scenery",
        CommandError::SceneryOutsideWorld => "Outside world",
        CommandError::InvalidSceneryOrientation => "Invalid scenery orientation",
        CommandError::UnknownPathKind => "Unknown path kind",
        CommandError::PathRouteNeedsSixCoordinates => "Path route needs six coordinates",
        CommandError::PathCoordinatesOutsideSupportedRange => {
            "Path coordinates are outside the supported range"
        }
        CommandError::TerrainNeedsEightParameters => "Terrain edit needs eight integer parameters",
        CommandError::TerrainCoordinatesOutsideSupportedRange => {
            "Terrain coordinates are outside the supported range"
        }
        CommandError::TerrainBrushSizeOutsideSupportedRange => {
            "Terrain brush size is outside the supported range"
        }
        CommandError::TerrainCornerOutsideSupportedRange => {
            "Terrain corner is outside the supported range"
        }
        CommandError::TerrainStepsOutsideSupportedRange => {
            "Terrain step count is outside the supported range"
        }
        CommandError::TerrainHeightOutsideSupportedRange => {
            "Terrain height is outside the supported range"
        }
        CommandError::ExcavationBottomOutsideSupportedRange => {
            "Excavation bottom is outside the supported range"
        }
        CommandError::ExcavationTopOutsideSupportedRange => {
            "Excavation top is outside the supported range"
        }
        CommandError::UnknownTerrainCorner => "Unknown terrain corner",
        CommandError::UnknownTerrainCommand => "Unknown terrain command",
    }
}

pub(crate) fn block_error_message(error: BlockCommandError) -> &'static str {
    match error {
        BlockCommandError::InvalidTrainCount => "",
        BlockCommandError::CompleteCircuitRequired => {
            "A complete circuit is required to open the coaster."
        }
        BlockCommandError::MissingStationAccess => {
            "Connect a queue and an exit path to the station."
        }
        BlockCommandError::ClosedCircuitRequiredForPlacement => {
            "A closed circuit is required to place block sections."
        }
        BlockCommandError::ClosedCircuitRequiredForEditing => {
            "A closed circuit is required to edit block sections."
        }
        BlockCommandError::NoAutomaticSearch => {
            "There is no automatic block placement search in progress."
        }
        BlockCommandError::NoBlockLayout => "Place automatic block sections before editing them.",
        BlockCommandError::IndexOutOfRange => "Block section index is out of range.",
        BlockCommandError::Validation(reason) => reason,
    }
}

fn park_error_message(error: ParkCommandError) -> &'static str {
    match error {
        ParkCommandError::Facility(PlacementError::OutsideWorld) => "OutsideWorld",
        ParkCommandError::Facility(PlacementError::InvalidTerrain) => "InvalidTerrain",
        ParkCommandError::Facility(PlacementError::Occupied) => "Occupied",
        ParkCommandError::Facility(PlacementError::EntranceExists) => "EntranceExists",
        ParkCommandError::Facility(PlacementError::UnknownBuilding) => "UnknownBuilding",
        ParkCommandError::Facility(PlacementError::InvalidPrice) => "InvalidPrice",
        ParkCommandError::Facility(PlacementError::MissingAccess) => "MissingAccess",
        ParkCommandError::Facility(PlacementError::QueueMustEndAtFacility) => {
            "QueueMustEndAtFacility"
        }
        ParkCommandError::Facility(PlacementError::ExitMustBeNormal) => "ExitMustBeNormal",
        ParkCommandError::Facility(PlacementError::QueueConnectedToExit) => "QueueConnectedToExit",
        ParkCommandError::Facility(PlacementError::QueueAlreadyOwned) => "QueueAlreadyOwned",
        ParkCommandError::Path(error) => path_error_message(error),
        ParkCommandError::Terrain(error) => terrain_error_message(error),
        ParkCommandError::Scenery(error) => scenery_error_message(error),
        ParkCommandError::FacilityIntersectsTrack => "Facility intersects track or train clearance",
        ParkCommandError::PathMissing => "No path at that position",
        ParkCommandError::SceneryMissing => "No scenery with that ID",
        ParkCommandError::SceneryIntersectsFacility => "Scenery intersects a facility",
        ParkCommandError::SceneryIntersectsTrack => {
            "Scenery intersects coaster track or train clearance"
        }
        ParkCommandError::ScenerySupportRoute => "Scenery leaves no route for coaster supports",
        ParkCommandError::TerrainUnderBuilding => "Cannot edit terrain beneath a building or ride",
        ParkCommandError::TerrainTrackClearance => "Terrain edit would collide with coaster track",
        ParkCommandError::TerrainSupportRoute => {
            "Terrain edit leaves no route for coaster supports"
        }
    }
}

fn path_error_message(error: PathBuildError) -> &'static str {
    match error {
        PathBuildError::OutsideWorld => "Path route is outside the park",
        PathBuildError::UnsupportedTerrain => "Path route crosses unsupported terrain",
        PathBuildError::EndpointHeight => "Path endpoint heights do not match the route",
        PathBuildError::Disconnected => "Path route does not connect across the terrain",
        PathBuildError::Occupied => "Path route overlaps another path",
        PathBuildError::Unsupported => "Path route lacks terrain support or clearance",
        PathBuildError::QueueConnections => {
            "A queue path must have at most one normal path entrance"
        }
        PathBuildError::FacilityOccupied => "Path overlaps a facility",
        PathBuildError::FacilityAccess => "Path conflicts with attraction access",
    }
}

fn terrain_error_message(error: TerrainEditError) -> &'static str {
    match error {
        TerrainEditError::OutsideWorld => "Terrain edit is outside the park",
        TerrainEditError::InvalidBrushSize => "Brush size must be between 1 and 9",
        TerrainEditError::InvalidStepCount => "Terrain change must be at least one step",
        TerrainEditError::InvalidInterval => "Excavation interval is invalid",
        TerrainEditError::CarveAboveTerrain => "Excavation exceeds the terrain height",
        TerrainEditError::NoWater => "There is no water to lower in this area",
        TerrainEditError::HeightOverflow => "Terrain height exceeds the supported range",
        TerrainEditError::SceneryObstructed => "Terrain edit would disturb scenery",
    }
}

fn scenery_error_message(error: SceneryPlacementError) -> &'static str {
    match error {
        SceneryPlacementError::OutsideWorld => "OutsideWorld",
        SceneryPlacementError::InvalidOrientation => "InvalidOrientation",
        SceneryPlacementError::InvalidTerrain => "InvalidTerrain",
        SceneryPlacementError::Occupied => "Occupied",
    }
}

impl From<ParkCommandError> for CommandError {
    fn from(error: ParkCommandError) -> Self {
        Self::Park(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_command_errors_keep_the_existing_player_messages() {
        for (error, message) in [
            (
                BlockCommandError::CompleteCircuitRequired,
                "A complete circuit is required to open the coaster.",
            ),
            (
                BlockCommandError::MissingStationAccess,
                "Connect a queue and an exit path to the station.",
            ),
            (
                BlockCommandError::ClosedCircuitRequiredForPlacement,
                "A closed circuit is required to place block sections.",
            ),
            (
                BlockCommandError::ClosedCircuitRequiredForEditing,
                "A closed circuit is required to edit block sections.",
            ),
            (
                BlockCommandError::NoAutomaticSearch,
                "There is no automatic block placement search in progress.",
            ),
            (
                BlockCommandError::NoBlockLayout,
                "Place automatic block sections before editing them.",
            ),
            (
                BlockCommandError::IndexOutOfRange,
                "Block section index is out of range.",
            ),
            (
                BlockCommandError::Validation("validation failed"),
                "validation failed",
            ),
        ] {
            assert_eq!(command_error_message(CommandError::Block(error)), message);
        }
        assert_eq!(
            command_error_message(CommandError::UnknownFacility),
            "Unknown facility"
        );
        assert_eq!(
            command_error_message(CommandError::Park(ParkCommandError::Facility(
                PlacementError::OutsideWorld,
            ))),
            "OutsideWorld"
        );
        assert_eq!(
            command_error_message(CommandError::Park(ParkCommandError::Terrain(
                TerrainEditError::OutsideWorld,
            ))),
            "Terrain edit is outside the park"
        );
        assert_eq!(
            command_error_message(CommandError::Park(ParkCommandError::Path(
                PathBuildError::FacilityAccess,
            ))),
            "Path conflicts with attraction access"
        );
        assert_eq!(
            command_error_message(CommandError::Park(ParkCommandError::SceneryIntersectsTrack,)),
            "Scenery intersects coaster track or train clearance"
        );
    }

    #[test]
    fn recording_success_can_clear_a_previous_message() {
        let mut message = "stale error".to_owned();

        let result = record_command_result(Ok(()), &mut message, true);

        assert_eq!(result, Some(()));
        assert!(message.is_empty());
    }

    #[test]
    fn block_command_errors_reach_both_player_messages() {
        let mut simulation = crate::WasmSimulation::new(42, 0);
        assert!(!simulation.construction_auto_blocks());
        assert_eq!(
            simulation.construction_message(),
            "A closed circuit is required to place block sections."
        );
        assert_eq!(
            simulation.park.block_message,
            "A closed circuit is required to place block sections."
        );
        assert!(!simulation.construction_set_train_count(0));
        assert_eq!(
            simulation.construction_message(),
            "A closed circuit is required to place block sections."
        );
    }
}
