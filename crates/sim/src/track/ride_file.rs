use crate::track::blocks::BlockSectionPlacement;
use crate::track::shaped::{
    ElementIntent, ElementSpec, ElementSpecLoadError, GeometryDigestMismatch,
};
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RideFileJson {
    format: RideFileFormat,
    version: u32,
    name: String,
    model: String,
    station: RideStation,
    cars_per_train: u8,
    trains: u8,
    steps: Vec<serde_json::Value>,
    #[serde(default)]
    expect: Option<RideExpect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RideShapeStepJson {
    intent: ElementIntent,
    spec: serde_json::Value,
}

#[derive(Debug)]
pub enum RideFileLoadError {
    InvalidJson(serde_json::Error),
    InvalidStep {
        step_index: usize,
        source: serde_json::Error,
    },
    InvalidShapedRecipe {
        step_index: usize,
        message: String,
    },
    ShapedGeometryDigestMismatch {
        step_index: usize,
        mismatch: GeometryDigestMismatch,
    },
}

impl std::fmt::Display for RideFileLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid ride JSON: {error}"),
            Self::InvalidStep { step_index, source } => {
                write!(formatter, "invalid ride step {step_index}: {source}")
            }
            Self::InvalidShapedRecipe {
                step_index,
                message,
            } => write!(
                formatter,
                "invalid shaped recipe at ride step {step_index}: {message}"
            ),
            Self::ShapedGeometryDigestMismatch {
                step_index,
                mismatch,
            } => write!(formatter, "ride step {step_index}: {mismatch}"),
        }
    }
}

impl std::error::Error for RideFileLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(error) => Some(error),
            Self::InvalidStep { source, .. } => Some(source),
            Self::InvalidShapedRecipe { .. } => None,
            Self::ShapedGeometryDigestMismatch { mismatch, .. } => Some(mismatch),
        }
    }
}

impl RideFile {
    /// Parses a ride file and preserves shaped geometry digest failures as typed errors.
    pub fn from_json_str(source: &str) -> Result<Self, RideFileLoadError> {
        let value = serde_json::from_str(source).map_err(RideFileLoadError::InvalidJson)?;
        Self::from_json_value(value)
    }

    /// Parses a ride file byte slice and preserves shaped geometry digest failures as typed errors.
    pub fn from_json_slice(source: &[u8]) -> Result<Self, RideFileLoadError> {
        let value = serde_json::from_slice(source).map_err(RideFileLoadError::InvalidJson)?;
        Self::from_json_value(value)
    }

    /// Parses a ride JSON value and preserves shaped geometry digest failures as typed errors.
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, RideFileLoadError> {
        let wire: RideFileJson =
            serde_json::from_value(value).map_err(RideFileLoadError::InvalidJson)?;
        Self::from_json_wire(wire)
    }

