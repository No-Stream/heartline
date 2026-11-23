use crate::WasmSimulation;
use crate::builder_session::BuilderSession;
use serde_json::{Value, json};
use theme_park_sim::physics::{PhysicsConfig, PreviewCache, PreviewStatus, TrackPreview};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, LoopTopFeel, SolveContext, SolvedElement,
    TurnSizeHint, default_hill_strength, default_loop_peak_g, default_raven_turn_peak_g,
    default_zero_g_roll_peak_g, solve,
};
use theme_park_sim::track::{
    Hand, HillFeel, Inversion, Piece, PieceId, PieceKind, TrackState, catalog, coaster_models,
};
use theme_park_sim::world::{Surface, World};
use wasm_bindgen::prelude::*;

const HYPER_HILL_ENTRY_SPEED_MPS: f64 = 34.0;
const HYPER_TURN_ENTRY_SPEED_MPS: f64 = 28.0;
const GRID_MIDPOINT_ENTRY_SPEED_MPS: f64 = 30.25;
const STRONG_FLOATER_TARGET_G: f64 = -0.85;
const COLD_BENCHMARK_CARS: [u8; 3] = [4, 6, 8];
const COLD_BENCHMARK_SPEEDS_MPS: [f64; 5] = [22.0, 26.0, 30.0, 34.0, 36.0];
const COLD_BENCHMARK_ELEMENTS: [&str; 3] =
    ["floater-hill", "strong-floater-hill", "fully-banked-turn"];

#[wasm_bindgen]
pub fn horizon_hawk_first_turn_arc_table_json() -> String {
    let mut ride = RideFile::from_json_str(include_str!(
        "../../../content/rides/horizon-hawk.ride.json"
    ))
    .expect("Horizon Hawk ride file is valid");
    let first_turn_index = ride
        .steps
        .iter()
        .position(|step| matches!(step, RideStep::Shape(shape) if matches!(shape.spec, ElementSpec::Turn { .. })))
        .expect("Horizon Hawk contains a shaped turn");
    ride.steps.truncate(first_turn_index + 1);

    let mut session = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    session
        .replay_ride(&ride)
        .expect("Horizon Hawk prefix replays through the first shaped turn");
    session
        .simulation()
        .construction_last_piece_arc_table_json()
}

#[wasm_bindgen]
pub fn horizon_hawk_first_turn_candidate_preview_json() -> String {
    let mut ride = RideFile::from_json_str(include_str!(
        "../../../content/rides/horizon-hawk.ride.json"
    ))
    .expect("Horizon Hawk ride file is valid");
    let first_turn_index = ride
        .steps
        .iter()
        .position(|step| matches!(step, RideStep::Shape(shape) if matches!(shape.spec, ElementSpec::Turn { .. })))
        .expect("Horizon Hawk contains a shaped turn");
    ride.steps.truncate(first_turn_index + 1);

    let mut session = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    let estimates = session
        .replay_ride(&ride)
        .expect("Horizon Hawk prefix replays through the first shaped turn");
    serde_json::to_string(
        estimates
            .candidate_preview
            .as_ref()
            .expect("first shaped turn leaves a selected candidate"),
    )
    .expect("candidate preview serializes")
}

/// Selects the first shaped hill of Horizon Hawk after its saved prefix, as the builder does,
/// so native and WASM solves of the same intent can be compared exactly.
#[wasm_bindgen]
pub fn horizon_hawk_first_hill_selected_shape_json() -> String {
    let mut ride = RideFile::from_json_str(include_str!(
        "../../../content/rides/horizon-hawk.ride.json"
    ))
    .expect("Horizon Hawk ride file is valid");
    let first_hill_index = ride
        .steps
        .iter()
        .position(|step| matches!(step, RideStep::Shape(shape) if matches!(shape.spec, ElementSpec::Hill { .. })))
        .expect("Horizon Hawk contains a shaped hill");
    let RideStep::Shape(first_hill) = ride.steps[first_hill_index].clone() else {
        unreachable!("the first hill index points at a shaped step")
    };
    ride.steps.truncate(first_hill_index);

    let mut session = BuilderSession::new(WasmSimulation::new(42, 0, String::from("new")));
    session
        .replay_ride(&ride)
        .expect("Horizon Hawk prefix replays up to the first shaped hill");
    let ElementIntent::Hill {
        feel: HillFeel::StrongFloater,
        strength_g,
        size: HillSizeHint::Height(height_m),
    } = first_hill.intent
    else {
        panic!("Horizon Hawk's first shaped hill is expected to be a fixed-height strong floater");
    };
    session
        .select(
            &format!("shape:hill:strongFloater:{strength_g}:fixed-{height_m}"),
            true,
        )
        .expect("the first shaped hill selects");
    serde_json::to_string(
        &session
            .estimates()
            .selected_shape
            .expect("the first shaped hill is the selected shape"),
    )
    .expect("selected shape serializes")
}

