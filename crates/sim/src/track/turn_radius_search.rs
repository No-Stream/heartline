use crate::physics::{
    Circuit, ConstructionTestRunRecorder, PhysicsConfig, PreviewCache, PreviewStatus, Train,
};
use crate::track::curves::{speed_turn_geometry, speed_turn_radius_for_peak_bank};
use crate::track::shaped::{ElementSpec, piece_from_spec};
use crate::track::{
    CoasterModel, Hand, Piece, PieceId, PieceKind, TrackState, catalog, coaster_models,
};

const SPEED_OFFSETS_MPS: [f64; 3] = [-0.5, 0.0, 0.5];
const RADIUS_OFFSETS: [i8; 13] = [-6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6];
const MAXIMUM_LATERAL_G: f64 = 0.1;
const MINIMUM_FORCE_HEADROOM_G: f64 = 0.25;
const MAXIMUM_BANK_DEGREES: f64 = 75.0;

#[derive(Debug)]
struct RadiusMeasurement {
    radius_m: f64,
    minimum_track_radius_m: f64,
    design_lateral_g: f64,
    maximum_lateral_g: f64,
    minimum_force_headroom_g: f64,
    passed: bool,
}

#[test]
#[ignore = "bounded connector-radius search for representative fully banked turns"]
fn search_nearby_grid_radii_for_failed_turns_in_every_model() {
    let models = coaster_models();
    let cases = [
        ("wooden_twister", 4, 25.0),
        ("hybrid", 4, 25.0),
        ("velocity_viper", 3, 25.0),
        ("family_coaster", 4, 14.0),
        ("hyper", 4, 25.0),
    ];

    for (model_key, cars, speed_mps) in cases {
        let base_model = models
            .iter()
            .find(|model| model.key == model_key)
            .expect("diagnostic model exists");
        let model = model_with_train_length(base_model, cars);
        let rules = model.shaped_element_rules();
        let target_bank = (1.0 / rules.turn_vertical_g).acos();
        let ideal_radius_m = speed_turn_radius_for_peak_bank(target_bank, speed_mps);
        let baseline_radius_m =
            (ideal_radius_m.max(rules.minimum_turn_radius_m) / 2.0).ceil() * 2.0;
        let mut results = Vec::with_capacity(RADIUS_OFFSETS.len());

        for radius_offset in RADIUS_OFFSETS {
            let radius_m = baseline_radius_m + f64::from(radius_offset) * 2.0;
            if radius_m < 2.0 || radius_m > rules.maximum_turn_radius_m {
                continue;
            }
            results.push(measure_radius(&model, speed_mps, radius_m));
        }

        let passing = results
            .iter()
            .filter(|result| result.passed)
            .map(|result| result.radius_m)
            .collect::<Vec<_>>();
        let best = results.iter().min_by(|left, right| {
            let left_score = (MAXIMUM_LATERAL_G - left.design_lateral_g).abs()
                + (MINIMUM_FORCE_HEADROOM_G - left.minimum_force_headroom_g).max(0.0)
                + (model.minimum_radius_metres - left.minimum_track_radius_m).max(0.0);
            let right_score = (MAXIMUM_LATERAL_G - right.design_lateral_g).abs()
                + (MINIMUM_FORCE_HEADROOM_G - right.minimum_force_headroom_g).max(0.0)
                + (model.minimum_radius_metres - right.minimum_track_radius_m).max(0.0);
            left_score.total_cmp(&right_score)
        });

        eprintln!(
            "radius search model={model_key} cars={cars} speed={speed_mps:.1} baseline={baseline_radius_m:.1} candidates={} passing={passing:?} best={best:?}",
            results.len()
        );
        for result in &results {
            eprintln!(
                "  radius={:.1} min_track_radius={:.3} design_lateral={:.5} max_lateral_six={:.5} min_headroom_six={:.5} passed={}",
                result.radius_m,
                result.minimum_track_radius_m,
                result.design_lateral_g,
                result.maximum_lateral_g,
                result.minimum_force_headroom_g,
                result.passed
            );
        }
    }
}

