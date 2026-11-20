use crate::track::blocks::BlockSectionPlacement;
use crate::track::shaped::{ElementIntent, ElementSpec};
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
    Shape(RideShapeStep),
    Chain(RideChainStep),
    Close(RideCloseStep),
    BlockSections(RideBlockSectionsStep),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RideShapeStep {
    pub intent: ElementIntent,
    pub spec: ElementSpec,
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
}

#[cfg(test)]
mod tests {
    use super::{RideFile, RideFileFormat, RideHeading, RideShapeStep, RideStep};
    use crate::track::shaped::{
        ElementIntent, ElementSpec, HillSizeHint, ResolvedTurnGeometry, SpeedTurnKnot, TurnSizeHint,
    };
    use crate::track::{Hand, HillFeel};
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
        assert!(matches!(ride.steps[5], RideStep::BlockSections(_)));
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

    #[test]
    fn ride_file_shapes_store_numbers_at_fixed_decimal_precision() {
        let mut ride: RideFile = serde_json::from_str(VALID_RIDE).unwrap();
        ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: ElementIntent::Hill {
                feel: HillFeel::Floater,
                strength_g: Some(-0.512345678),
                size: HillSizeHint::Height(36.123456789),
            },
            spec: serde_json::from_value(json!({
                "kind": "hill",
                "design_speed_mps": 34.123456789,
                "cars": 8,
                "target_g": -0.512345678,
                "delivered_g": -0.498765432,
                "height_m": 36.123456789,
                "span_m": 64.987654321,
                "profile": {
                    "points": [
                        {"distance": 0.123456789, "x": 0.234567891, "z": 0.345678912, "angle": 0.456789123, "heart_curvature": 0.567891234, "curvature": 0.678912345},
                        {"distance": 2.123456789, "x": 2.234567891, "z": 0.345678912, "angle": 0.456789123, "heart_curvature": 0.567891234, "curvature": 0.678912345}
                    ],
                    "cached_minimum_entry": 0.789123456,
                    "cached_minimum_radius": 1.891234567
                }
            }))
            .unwrap(),
        }));
        ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Radius(64.123456789),
            },
            spec: ElementSpec::Turn {
                design_speed_mps: 28.123456789,
                cars: 8,
                hand: Hand::Left,
                vertical_g: 3.123456789,
                lateral_g: 0.012345678,
                radius_m: 64.123456789,
                bank_degrees: 65.123456789,
                geometry: ResolvedTurnGeometry {
                    peak_bank: 1.123456789,
                    length: 100.123456789,
                    knots: vec![
                        SpeedTurnKnot {
                            x: 0.123456789,
                            y: 0.234567891,
                            heading: 0.345678912,
                        },
                        SpeedTurnKnot {
                            x: 64.123456789,
                            y: 64.234567891,
                            heading: 1.456789123,
                        },
                    ],
                },
            },
        }));

        let serialized = serde_json::to_string(&ride).unwrap();
        let encoded: Value = serde_json::from_str(&serialized).unwrap();
        let hill_spec = &encoded["steps"][6]["spec"];
        for field in [
            "design_speed_mps",
            "target_g",
            "delivered_g",
            "height_m",
            "span_m",
        ] {
            assert_decimal_precision(&hill_spec[field], 1_000_000.0);
        }
        assert_decimal_precision(&hill_spec["profile"], 1_000_000_000.0);
        assert!(
            (hill_spec["profile"]["points"][0]["angle"].as_f64().unwrap() - 0.456_789_123).abs()
                < 1e-9,
            "profile coordinates retain the measured nine-decimal precision"
        );
        assert_decimal_precision(&encoded["steps"][7]["spec"], 1_000_000.0);

        let decoded: RideFile = serde_json::from_str(&serialized).unwrap();
        let decoded_again: Value = serde_json::to_value(decoded).unwrap();
        assert_eq!(
            decoded_again["steps"][6]["spec"],
            encoded["steps"][6]["spec"]
        );
        assert_eq!(
            decoded_again["steps"][7]["spec"],
            encoded["steps"][7]["spec"]
        );
    }

    #[test]
    fn shaped_ride_step_round_trips_intent_and_resolved_geometry() {
        let ride: RideFile = serde_json::from_str(VALID_RIDE).unwrap();
        let mut shaped_ride = ride;
        shaped_ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: ElementIntent::Hill {
                feel: HillFeel::Floater,
                strength_g: Some(-0.5),
                size: HillSizeHint::Height(36.0),
            },
            spec: serde_json::from_value(json!({
                "kind": "hill",
                "design_speed_mps": 34.0,
                "cars": 8,
                "target_g": -0.5,
                "delivered_g": -0.48,
                "height_m": 36.0,
                "span_m": 64.0,
                "profile": {
                    "points": [
                        {"distance": 0.0, "x": 0.0, "z": 0.0, "angle": 0.0, "heart_curvature": 0.0, "curvature": 0.0},
                        {"distance": 2.0, "x": 2.0, "z": 0.0, "angle": 0.0, "heart_curvature": 0.0, "curvature": 0.0}
                    ],
                    "cached_minimum_entry": null,
                    "cached_minimum_radius": null
                }
            }))
            .unwrap(),
        }));
        shaped_ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Radius(64.0),
            },
            spec: ElementSpec::Turn {
                design_speed_mps: 28.0,
                cars: 8,
                hand: Hand::Left,
                vertical_g: 3.0,
                lateral_g: 0.0,
                radius_m: 64.0,
                bank_degrees: 65.0,
                geometry: ResolvedTurnGeometry {
                    peak_bank: 65.0_f64.to_radians(),
                    length: 100.0,
                    knots: vec![
                        SpeedTurnKnot {
                            x: 0.0,
                            y: 0.0,
                            heading: 0.0,
                        },
                        SpeedTurnKnot {
                            x: 64.0,
                            y: 64.0,
                            heading: 90.0_f64.to_radians(),
                        },
                    ],
                },
            },
        }));

        let encoded = serde_json::to_value(&shaped_ride).unwrap();
        assert!(encoded["steps"][6]["intent"].is_object());
        assert!(encoded["steps"][6]["spec"].to_string().contains("profile"));
        assert!(encoded["steps"][7]["spec"].to_string().contains("knots"));
        let restored = serde_json::from_value::<RideFile>(encoded.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            encoded,
            "human-readable shaped specs retain their canonical JSON values"
        );
    }

    fn assert_decimal_precision(value: &Value, decimal_scale: f64) {
        match value {
            Value::Number(number) => {
                let value = number.as_f64().expect("ride spec numbers are finite");
                let rounded = (value * decimal_scale).round() / decimal_scale;
                assert!(
                    (value - rounded).abs() <= 1e-9,
                    "ride spec number {value} is not at decimal scale {decimal_scale}"
                );
            }
            Value::Array(values) => {
                for value in values {
                    assert_decimal_precision(value, decimal_scale);
                }
            }
            Value::Object(values) => {
                for value in values.values() {
                    assert_decimal_precision(value, decimal_scale);
                }
            }
            Value::Null | Value::Bool(_) | Value::String(_) => {}
        }
    }
}