#[wasm_bindgen]
pub struct ShapedSolveBenchmark {
    kind: String,
    intent: ElementIntent,
    requested_entry_speed_mps: f64,
    observed_entry_speed_mps: f64,
    model: theme_park_sim::track::CoasterModel,
    prefix: Vec<Piece>,
    entry: TrackState,
    physics: PhysicsConfig,
    preview_cache: PreviewCache,
    solved: Option<SolvedElement>,
    calibration_piece: Option<Piece>,
    calibration_pieces: Vec<Piece>,
    front_preview_cache: PreviewCache,
    all_cars_preview_cache: PreviewCache,
    front_preview: Option<TrackPreview>,
    all_cars_preview: Option<TrackPreview>,
    calibration_result: Option<(usize, usize)>,
    solve_refusal: Option<String>,
    commit_would_tunnel: Option<bool>,
    commit_world: World,
}

#[wasm_bindgen]
impl ShapedSolveBenchmark {
    #[wasm_bindgen(constructor)]
    pub fn new(kind: &str) -> Self {
        let (model, intent, requested_entry_speed_mps) = benchmark_case(kind);
        let prefix = lift_prefix(&model, requested_entry_speed_mps);
        let entry = prefix.last().expect("lift prefix has a station").exit;
        let physics = PhysicsConfig {
            lift_speed: requested_entry_speed_mps,
            ..PhysicsConfig::default()
        };
        Self {
            kind: kind.to_owned(),
            intent,
            requested_entry_speed_mps,
            observed_entry_speed_mps: f64::NAN,
            model,
            prefix,
            entry,
            physics,
            preview_cache: PreviewCache::default(),
            solved: None,
            calibration_piece: None,
            calibration_pieces: Vec::new(),
            front_preview_cache: PreviewCache::default(),
            all_cars_preview_cache: PreviewCache::default(),
            front_preview: None,
            all_cars_preview: None,
            calibration_result: None,
            solve_refusal: None,
            commit_would_tunnel: None,
            commit_world: World::new(
                512,
                512,
                Surface {
                    corners: [0; 4],
                    water: None,
                },
            ),
        }
    }

    /// Rebuilds the prefix preview outside the timed solve region.
    pub fn prepare(&mut self) {
        self.preview_cache = PreviewCache::default();
        let preview = self
            .preview_cache
            .preview(&self.prefix, &self.model, self.physics)
            .expect("lift prefix contains a station");
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        self.observed_entry_speed_mps = preview
            .report
            .samples
            .last()
            .expect("prefix preview records the lead car")
            .speed;
        assert!(
            (self.observed_entry_speed_mps - self.requested_entry_speed_mps).abs() <= 0.01,
            "{} prefix delivered {:.5} m/s, requested {:.5} m/s",
            self.kind,
            self.observed_entry_speed_mps,
            self.requested_entry_speed_mps
        );
        self.solved = None;
        self.solve_refusal = None;
        self.commit_would_tunnel = None;
    }

    /// Runs the production solver against the configured model train and seeded prefix preview.
    pub fn solve(&mut self) {
        self.try_solve();
        if let Some(refusal) = &self.solve_refusal {
            panic!("{} refused: {refusal}", self.kind);
        }
    }

    /// Runs one solve while returning typed refusals as benchmark data.
    pub fn try_solve(&mut self) {
        assert!(
            self.observed_entry_speed_mps.is_finite(),
            "call prepare first"
        );
        self.solved = None;
        self.calibration_piece = None;
        self.solve_refusal = None;
        self.commit_would_tunnel = None;
        let solved = {
            let mut context = SolveContext {
                entry: self.entry,
                entry_speed_mps: self.observed_entry_speed_mps,
                model: &self.model,
                train: &self.model.train,
                prefix: &self.prefix,
                preview_cache: &mut self.preview_cache,
                physics: self.physics,
            };
            match solve(&self.intent, &mut context) {
                Ok(solved) => solved,
                Err(refusal) => {
                    self.solve_refusal = Some(format!("{refusal:?}"));
                    self.solved = None;
                    self.calibration_piece = None;
                    return;
                }
            }
        };
        self.calibration_piece = Some(solved.piece.clone());
        self.solved = Some(solved);
    }