    fn from_json_wire(wire: RideFileJson) -> Result<Self, RideFileLoadError> {
        let steps = wire
            .steps
            .into_iter()
            .enumerate()
            .map(|(step_index, value)| Self::decode_json_step(value, step_index))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            format: wire.format,
            version: wire.version,
            name: wire.name,
            model: wire.model,
            station: wire.station,
            cars_per_train: wire.cars_per_train,
            trains: wire.trains,
            steps,
            expect: wire.expect,
        })
    }

    fn decode_json_step(
        value: serde_json::Value,
        step_index: usize,
    ) -> Result<RideStep, RideFileLoadError> {
        let is_shaped_step = value
            .as_object()
            .is_some_and(|object| object.contains_key("intent") || object.contains_key("spec"));
        if is_shaped_step {
            let wire: RideShapeStepJson = serde_json::from_value(value)
                .map_err(|source| RideFileLoadError::InvalidStep { step_index, source })?;
            let spec =
                ElementSpec::from_human_readable_json(wire.spec).map_err(|error| match error {
                    ElementSpecLoadError::GeometryDigestMismatch(mismatch) => {
                        RideFileLoadError::ShapedGeometryDigestMismatch {
                            step_index,
                            mismatch,
                        }
                    }
                    ElementSpecLoadError::InvalidRecipe(message) => {
                        RideFileLoadError::InvalidShapedRecipe {
                            step_index,
                            message,
                        }
                    }
                })?;
            return Ok(RideStep::Shape(RideShapeStep {
                intent: wire.intent,
                spec,
            }));
        }
        serde_json::from_value(value)
            .map_err(|source| RideFileLoadError::InvalidStep { step_index, source })
    }
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
    pub design_margins: Option<bool>,
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
        RideShapeStep, RideStep,
    };
    use crate::physics::{PhysicsConfig, PreviewCache, PreviewStatus};
    use crate::track::shaped::{
        ElementIntent, ElementSpec, HillSizeHint, SolveContext, TurnSizeHint, solve,
    };
    use crate::track::{Hand, HillFeel, PieceId, PieceKind, TrackState, catalog, hyper_model};
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

    fn solved_spec(intent: &ElementIntent, speed_mps: f64) -> ElementSpec {
        let mut model = hyper_model();
        let cars = 8;
        model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
        model.train.cars = cars;
        let templates = catalog();
        let station = templates
            .iter()
            .find(|piece| piece.id == PieceId::Station)
            .expect("catalog has a station")
            .at(TrackState::origin())
            .expect("station connects to origin");
        let straight = templates
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .expect("catalog has a straight");
        let physics = PhysicsConfig {
            lift_speed: speed_mps,
            ..PhysicsConfig::default()
        };
        let acceleration_distance_m = (speed_mps.powi(2) - physics.station_speed.powi(2)).max(0.0)
            / (2.0 * physics.drive_acceleration);
        let straight_count = ((acceleration_distance_m + model.train.length_metres + 40.0)
            / straight.length())
        .ceil() as usize;
        let mut prefix = vec![station];
        for _ in 0..straight_count {
            let mut lift = straight
                .at(prefix.last().expect("prefix contains station").exit)
                .expect("lift straight connects");
            lift.kind = PieceKind::Lift;
            prefix.push(lift);
        }
        let mut preview_cache = PreviewCache::default();
        let preview = preview_cache
            .preview(&prefix, &model, physics)
            .expect("lift prefix previews");
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        let mut context = SolveContext {
            entry: prefix.last().expect("prefix contains station").exit,
            entry_speed_mps: speed_mps,
            model: &model,
            train: &model.train,
            prefix: &prefix,
            preview_cache: &mut preview_cache,
            physics,
        };
        solve(intent, &mut context)
            .expect("shaped test element solves")
            .spec
    }

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

    #[test]
    fn ride_file_shapes_store_numbers_at_fixed_decimal_precision() {
        let mut ride: RideFile = serde_json::from_str(VALID_RIDE).unwrap();
        let hill_intent = ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: -0.512345678,
            size: HillSizeHint::Height(36.123456789),
        };
        let mut hill_spec = solved_spec(&hill_intent, 34.0);
        let ElementSpec::Hill {
            target_g,
            delivered_g,
            height_m,
            span_m,
            ..
        } = &mut hill_spec
        else {
            panic!("hill intent solved to a turn");
        };
        *target_g = -0.512345678;
        *delivered_g = -0.498765432;
        *height_m = 36.123456789;
        *span_m = 64.987654321;
        ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: hill_intent,
            spec: hill_spec,
        }));
        let turn_intent = ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        };
        let mut turn_spec = solved_spec(&turn_intent, 30.0);
        let ElementSpec::Turn { bank_degrees, .. } = &mut turn_spec else {
            panic!("turn intent solved to a hill");
        };
        *bank_degrees = 65.123456789;
        ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: turn_intent,
            spec: turn_spec,
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
        assert_decimal_precision(&hill_spec["train_length_metres"], 1_000_000_000.0);
        assert_decimal_precision(&hill_spec["pull_g"], 1_000_000_000.0);
        assert!(hill_spec.get("profile").is_none());
        assert!(hill_spec["geometry_digest"].as_str().is_some());
        assert_decimal_precision(&encoded["steps"][7]["spec"]["bank_degrees"], 1_000_000.0);
        assert!(encoded["steps"][7]["spec"].get("geometry").is_none());

        let decoded: RideFile = RideFile::from_json_str(&serialized).unwrap();
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
    fn shaped_ride_step_round_trips_intent_and_compact_recipe() {
        let ride: RideFile = serde_json::from_str(VALID_RIDE).unwrap();
        let mut shaped_ride = ride;
        let hill_intent = ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: -0.5,
            size: HillSizeHint::Height(36.0),
        };
        shaped_ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: hill_intent,
            spec: solved_spec(
                &ElementIntent::Hill {
                    feel: HillFeel::Floater,
                    strength_g: -0.5,
                    size: HillSizeHint::Height(36.0),
                },
                34.0,
            ),
        }));
        let turn_intent = ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        };
        shaped_ride.steps.push(RideStep::Shape(RideShapeStep {
            intent: turn_intent.clone(),
            spec: solved_spec(&turn_intent, 30.0),
        }));

        let encoded = serde_json::to_value(&shaped_ride).unwrap();
        assert!(encoded["steps"][6]["intent"].is_object());
        assert!(encoded["steps"][6]["spec"].get("profile").is_none());
        assert!(encoded["steps"][6]["spec"].get("geometry_digest").is_some());
        assert!(encoded["steps"][7]["spec"].get("geometry").is_none());
        assert!(encoded["steps"][7]["spec"].get("geometry_digest").is_some());
        let restored = RideFile::from_json_value(encoded.clone()).unwrap();
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
