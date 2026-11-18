use std::hint::black_box;
use std::time::{Duration, Instant};

use theme_park_sim::physics::{PhysicsConfig, preview_open_track};
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    CoasterModel, Heading, Piece, PieceId, TrackState, catalog, hybrid_model, multi_launch_model,
    wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");
const WARMUP_RUNS: usize = 5;
const MEASURED_RUNS: usize = 100;

fn main() {
    for (name, model) in [
        ("Timber Tumbler", hybrid_model()),
        ("Velocity Viper", multi_launch_model()),
        ("Briar Blitz", wooden_twister_model()),
    ] {
        let pieces = build_player_circuit(name, model.clone());
        measure_preview(name, &pieces, &model);
    }
}

fn build_player_circuit(name: &str, model: CoasterModel) -> Vec<Piece> {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 482,
        y: 498,
        z: 6,
        heading: Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model);
    construction.append(PieceId::Station, &world, &[]).unwrap();

    let header = format!("[{name}]");
    let mut sections = CIRCUITS.split(&header);
    sections.next().unwrap();
    let section = sections
        .next()
        .unwrap_or_else(|| panic!("{name} circuit missing"));
    assert!(sections.next().is_none(), "duplicate {name} circuit");
    for piece_name in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        match piece_name {
            "+chain" => {
                construction.set_chain(true);
                continue;
            }
            "-chain" => {
                construction.set_chain(false);
                continue;
            }
            _ => {}
        }
        let piece = catalog()
            .iter()
            .find(|piece| format!("{:?}", piece.id) == piece_name)
            .unwrap_or_else(|| panic!("unknown piece {piece_name}"));
        construction
            .append(piece.id, &world, &[])
            .unwrap_or_else(|error| panic!("{piece_name}: {error:?}"));
    }
    construction.close().unwrap();
    construction.pieces().to_vec()
}

fn measure_preview(name: &str, pieces: &[Piece], model: &CoasterModel) {
    let config = PhysicsConfig::default();
    let warmup = preview_open_track(pieces, &model.train, config, model.name)
        .unwrap_or_else(|| panic!("{name} has no station"));
    let status = warmup.status;
    let sample_count = warmup.report.samples.len();
    let furthest_distance_metres = warmup.report.furthest_distance;
    black_box(warmup);
    for _ in 1..WARMUP_RUNS {
        let report = preview_open_track(pieces, &model.train, config, model.name)
            .unwrap_or_else(|| panic!("{name} has no station"));
        assert_eq!(report.status, status, "{name} preview status changed");
        assert_eq!(report.report.samples.len(), sample_count);
        assert_eq!(report.report.furthest_distance, furthest_distance_metres);
        black_box(report);
    }

    let mut elapsed = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let started_at = Instant::now();
        let report = preview_open_track(pieces, &model.train, config, model.name)
            .unwrap_or_else(|| panic!("{name} has no station"));
        let duration = started_at.elapsed();
        assert_eq!(report.status, status, "{name} preview status changed");
        assert_eq!(report.report.samples.len(), sample_count);
        assert_eq!(report.report.furthest_distance, furthest_distance_metres);
        black_box(report);
        elapsed.push(duration);
    }
    elapsed.sort_unstable();

    let median = (elapsed[MEASURED_RUNS / 2 - 1] + elapsed[MEASURED_RUNS / 2]) / 2;
    let p95 = elapsed[(MEASURED_RUNS * 95).div_ceil(100) - 1];
    let maximum = elapsed[MEASURED_RUNS - 1];
    let length_metres: f64 = pieces.iter().map(Piece::length).sum();
    println!(
        "{name}: {} pieces, {:.1} m, {:?} at {:.1} m with {sample_count} samples; median {:.3} ms, p95 {:.3} ms, max {:.3} ms",
        pieces.len(),
        length_metres,
        status,
        furthest_distance_metres,
        milliseconds(median),
        milliseconds(p95),
        milliseconds(maximum),
    );
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