    /// Runs the production solve and validates the resolved piece through construction append.
    pub fn try_solve_and_commit(&mut self) {
        self.try_solve();
        let Some(solved) = self.solved.as_ref() else {
            return;
        };
        let mut construction = Construction::new(solved.piece.entry, self.model.clone());
        self.commit_would_tunnel = Some(
            construction
                .append_resolved(&self.intent, &solved.spec, &self.commit_world, &[])
                .unwrap_or_else(|error| panic!("{} commit failed: {error:?}", self.kind)),
        );
    }

    /// Resets and seeds the common-prefix snapshot outside the calibration timer.
    pub fn prepare_calibration(&mut self) {
        let candidate = self
            .calibration_piece
            .as_ref()
            .expect("solve must run before preparing calibration")
            .clone();
        self.front_preview_cache = seeded_preview_cache(
            &self.prefix,
            &self.model,
            self.physics,
            self.requested_entry_speed_mps,
        );
        self.all_cars_preview_cache = seeded_preview_cache(
            &self.prefix,
            &self.model,
            self.physics,
            self.requested_entry_speed_mps,
        );

        let mut pieces = self.prefix.clone();
        pieces.push(candidate.clone());
        let straight_template = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .expect("catalog includes straight track");
        let runout_count =
            (self.model.train.length_metres / straight_template.length()).ceil() as usize + 2;
        let mut state = candidate.exit;
        for _ in 0..runout_count {
            let piece = straight_template
                .at(state)
                .expect("candidate exit accepts a straight runout");
            state = piece.exit;
            pieces.push(piece);
        }
        self.calibration_pieces = pieces;
        self.front_preview = None;
        self.all_cars_preview = None;
        self.calibration_result = None;
    }

    /// Runs one actual-train calibration preview, including the rear-car runout.
    pub fn calibrate_once(&mut self) {
        assert!(
            !self.calibration_pieces.is_empty(),
            "prepare calibration first"
        );
        let candidate_index = self.prefix.len();
        let preview = self
            .all_cars_preview_cache
            .preview_candidate(
                &self.calibration_pieces,
                &self.model,
                self.physics,
                candidate_index,
            )
            .expect("calibration track contains a station");
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        let cars = preview.cars.len();
        let within_limits_cars = preview.cars.iter().filter(|car| car.within_limits).count();
        self.calibration_result = Some((cars, within_limits_cars));
    }

    /// Runs the common candidate preview path and records only the front car.
    pub fn preview_front_car_once(&mut self) {
        assert!(
            !self.calibration_pieces.is_empty(),
            "prepare calibration first"
        );
        let preview = self
            .front_preview_cache
            .preview(&self.calibration_pieces, &self.model, self.physics)
            .expect("calibration track contains a station");
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        self.front_preview = Some(preview);
    }

    /// Runs full-train calibration against the same candidate and seeded prefix.
    pub fn preview_all_cars_once(&mut self) {
        assert!(
            !self.calibration_pieces.is_empty(),
            "prepare calibration first"
        );
        let preview = self
            .all_cars_preview_cache
            .preview_all_cars(&self.calibration_pieces, &self.model, self.physics)
            .expect("calibration track contains a station");
        assert_eq!(preview.status, PreviewStatus::ReachedEnd);
        self.all_cars_preview = Some(preview);
    }

    /// Returns the number of cars represented in the candidate piece's calibration data.
    pub fn calibration_result_json(&self) -> String {
        let (cars, within_limits_cars) = self
            .calibration_result
            .expect("calibrate_once must run before reading results");
        serde_json::to_string(&json!({
            "candidate_cars": cars,
            "within_limits_cars": within_limits_cars,
            "expected_cars": self.model.train.cars,
        }))
        .expect("calibration result serializes")
    }

