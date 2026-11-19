use crate::track::blocks::BlockSectionPlacement;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RideFileFormat {
    #[serde(rename = "heartline-ride")]
    HeartlineRide,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideFile {
    pub format: RideFileFormat,
    pub version: u32,
    pub name: String,
    pub model: String,
    pub station: RideStation,
    pub cars_per_train: u8,
    pub trains: u8,
    pub steps: Vec<RideStep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<RideExpect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RideHeading {
    East,
    North,
    West,
    South,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideStation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: RideHeading,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RideStep {
    Place(RidePlaceStep),
    Chain(RideChainStep),
    Close(RideCloseStep),
    BlockSections(RideBlockSectionsStep),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RidePlaceStep {
    pub place: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideChainStep {
    pub chain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideCloseStep {
    pub close: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideBlockSectionsStep {
    pub block_sections: Vec<BlockSectionPlacement>,
    pub train_capacity: usize,
    pub capacity_search_complete: bool,
    pub placement_search_complete: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideExpect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excitement: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sustained_max_vertical_g: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sustained_min_vertical_g: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lateral_g: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inversions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub airtime_moments_at_least: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_comfortable_force_limits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inversions_at_least: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launches: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biggest_drop_at_least: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speed_at_least: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inverted_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<RideExpectFeatures>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideExpectFeatures {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub piece_prefixes_at_least: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub piece_prefix_groups_at_least: Vec<RideExpectPiecePrefixGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartlined_inversions_at_least: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub piece_prefixes_after: Vec<RideExpectPiecePrefixAfter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ordered_steps: Vec<Vec<RideExpectFeatureStep>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjacent_steps: Vec<Vec<RideExpectFeatureStep>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideExpectPiecePrefixGroup {
    pub prefixes: Vec<String>,
    pub minimum: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideExpectPiecePrefixAfter {
    pub prefix: String,
    pub after: RideExpectFeatureStep,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RideExpectFeatureStep {
    Place(RideExpectPlaceFeatureStep),
    PlacePrefix(RideExpectPlacePrefixFeatureStep),
    Chain(RideExpectChainFeatureStep),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideExpectPlaceFeatureStep {
    pub place: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideExpectPlacePrefixFeatureStep {
    pub place_prefix: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RideExpectChainFeatureStep {
    pub chain: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        RideExpectChainFeatureStep, RideExpectFeatureStep, RideFile, RideFileFormat, RideHeading,
        RideStep,
    };
    use serde_json::{Value, json};

    const VALID_RIDE: &str = r#"
    {
      "format": "heartline-ride",
      "version": 1,
      "name": "Horizon Hawk showcase",
      "model": "hyper",
      "station": { "x": 241, "y": 249, "z": 2, "heading": "north" },
      "carsPerTrain": 8,
      "trains": 1,
      "steps": [
        { "chain": true },
        { "place": "LevelToSteepUp", "note": "lift" },
        { "place": "LaunchLong", "launchSpeed": 31.3 },
        { "place": "element:immelmann-left" },
        { "close": true },
        {
          "blockSections": [{
            "boundaryMetres": 38.0,
            "brakeStartMetres": 12.5,
            "brakePieceIndices": [4, 5]
          }],
          "trainCapacity": 2,
          "capacitySearchComplete": true,
          "placementSearchComplete": false
        }
      ],
      "expect": {
        "excitement": [6.0, 8.0],
        "sustainedMaxVerticalG": [3.0, 3.5],
        "sustainedMinVerticalG": [-1.2, -0.8],
        "maxLateralG": 0.6,
        "inversions": 0,
        "airtimeMomentsAtLeast": 4,
        "withinComfortableForceLimits": true,
        "inversionsAtLeast": 2,
        "launches": 2,
        "biggestDropAtLeast": 50.0,
        "maxSpeedAtLeast": 30.0,
        "invertedSeconds": 0.0,
        "features": {
          "piecePrefixesAtLeast": {"FamilyHill": 2},
          "piecePrefixGroupsAtLeast": [{"prefixes": ["TightTwister", "Twister"], "minimum": 2}],
          "heartlinedInversionsAtLeast": 2,
          "piecePrefixesAfter": [{"prefix": "BankedTurnStandalone", "after": {"chain": false}}],
          "orderedSteps": [[{"chain": true}, {"place": "TurnSmallStandalone"}, {"chain": false}]],
          "adjacentSteps": [[{"place": "Launch"}, {"place": "LaunchedTopHat"}]]
        }
      }
    }
    "#;

    #[test]
    fn ride_file_round_trips_the_documented_json_shape() {
        let ride: RideFile = serde_json::from_str(VALID_RIDE).unwrap();

        assert_eq!(ride.format, RideFileFormat::HeartlineRide);
        assert_eq!(ride.station.heading, RideHeading::North);
        assert!(matches!(ride.steps[0], RideStep::Chain(_)));
        assert!(matches!(ride.steps[1], RideStep::Place(_)));
        assert!(matches!(ride.steps[5], RideStep::BlockSections(_)));
        assert_eq!(
            ride.expect
                .as_ref()
                .unwrap()
                .within_comfortable_force_limits,
            Some(true)
        );
        assert_eq!(ride.expect.as_ref().unwrap().inversions_at_least, Some(2));
        assert_eq!(ride.expect.as_ref().unwrap().launches, Some(2));
        assert_eq!(
            ride.expect.as_ref().unwrap().biggest_drop_at_least,
            Some(50.0)
        );
        assert_eq!(ride.expect.as_ref().unwrap().max_speed_at_least, Some(30.0));
        assert_eq!(ride.expect.as_ref().unwrap().inverted_seconds, Some(0.0));
        let features = ride.expect.as_ref().unwrap().features.as_ref().unwrap();
        assert_eq!(features.piece_prefixes_at_least["FamilyHill"], 2);
        assert_eq!(features.piece_prefix_groups_at_least[0].minimum, 2);
        assert_eq!(features.heartlined_inversions_at_least, Some(2));
        assert_eq!(
            features.piece_prefixes_after[0].prefix,
            "BankedTurnStandalone"
        );
        assert!(matches!(
            features.piece_prefixes_after[0].after,
            RideExpectFeatureStep::Chain(RideExpectChainFeatureStep { chain: false })
        ));
        assert_eq!(features.ordered_steps.len(), 1);
        assert_eq!(features.adjacent_steps.len(), 1);
        let serialized_ride = serde_json::to_string(&ride).unwrap();
        assert_eq!(
            serde_json::from_str::<RideFile>(&serialized_ride).unwrap(),
            ride
        );
        assert!(serialized_ride.contains(r#""boundaryMetres":38.0"#));
    }

    #[test]
    fn ride_file_rejects_unknown_keys_at_every_level() {
        for invalid_ride in [
            VALID_RIDE.replace("\"version\": 1", "\"version\": 1, \"extra\": true"),
            VALID_RIDE.replace(
                "\"heading\": \"north\"",
                "\"heading\": \"north\", \"extra\": true",
            ),
            VALID_RIDE.replace(
                "\"place\": \"LevelToSteepUp\"",
                "\"place\": \"LevelToSteepUp\", \"extra\": true",
            ),
            VALID_RIDE.replace(
                "\"trainCapacity\": 2",
                "\"trainCapacity\": 2, \"extra\": true",
            ),
            VALID_RIDE.replace("\"inversions\": 0", "\"inversions\": 0, \"extra\": true"),
        ] {
            assert!(serde_json::from_str::<RideFile>(&invalid_ride).is_err());
        }
    }

    #[test]
    fn ride_file_rejects_unknown_format_and_heading_values() {
        assert!(
            serde_json::from_str::<RideFile>(&VALID_RIDE.replace("heartline-ride", "unknown-ride"))
                .is_err()
        );
        assert!(
            serde_json::from_str::<RideFile>(&VALID_RIDE.replace("\"north\"", "\"northwest-ish\""))
                .is_err()
        );
    }

    #[test]
    fn ride_file_rejects_search_based_auto_blocks_steps() {
        let mut ride: Value = serde_json::from_str(VALID_RIDE).unwrap();
        ride["steps"][5] = json!({"autoBlocks": true});
        assert!(serde_json::from_value::<RideFile>(ride).is_err());
    }
}