fn measure_radius(model: &CoasterModel, design_speed_mps: f64, radius_m: f64) -> RadiusMeasurement {
    let geometry = (*speed_turn_geometry(radius_m, design_speed_mps)).clone();
    let bank_degrees = geometry.peak_bank.to_degrees();
    let spec = ElementSpec::Turn {
        design_speed_mps,
        cars: model.train.cars,
        hand: Hand::Left,
        vertical_g: 1.0 / geometry.peak_bank.cos(),
        lateral_g: 0.0,
        radius_m,
        bank_degrees,
        geometry,
    };
    let design_prefix = lift_prefix(model, design_speed_mps, PhysicsConfig::default());
    let piece = piece_from_spec(
        design_prefix.last().expect("prefix contains station").exit,
        &spec,
    )
    .expect("even-radius turn ends at a grid connector");
    let minimum_track_radius_m = piece.maximum_curvature().recip();
    if minimum_track_radius_m < model.minimum_radius_metres || bank_degrees > MAXIMUM_BANK_DEGREES {
        return RadiusMeasurement {
            radius_m,
            minimum_track_radius_m,
            design_lateral_g: f64::INFINITY,
            maximum_lateral_g: f64::INFINITY,
            minimum_force_headroom_g: f64::NEG_INFINITY,
            passed: false,
        };
    }

    let mut minimum_force_headroom_g = f64::INFINITY;
    let mut design_lateral_g = 0.0_f64;
    let mut maximum_lateral_g = 0.0_f64;
    let mut all_scenarios_reached_end = true;

    for speed_offset_mps in SPEED_OFFSETS_MPS {
        let speed_mps = design_speed_mps + speed_offset_mps;
        let runtime_preview = preview_runtime_candidate(model, &spec, speed_mps);
        all_scenarios_reached_end &= runtime_preview.0;
        minimum_force_headroom_g = minimum_force_headroom_g.min(runtime_preview.1);
        maximum_lateral_g = maximum_lateral_g.max(runtime_preview.2);
        if speed_offset_mps == 0.0 {
            design_lateral_g = design_lateral_g.max(runtime_preview.2);
        }

        let construction_preview = preview_construction_candidate(model, &spec, speed_mps);
        all_scenarios_reached_end &= construction_preview.0;
        minimum_force_headroom_g = minimum_force_headroom_g.min(construction_preview.1);
        maximum_lateral_g = maximum_lateral_g.max(construction_preview.2);
        if speed_offset_mps == 0.0 {
            design_lateral_g = design_lateral_g.max(construction_preview.2);
        }
    }

    RadiusMeasurement {
        radius_m,
        minimum_track_radius_m,
        design_lateral_g,
        maximum_lateral_g,
        minimum_force_headroom_g,
        passed: all_scenarios_reached_end
            && design_lateral_g <= MAXIMUM_LATERAL_G
            && minimum_force_headroom_g >= MINIMUM_FORCE_HEADROOM_G,
    }
}

fn preview_runtime_candidate(
    model: &CoasterModel,
    spec: &ElementSpec,
    speed_mps: f64,
) -> (bool, f64, f64) {
    let prefix = lift_prefix(model, speed_mps, PhysicsConfig::default());
    let candidate =
        match piece_from_spec(prefix.last().expect("prefix contains station").exit, spec) {
            Ok(candidate) => candidate,
            Err(_) => return (false, f64::NEG_INFINITY, f64::INFINITY),
        };
    let pieces = candidate_pieces(&prefix, &candidate, &model.train);
    let mut cache = PreviewCache::default();
    let Some(preview) = cache.preview_candidate(
        &pieces,
        model,
        PhysicsConfig {
            lift_speed: speed_mps,
            ..PhysicsConfig::default()
        },
        prefix.len(),
    ) else {
        return (false, f64::NEG_INFINITY, f64::INFINITY);
    };
    if preview.status != PreviewStatus::ReachedEnd
        || preview.cars.len() != usize::from(model.train.cars)
    {
        return (false, f64::NEG_INFINITY, f64::INFINITY);
    }
    let limits = model.comfortable_force_limits;
    let minimum_headroom_g = preview
        .cars
        .iter()
        .map(|car| force_headroom(car.peaks, limits))
        .reduce(f64::min)
        .unwrap_or(f64::NEG_INFINITY);
    let maximum_lateral_g = preview
        .cars
        .iter()
        .map(|car| car.peaks.maximum_lateral_g)
        .reduce(f64::max)
        .unwrap_or(f64::INFINITY);
    (true, minimum_headroom_g, maximum_lateral_g)
}