    /// Returns the number of car-specific candidate records from both preview modes.
    pub fn preview_modes_result_json(&self) -> String {
        let candidate_index = self.prefix.len();
        let front_preview = self
            .front_preview
            .as_ref()
            .expect("front-car preview must run before reading results");
        let all_cars_preview = self
            .all_cars_preview
            .as_ref()
            .expect("full-train preview must run before reading results");
        let front_cars = candidate_car_count(front_preview, candidate_index);
        let all_cars = candidate_car_count(all_cars_preview, candidate_index);
        serde_json::to_string(&json!({
            "front_preview_cars": front_cars,
            "all_cars_preview_cars": all_cars,
            "expected_cars": self.model.train.cars,
        }))
        .expect("preview mode result serializes")
    }

    /// Resets both candidate preview caches to the same common-prefix snapshot.
    pub fn prepare_preview_modes(&mut self) {
        self.prepare_calibration();
    }

    /// Returns a compact result after timing has stopped.
    pub fn result_json(&self) -> String {
        let Some(solved) = self.solved.as_ref() else {
            let refusal = self
                .solve_refusal
                .as_ref()
                .expect("solve must run before reading results");
            return serde_json::to_string(&json!({
                "status": "refused",
                "kind": &self.kind,
                "model": self.model.key,
                "cars": self.model.train.cars,
                "requested_entry_speed_mps": self.requested_entry_speed_mps,
                "observed_entry_speed_mps": self.observed_entry_speed_mps,
                "refusal": refusal,
            }))
            .expect("refusal result serializes");
        };
        let mut result = result_summary(
            &self.kind,
            self.model.key,
            self.requested_entry_speed_mps,
            self.observed_entry_speed_mps,
            solved,
        );
        result["construction_append_would_tunnel"] = self
            .commit_would_tunnel
            .map_or(Value::Null, |would_tunnel| json!(would_tunnel));
        serde_json::to_string(&result).expect("benchmark result serializes")
    }
}

/// Lists the stage-1 hyper cases across the supported train lengths and speeds.
#[wasm_bindgen]
pub fn shaped_benchmark_cases_json() -> String {
    let cases: Vec<_> = COLD_BENCHMARK_ELEMENTS
        .into_iter()
        .flat_map(|element| {
            COLD_BENCHMARK_CARS.into_iter().flat_map(move |cars| {
                COLD_BENCHMARK_SPEEDS_MPS
                    .into_iter()
                    .map(move |requested_speed_mps| {
                        json!({
                            "id": format!("{element}:hyper:{cars}:{requested_speed_mps}"),
                            "model": "hyper",
                            "cars": cars,
                            "element": element,
                            "requested_speed_mps": requested_speed_mps,
                        })
                    })
            })
        })
        .collect();
    serde_json::to_string(&cases).expect("benchmark cases serialize")
}

/// Lists shaped hill, turn, and supported inversion parity cases for every profiled coaster model.
#[wasm_bindgen]
pub fn shaped_parity_cases_json() -> String {
    let mut cases = Vec::new();
    for model in coaster_models() {
        let Some(profile) = model.shaped_profile else {
            continue;
        };
        let cars = model.train.cars;
        let midpoint_speed =
            ((profile.minimum_speed_mps + profile.maximum_speed_mps) / 2.0).round();
        cases.push(json!({
            "id": format!("floater-hill:{}:{cars}:{midpoint_speed}", model.key),
            "model": model.key,
            "cars": cars,
            "element": "floater-hill",
            "requested_speed_mps": midpoint_speed,
        }));
        cases.push(json!({
            "id": format!("fully-banked-turn:{}:{cars}:{midpoint_speed}", model.key),
            "model": model.key,
            "cars": cars,
            "element": "fully-banked-turn",
            "requested_speed_mps": midpoint_speed,
        }));
        if profile.minimum_inversion_speed_mps > 0.0 {
            let inversion_speed =
                ((profile.minimum_inversion_speed_mps + profile.maximum_inversion_speed_mps) / 2.0)
                    .round();
            if model.allowed_inversions.contains(&Inversion::VerticalLoop) {
                cases.push(json!({
                    "id": format!("vertical-loop:{}:{cars}:{inversion_speed}", model.key),
                    "model": model.key,
                    "cars": cars,
                    "element": "vertical-loop",
                    "requested_speed_mps": inversion_speed,
                }));
            }
            if model.allowed_inversions.contains(&Inversion::HeartlineRoll) {
                cases.push(json!({
                    "id": format!("zero-g-roll:{}:{cars}:{inversion_speed}", model.key),
                    "model": model.key,
                    "cars": cars,
                    "element": "zero-g-roll",
                    "requested_speed_mps": inversion_speed,
                }));
            }
            if model.allowed_inversions.contains(&Inversion::VerticalLoop)
                && model.allowed_inversions.contains(&Inversion::Corkscrew)
            {
                cases.push(json!({
                    "id": format!("raven-turn:{}:{cars}:{inversion_speed}", model.key),
                    "model": model.key,
                    "cars": cars,
                    "element": "raven-turn",
                    "requested_speed_mps": inversion_speed,
                }));
            }
        }
    }
    serde_json::to_string(&cases).expect("parity cases serialize")
}

