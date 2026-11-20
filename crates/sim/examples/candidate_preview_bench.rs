use std::hint::black_box;
use std::time::{Duration, Instant};

use theme_park_sim::physics::{ForcePeakValues, PhysicsConfig, PreviewCache, PreviewStatus};
use theme_park_sim::track::{Heading, Piece, PieceId, TrackState, catalog, multi_launch_model};

const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 100;
type PreviewSummary = (
    PreviewStatus,
    Option<f64>,
    Option<f64>,
    Vec<(usize, ForcePeakValues, Option<f64>)>,
);

fn main() {
    let model = multi_launch_model();
    let (prefix, candidate_track, candidate_index) = build_fixture(&model);
    let config = PhysicsConfig {
        lift_speed: 5.0,
        ..PhysicsConfig::default()
    };

    let expected = measure_once(&prefix, &candidate_track, candidate_index, &model, config);
    assert_eq!(expected.0, PreviewStatus::ReachedEnd);
    assert_eq!(expected.3.len(), usize::from(model.train.cars));

    for _ in 1..WARMUP_RUNS {
        assert_eq!(
            measure_once(&prefix, &candidate_track, candidate_index, &model, config),
            expected
        );
    }

    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let mut cache = PreviewCache::default();
        let prefix_preview = cache
            .preview(&prefix, &model, config)
            .expect("station prefix");
        assert_eq!(prefix_preview.status, PreviewStatus::ReachedEnd);

        let started_at = Instant::now();
        let preview = cache
            .preview_candidate(&candidate_track, &model, config, candidate_index)
            .expect("candidate preview");
        elapsed.push(started_at.elapsed());

        let observed = (
            preview.status,
            prefix_preview.end_speed_mps,
            preview.end_speed_mps,
            preview
                .cars
                .iter()
                .map(|car| (car.car_index, car.peaks, car.sustained_minimum_vertical_g))
                .collect(),
        );
        assert_eq!(observed, expected);
        black_box(preview);
    }
    elapsed.sort_unstable();

    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    let maximum = elapsed[MEASURED_RUNS - 1];
    let prefix_length: f64 = prefix.iter().map(Piece::length).sum();
    let track_length: f64 = candidate_track.iter().map(Piece::length).sum();
    println!(
        "candidate preview: model={}, train_cars={}, candidate={:?}, prefix_pieces={}, prefix_m={prefix_length:.1}, candidate_track_m={track_length:.1}, status={:?}, entry_speed_mps={:.2}, end_speed_mps={:.2}, per_car_records={}; median={:.3} ms, p95={:.3} ms, max={:.3} ms",
        model.name,
        model.train.cars,
        candidate_track[candidate_index].id,
        prefix.len(),
        expected.0,
        expected.1.unwrap_or_default(),
        expected.2.unwrap_or_default(),
        expected.3.len(),
        milliseconds(median),
        milliseconds(p95),
        milliseconds(maximum),
    );
}

fn build_fixture(model: &theme_park_sim::track::CoasterModel) -> (Vec<Piece>, Vec<Piece>, usize) {
    let mut state = TrackState {
        x: 96,
        y: 96,
        z: 6,
        heading: Heading::East,
        ..TrackState::origin()
    };
    let station = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Station)
        .and_then(|piece| piece.at(state))
        .expect("catalog station fits fixture state");
    state = station.exit;
    let mut prefix = vec![station];
    let launch = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::LaunchLong)
        .and_then(|piece| piece.at(state))
        .expect("classic long launch fits fixture entry");
    state = launch.exit;
    prefix.push(launch);
    let straight_template = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::Straight)
        .expect("catalog includes straight track");
    for _ in 0..2 {
        let straight = straight_template
            .at(state)
            .expect("straight follows fixture prefix");
        state = straight.exit;
        prefix.push(straight);
    }

    let hill = catalog()
        .iter()
        .find(|piece| piece.id == PieceId::FloaterHill4x48)
        .and_then(|piece| piece.at(state))
        .expect("classic floater hill fits fixture entry");
    state = hill.exit;
    let candidate_index = prefix.len();
    let mut candidate_track = prefix.clone();
    candidate_track.push(hill);
    let runout_count = (model.train.length_metres / straight_template.length()).ceil() as usize + 2;
    for _ in 0..runout_count {
        let straight = straight_template
            .at(state)
            .expect("straight follows classic hill exit");
        state = straight.exit;
        candidate_track.push(straight);
    }
    (prefix, candidate_track, candidate_index)
}

fn measure_once(
    prefix: &[Piece],
    candidate_track: &[Piece],
    candidate_index: usize,
    model: &theme_park_sim::track::CoasterModel,
    config: PhysicsConfig,
) -> PreviewSummary {
    let mut cache = PreviewCache::default();
    let prefix_preview = cache
        .preview(prefix, model, config)
        .expect("station prefix");
    assert_eq!(prefix_preview.status, PreviewStatus::ReachedEnd);
    let preview = cache
        .preview_candidate(candidate_track, model, config, candidate_index)
        .expect("candidate preview");
    (
        preview.status,
        prefix_preview.end_speed_mps,
        preview.end_speed_mps,
        preview
            .cars
            .iter()
            .map(|car| (car.car_index, car.peaks, car.sustained_minimum_vertical_g))
            .collect(),
    )
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
