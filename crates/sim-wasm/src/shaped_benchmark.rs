use serde_json::{Value, json};
use theme_park_sim::physics::{PhysicsConfig, PreviewCache, PreviewStatus, TrackPreview};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, SolveContext, SolvedElement, TurnSizeHint, solve,
};
use theme_park_sim::track::{
    Hand, HillFeel, Piece, PieceId, PieceKind, TrackState, catalog, coaster_models,
};
use wasm_bindgen::prelude::*;

const HYPER_HILL_ENTRY_SPEED_MPS: f64 = 34.0;
const HYPER_TURN_ENTRY_SPEED_MPS: f64 = 28.0;
const GRID_MIDPOINT_ENTRY_SPEED_MPS: f64 = 30.25;

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
        serde_json::to_string(&result_summary(
            &self.kind,
            self.model.key,
            self.requested_entry_speed_mps,
            self.observed_entry_speed_mps,
            solved,
        ))
        .expect("benchmark result serializes")
    }
}

/// Lists the interpolation cases across every model and allowed train length.
#[wasm_bindgen]
pub fn shaped_benchmark_cases_json() -> String {
    let cases: Vec<_> = coaster_models()
        .into_iter()
        .flat_map(|model| {
            let model_key = model.key;
            model.cars_per_train.clone().flat_map(move |cars| {
                ["floater-hill", "fully-banked-turn"].map(move |element| {
                    json!({
                        "id": format!("{element}:{model_key}:{cars}"),
                        "model": model_key,
                        "cars": cars,
                        "element": element,
                        "requested_speed_mps": GRID_MIDPOINT_ENTRY_SPEED_MPS,
                    })
                })
            })
        })
        .collect();
    serde_json::to_string(&cases).expect("benchmark cases serialize")
}

fn benchmark_case(kind: &str) -> (theme_park_sim::track::CoasterModel, ElementIntent, f64) {
    let (model_key, cars, intent, speed_mps) = match kind {
        "hyper-strong-floater-hill" => (
            "hyper",
            None,
            ElementIntent::Hill {
                feel: HillFeel::StrongFloater,
                strength_g: None,
                size: HillSizeHint::Auto,
            },
            HYPER_HILL_ENTRY_SPEED_MPS,
        ),
        "hyper-fully-banked-turn" => (
            "hyper",
            None,
            ElementIntent::Turn {
                hand: Hand::Left,
                size: TurnSizeHint::Auto,
            },
            HYPER_TURN_ENTRY_SPEED_MPS,
        ),
        _ => {
            let fields: Vec<_> = kind.split(':').collect();
            assert_eq!(fields.len(), 3, "unknown shaped benchmark case: {kind}");
            let intent = match fields[0] {
                "floater-hill" => ElementIntent::Hill {
                    feel: HillFeel::Floater,
                    strength_g: None,
                    size: HillSizeHint::Auto,
                },
                "fully-banked-turn" => ElementIntent::Turn {
                    hand: Hand::Left,
                    size: TurnSizeHint::Auto,
                },
                _ => panic!("unknown shaped benchmark element: {}", fields[0]),
            };
            let cars = fields[2]
                .parse::<u8>()
                .expect("benchmark car count is an unsigned integer");
            (fields[1], Some(cars), intent, GRID_MIDPOINT_ENTRY_SPEED_MPS)
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
    let mut prefix = vec![
        station_template
            .at(TrackState::origin())
            .expect("station starts at the origin"),
    ];
    let physics = PhysicsConfig::default();
    let acceleration_distance_m = (target_speed_mps.powi(2) - physics.station_speed.powi(2))
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
    }
    result
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}