fn benchmark_case(kind: &str) -> (theme_park_sim::track::CoasterModel, ElementIntent, f64) {
    let (model_key, cars, element_kind, speed_mps) = match kind {
        "hyper-strong-floater-hill" => (
            "hyper",
            None,
            "strong-floater-hill",
            HYPER_HILL_ENTRY_SPEED_MPS,
        ),
        "hyper-fully-banked-turn" => (
            "hyper",
            None,
            "fully-banked-turn",
            HYPER_TURN_ENTRY_SPEED_MPS,
        ),
        _ => {
            let fields: Vec<_> = kind.split(':').collect();
            assert!(
                fields.len() == 3 || fields.len() == 4,
                "unknown shaped benchmark case: {kind}"
            );
            let cars = fields[2]
                .parse::<u8>()
                .expect("benchmark car count is an unsigned integer");
            let requested_speed_mps = fields
                .get(3)
                .map_or(GRID_MIDPOINT_ENTRY_SPEED_MPS, |speed| {
                    speed.parse::<f64>().expect("benchmark speed is a number")
                });
            (fields[1], Some(cars), fields[0], requested_speed_mps)
        }
    };
    let mut model = coaster_models()
        .into_iter()
        .find(|model| model.key == model_key)
        .unwrap_or_else(|| panic!("unknown shaped benchmark model: {model_key}"));
    if let Some(cars) = cars {
        assert!(
            model.cars_per_train.contains(&cars),
            "{cars} cars are not allowed for {model_key}"
        );
        model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
        model.train.cars = cars;
    }
    let intent = match element_kind {
        "floater-hill" => ElementIntent::Hill {
            feel: HillFeel::Floater,
            strength_g: default_hill_strength(HillFeel::Floater, &model),
            size: HillSizeHint::Auto,
        },
        "strong-floater-hill" => ElementIntent::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: if model.key == "hyper" {
                STRONG_FLOATER_TARGET_G
            } else {
                default_hill_strength(HillFeel::StrongFloater, &model)
            },
            size: HillSizeHint::Auto,
        },
        "ejector-hill" => ElementIntent::Hill {
            feel: HillFeel::Ejector,
            strength_g: default_hill_strength(HillFeel::Ejector, &model),
            size: HillSizeHint::Auto,
        },
        "fully-banked-turn" => ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        },
        "vertical-loop" => ElementIntent::Loop {
            hand: Hand::Left,
            peak_g: default_loop_peak_g(&model),
            top_feel: LoopTopFeel::Neutral,
        },
        "zero-g-roll" => ElementIntent::ZeroGRoll {
            hand: Hand::Left,
            peak_g: default_zero_g_roll_peak_g(&model),
        },
        "raven-turn" => ElementIntent::RavenTurn {
            hand: Hand::Left,
            peak_g: default_raven_turn_peak_g(&model),
            top_feel: LoopTopFeel::Neutral,
        },
        _ => panic!("unknown shaped benchmark element: {element_kind}"),
    };
    (model, intent, speed_mps)
}

fn seeded_preview_cache(
    prefix: &[Piece],
    model: &theme_park_sim::track::CoasterModel,
    physics: PhysicsConfig,
    expected_entry_speed_mps: f64,
) -> PreviewCache {
    let mut preview_cache = PreviewCache::default();
    let preview = preview_cache
        .preview(prefix, model, physics)
        .expect("lift prefix contains a station");
    assert_eq!(preview.status, PreviewStatus::ReachedEnd);
    let observed_entry_speed_mps = preview
        .report
        .samples
        .last()
        .expect("prefix preview records the lead car")
        .speed;
    assert!((observed_entry_speed_mps - expected_entry_speed_mps).abs() <= 0.01);
    preview_cache
}

