use serde::{Deserialize, Serialize};

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
    AutoBlocks(RideAutoBlocksStep),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideAutoBlocksStep {
    pub auto_blocks: bool,
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
}

#[cfg(test)]
mod tests {
    use super::{RideFile, RideFileFormat, RideHeading, RideStep};

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
        { "autoBlocks": true }
      ],
      "expect": {
        "excitement": [6.0, 8.0],
        "sustainedMaxVerticalG": [3.0, 3.5],
        "sustainedMinVerticalG": [-1.2, -0.8],
        "maxLateralG": 0.6,
        "inversions": 0,
        "airtimeMomentsAtLeast": 4
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
        assert!(matches!(ride.steps[5], RideStep::AutoBlocks(_)));
        let serialized_ride = serde_json::to_string(&ride).unwrap();
        assert_eq!(
            serde_json::from_str::<RideFile>(&serialized_ride).unwrap(),
            ride
        );
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
}