fn preview_construction_candidate(
    model: &CoasterModel,
    spec: &ElementSpec,
    speed_mps: f64,
) -> (bool, f64, f64) {
    let prefix = lift_prefix(model, speed_mps, PhysicsConfig::default());
    let candidate =
        match piece_from_spec(prefix.last().expect("prefix contains station").exit, spec) {
            Ok(candidate) => candidate,
            Err(_) => return (false, f64::NEG_INFINITY, f64::INFINITY),
        };
    let pieces = candidate_pieces(&prefix, &candidate, &model.train);
    let circuit = Circuit::open(pieces).expect("candidate circuit is open and connected");
    let candidate_start_m = prefix.iter().map(Piece::length).sum::<f64>();
    let candidate_end_m = candidate_start_m + candidate.length();
    let mut train = Train::new(
        &circuit,
        &model.train,
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
        candidate_start_m,
        speed_mps,
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
        &circuit,
        &model.train,
        model,
    );
    let clear_candidate_distance_m = candidate_end_m + model.train.length_metres;
    for _ in 0..100_000 {
        train.step(&circuit);
        recorder.record(&train.take_readings(), model.name);
        if !train.notes().is_empty() {
            return (false, f64::NEG_INFINITY, f64::INFINITY);
        }
        if train.distance() >= clear_candidate_distance_m {
            let report = recorder.finish(false, false);
            let candidate_index = prefix.len();
            let force_peaks = report
                .car_piece_force_peaks()
                .into_iter()
                .filter(|peaks| peaks.piece_index == candidate_index)
                .collect::<Vec<_>>();
            if force_peaks.len() != usize::from(model.train.cars) {
                return (false, f64::NEG_INFINITY, f64::INFINITY);
            }
            let limits = model.comfortable_force_limits;
            let minimum_headroom_g = force_peaks
                .iter()
                .map(|peaks| force_headroom(peaks.peaks, limits))
                .reduce(f64::min)
                .unwrap_or(f64::NEG_INFINITY);
            let maximum_lateral_g = force_peaks
                .iter()
                .map(|peaks| peaks.peaks.maximum_lateral_g)
                .reduce(f64::max)
                .unwrap_or(f64::INFINITY);
            return (true, minimum_headroom_g, maximum_lateral_g);
        }
    }
    (false, f64::NEG_INFINITY, f64::INFINITY)
}

fn candidate_pieces(
    prefix: &[Piece],
    candidate: &Piece,
    train: &crate::track::TrainConfiguration,
) -> Vec<Piece> {
    let straight = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog has straight track");
    let mut pieces = prefix.to_vec();
    pieces.push(candidate.clone());
    let runout_count = (train.length_metres / straight.length()).ceil() as usize + 2;
    let mut state = candidate.exit;
    for _ in 0..runout_count {
        let piece = straight.at(state).expect("runout pieces connect");
        state = piece.exit;
        pieces.push(piece);
    }
    pieces
}

fn lift_prefix(model: &CoasterModel, speed_mps: f64, physics: PhysicsConfig) -> Vec<Piece> {
    let templates = catalog();
    let station = templates
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .expect("catalog has station");
    let straight = templates
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog has straight");
    let mut pieces = vec![
        station
            .at(TrackState::origin())
            .expect("station starts at origin"),
    ];
    let acceleration_distance_m = (speed_mps.powi(2) - physics.station_speed.powi(2)).max(0.0)
        / (2.0 * physics.drive_acceleration);
    let total_lift_length_m = acceleration_distance_m + model.train.length_metres + 40.0;
    let straight_count = (total_lift_length_m / straight.length()).ceil() as usize;
    for _ in 0..straight_count {
        let mut lift = straight
            .at(pieces.last().expect("prefix has station").exit)
            .expect("lift pieces connect");
        lift.kind = PieceKind::Lift;
        pieces.push(lift);
    }
    pieces
}

fn model_with_train_length(base_model: &CoasterModel, cars: u8) -> CoasterModel {
    let mut model = base_model.clone();
    model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
    model.train.cars = cars;
    model
}

fn force_headroom(
    peaks: crate::physics::ForcePeakValues,
    limits: crate::track::ComfortableForceLimits,
) -> f64 {
    (peaks.minimum_vertical_g - limits.minimum_vertical_g)
        .min(limits.maximum_vertical_g - peaks.maximum_vertical_g)
        .min(limits.maximum_lateral_g - peaks.maximum_lateral_g)
        .min(limits.maximum_longitudinal_g - peaks.maximum_longitudinal_g)
}