fn candidate_car_count(preview: &TrackPreview, candidate_index: usize) -> usize {
    preview
        .report
        .car_piece_force_peaks()
        .into_iter()
        .filter(|peaks| peaks.piece_index == candidate_index)
        .count()
}

fn lift_prefix(model: &theme_park_sim::track::CoasterModel, target_speed_mps: f64) -> Vec<Piece> {
    let templates = catalog();
    let station_template = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .expect("catalog includes the station");
    let straight_template = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    let start = TrackState {
        x: 128,
        y: 256,
        z: 8,
        ..TrackState::origin()
    };
    let mut prefix = vec![
        station_template
            .at(start)
            .expect("station starts at the origin"),
    ];
    let physics = PhysicsConfig::default();
    let acceleration_distance_m = (theme_park_sim::math::powi(target_speed_mps, 2)
        - theme_park_sim::math::powi(physics.station_speed, 2))
    .max(0.0)
        / (2.0 * physics.drive_acceleration);
    let required_lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
    let straight_count = (required_lift_length_m / straight_template.length()).ceil() as usize;

    for _ in 0..straight_count {
        let mut lift = straight_template
            .at(prefix.last().expect("prefix has a station").exit)
            .expect("straight pieces connect");
        lift.kind = PieceKind::Lift;
        prefix.push(lift);
    }
    prefix
}

fn result_summary(
    kind: &str,
    model_key: &str,
    requested_entry_speed_mps: f64,
    observed_entry_speed_mps: f64,
    solved: &SolvedElement,
) -> Value {
    let mut result = json!({
        "status": "solved",
        "kind": kind,
        "model": model_key,
        "cars": solved.spec.cars(),
        "requested_entry_speed_mps": requested_entry_speed_mps,
        "observed_entry_speed_mps": observed_entry_speed_mps,
        "design_speed_mps": solved.spec.design_speed_mps(),
        "calibration_runs": solved.calibration_runs,
        "spec_hash": stable_hash(&postcard::to_allocvec(&solved.spec).expect("spec serializes")),
    });
    match &solved.spec {
        ElementSpec::Hill {
            target_g,
            delivered_g,
            height_m,
            span_m,
            ..
        } => {
            result["target_g"] = json!(target_g);
            result["delivered_g"] = json!(delivered_g);
            result["height_m"] = json!(height_m);
            result["span_m"] = json!(span_m);
        }
        ElementSpec::Turn {
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
            ..
        } => {
            result["vertical_g"] = json!(vertical_g);
            result["lateral_g"] = json!(lateral_g);
            result["radius_m"] = json!(radius_m);
            result["bank_degrees"] = json!(bank_degrees);
        }
        ElementSpec::Loop {
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            height_m,
            span_m,
            steel_structure,
            ..
        } => {
            result["target_peak_g"] = json!(target_peak_g);
            result["delivered_peak_g"] = json!(delivered_peak_g);
            result["delivered_top_g"] = json!(delivered_top_g);
            result["height_m"] = json!(height_m);
            result["span_m"] = json!(span_m);
            result["steel_structure"] = json!(steel_structure);
        }
        ElementSpec::ZeroGRoll {
            target_peak_g,
            delivered_peak_g,
            delivered_crest_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            ..
        } => {
            result["target_peak_g"] = json!(target_peak_g);
            result["delivered_peak_g"] = json!(delivered_peak_g);
            result["delivered_crest_g"] = json!(delivered_crest_g);
            result["peak_roll_rate_deg_s"] = json!(peak_roll_rate_deg_s);
            result["height_m"] = json!(height_m);
            result["span_m"] = json!(span_m);
        }
        ElementSpec::RavenTurn {
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            ..
        } => {
            result["target_peak_g"] = json!(target_peak_g);
            result["delivered_peak_g"] = json!(delivered_peak_g);
            result["delivered_top_g"] = json!(delivered_top_g);
            result["peak_roll_rate_deg_s"] = json!(peak_roll_rate_deg_s);
            result["height_m"] = json!(height_m);
            result["span_m"] = json!(span_m);
        }
    }
    result
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
