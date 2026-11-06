use super::comfort::{EnvelopeExceedance, EnvelopeWorst};
use super::features::{ElementClass, Location, RideFeatures, ride_features};
use super::scoring::{ForceBandDurations, Scores, Term, score};
use super::testing::RideStatistics;
use super::{CarReading, ConstructionTestRunRecorder, GForces, PhysicsConfig, Train, demo_circuit};
use crate::track::construction::Construction;
use crate::track::{
    CoasterModel, Heading, Heartlining, Inversion, PieceId, TrackState, catalog, coaster_models,
    hybrid_model, wooden_twister_model,
};
use crate::world::{Surface, World};
use std::collections::{BTreeMap, BTreeSet};

struct Archetype {
    name: &'static str,
    features: RideFeatures,
    bands: ForceBandDurations,
}

#[derive(Clone, Copy)]
struct ScoreBand {
    minimum: f64,
    maximum: f64,
}

struct ExpectedScores {
    excitement: ScoreBand,
    intensity: Option<ScoreBand>,
    nausea: Option<ScoreBand>,
}

const fn score_band(minimum: f64, maximum: f64) -> ScoreBand {
    ScoreBand { minimum, maximum }
}

fn empty_features() -> RideFeatures {
    ride_features(&[], &[], &[], 0.0, false)
}

fn score_family_wooden() -> Archetype {
    let mut features = empty_features();
    features.ride_seconds = 40.0;
    features.active_airtime_moments = 3;
    features.active_inverted_seconds = 0.0;
    features.active_average_speed = 18.0;
    features.statistics = RideStatistics {
        floater_airtime_seconds: 1.5,
        airtime_moments: 3,
        max_positive_vertical_g: 2.0,
        // Assume a 1 g peak to make the specified strong lateral exposure possible.
        max_lateral_g: 1.0,
        max_speed: 18.0,
        average_speed: 18.0,
        duration_seconds: 40.0,
        track_length: 720.0,
        biggest_drop: 15.0,
        ..RideStatistics::default()
    };
    features.element_classes = BTreeSet::from([
        ElementClass::FloaterHill,
        // The plan specifies three classes but a 15 m drop is not a BigDrop.
        ElementClass::Helix,
        ElementClass::BankedTurn,
    ]);
    Archetype {
        name: "Family wooden coaster",
        features,
        bands: ForceBandDurations {
            airtime_good: 1.5,
            lateral_strong: 0.5,
            ..ForceBandDurations::default()
        },
    }
}

fn score_classic_arrow_looper() -> Archetype {
    let location = Location {
        piece_index: 4,
        piece_name: "Pull-out",
        distance_m: 90.0,
        seconds_from_start: 12.0,
    };
    let mut features = empty_features();
    features.ride_seconds = 60.0;
    features.active_airtime_moments = 1;
    features.active_inverted_seconds = 0.0;
    features.active_average_speed = 19.0;
    features.inversion_passages = vec![
        (Inversion::VerticalLoop, Heartlining::NotHeartlined),
        (Inversion::VerticalLoop, Heartlining::NotHeartlined),
        (Inversion::Corkscrew, Heartlining::NotHeartlined),
        (Inversion::Corkscrew, Heartlining::NotHeartlined),
    ];
    features.element_classes = BTreeSet::from([
        ElementClass::FloaterHill,
        ElementClass::BigDrop,
        ElementClass::VerticalLoop,
        ElementClass::Corkscrew,
    ]);
    features.dead_excess_seconds = 6.0;
    features.longest_dead_run = Some((location, 9.0));
    features.reversal_count = 5;
    features.envelope = EnvelopeExceedance {
        g_seconds: 0.45,
        seconds: 1.8,
        worst: Some(EnvelopeWorst {
            location,
            axis: super::comfort::EnvelopeAxis::Vertical,
            value: 5.5,
            allowed: 4.0,
            ratio: 0.375,
        }),
        stretches: Vec::new(),
    };
    features.statistics = RideStatistics {
        floater_airtime_seconds: 1.0,
        airtime_moments: 1,
        max_positive_vertical_g: 5.5,
        max_negative_vertical_g: -1.2,
        max_lateral_g: 1.5,
        max_speed: 25.0,
        average_speed: 19.0,
        duration_seconds: 60.0,
        track_length: 1140.0,
        biggest_drop: 35.0,
        inversions: 4,
        ..RideStatistics::default()
    };
    Archetype {
        name: "Classic Arrow looper",
        features,
        bands: ForceBandDurations {
            airtime_good: 1.0,
            vertical_great: 2.5,
            lateral_strong: 2.5,
            too_much: 1.5,
            ..ForceBandDurations::default()
        },
    }
}

fn score_b_and_m_hyper() -> Archetype {
    let mut features = empty_features();
    features.ride_seconds = 70.0;
    features.active_airtime_moments = 7;
    features.active_inverted_seconds = 0.0;
    features.active_average_speed = 30.0;
    features.element_classes = BTreeSet::from([
        ElementClass::EjectorHill,
        ElementClass::FloaterHill,
        ElementClass::BigDrop,
        ElementClass::BankedTurn,
        ElementClass::Helix,
    ]);
    features.statistics = RideStatistics {
        ejector_airtime_seconds: 0.5,
        floater_airtime_seconds: 6.0,
        airtime_moments: 7,
        strong_positive_g_seconds: 3.0,
        max_positive_vertical_g: 4.5,
        max_negative_vertical_g: -1.1,
        max_lateral_g: 1.0,
        max_speed: 35.0,
        average_speed: 30.0,
        duration_seconds: 70.0,
        track_length: 2100.0,
        biggest_drop: 60.0,
        ..RideStatistics::default()
    };
    Archetype {
        name: "B&M hyper",
        features,
        bands: ForceBandDurations {
            airtime_good: 6.0,
            airtime_great: 0.5,
            vertical_great: 3.0,
            ..ForceBandDurations::default()
        },
    }
}

fn score_rmc_hybrid() -> Archetype {
    let mut features = empty_features();
    features.ride_seconds = 100.0;
    features.active_airtime_moments = 15;
    features.active_inverted_seconds = 0.0;
    features.active_average_speed = 27.0;
    features.inversion_passages = vec![
        (Inversion::VerticalLoop, Heartlining::Heartlined),
        (Inversion::Corkscrew, Heartlining::Heartlined),
    ];
    features.element_classes = BTreeSet::from([
        ElementClass::EjectorHill,
        ElementClass::FloaterHill,
        ElementClass::BigDrop,
        ElementClass::VerticalLoop,
        ElementClass::Corkscrew,
        ElementClass::TopHat,
        ElementClass::BankedTurn,
        ElementClass::NearMiss,
    ]);
    features.snap_count = 8;
    features.max_roll_rate_deg_s = 150.0;
    features.roll_rate_seconds = 2.0;
    features.proximity_strength = 1.0;
    features.proximity_counts.headchopper = 1;
    features.statistics = RideStatistics {
        ejector_airtime_seconds: 9.0,
        floater_airtime_seconds: 2.0,
        airtime_moments: 15,
        strong_lateral_g_seconds: 3.0,
        max_positive_vertical_g: 4.5,
        max_negative_vertical_g: -1.3,
        max_lateral_g: 1.4,
        max_speed: 32.0,
        average_speed: 27.0,
        duration_seconds: 100.0,
        track_length: 2700.0,
        biggest_drop: 55.0,
        inversions: 2,
        ..RideStatistics::default()
    };
    Archetype {
        name: "RMC hybrid",
        features,
        bands: ForceBandDurations {
            airtime_good: 2.0,
            airtime_great: 9.0,
            lateral_strong: 3.0,
            vertical_great: 3.0,
            ..ForceBandDurations::default()
        },
    }
}

fn score_intamin_accelerator() -> Archetype {
    let mut features = empty_features();
    features.ride_seconds = 20.0;
    features.active_airtime_moments = 0;
    features.active_inverted_seconds = 0.0;
    features.active_average_speed = 30.0;
    features.element_classes = BTreeSet::from([
        ElementClass::EjectorHill,
        ElementClass::TopHat,
        ElementClass::BigDrop,
        ElementClass::Launch,
    ]);
    features.statistics = RideStatistics {
        ejector_airtime_seconds: 1.5,
        max_positive_vertical_g: 4.5,
        max_negative_vertical_g: -1.2,
        max_forward_g: 1.7,
        max_speed: 57.0,
        average_speed: 30.0,
        duration_seconds: 20.0,
        track_length: 600.0,
        biggest_drop: 120.0,
        launches: 1,
        launch_peak_speed: 57.0,
        ..RideStatistics::default()
    };
    Archetype {
        name: "Intamin accelerator",
        features,
        bands: ForceBandDurations {
            airtime_great: 1.5,
            vertical_great: 0.8,
            longitudinal_strong: 3.0,
            ..ForceBandDurations::default()
        },
    }
}

fn score_intamin_blitz() -> Archetype {
    let mut features = empty_features();
    features.ride_seconds = 85.0;
    features.active_airtime_moments = 10;
    features.active_average_speed = 27.0;
    features.inversion_passages = vec![
        (Inversion::VerticalLoop, Heartlining::Heartlined),
        (Inversion::Corkscrew, Heartlining::Heartlined),
        (Inversion::HeartlineRoll, Heartlining::Heartlined),
    ];
    features.element_classes = BTreeSet::from([
        ElementClass::EjectorHill,
        ElementClass::FloaterHill,
        ElementClass::TopHat,
        ElementClass::BigDrop,
        ElementClass::Launch,
        ElementClass::VerticalLoop,
        ElementClass::Corkscrew,
        ElementClass::BankedTurn,
    ]);
    features.snap_count = 6;
    features.max_roll_rate_deg_s = 130.0;
    features.roll_rate_seconds = 1.5;
    features.statistics = RideStatistics {
        ejector_airtime_seconds: 5.0,
        floater_airtime_seconds: 3.0,
        airtime_moments: 10,
        max_positive_vertical_g: 4.5,
        max_negative_vertical_g: -1.2,
        max_lateral_g: 1.2,
        max_forward_g: 1.3,
        max_speed: 32.0,
        average_speed: 27.0,
        duration_seconds: 85.0,
        track_length: 2295.0,
        biggest_drop: 45.0,
        launches: 3,
        launch_peak_speed: 32.0,
        inversions: 3,
        ..RideStatistics::default()
    };
    Archetype {
        name: "Intamin Blitz multi-launch",
        features,
        bands: ForceBandDurations {
            airtime_good: 3.0,
            airtime_great: 5.0,
            vertical_great: 3.0,
            lateral_strong: 1.5,
            longitudinal_strong: 3.0,
            ..ForceBandDurations::default()
        },
    }
}

fn archetypes() -> [Archetype; 6] {
    [
        score_family_wooden(),
        score_classic_arrow_looper(),
        score_b_and_m_hyper(),
        score_rmc_hybrid(),
        score_intamin_accelerator(),
        score_intamin_blitz(),
    ]
}

fn print_contributions(name: &str, score: &Scores) {
    println!(
        "{name}: excitement {:.2}, intensity {:.2}, nausea {:.2}",
        score.excitement, score.intensity, score.nausea
    );
    for (label, contributions) in [
        ("excitement", &score.breakdown.excitement),
        ("intensity", &score.breakdown.intensity),
        ("nausea", &score.breakdown.nausea),
    ] {
        let details = contributions
            .iter()
            .filter(|item| item.value.abs() >= 0.01)
            .map(|item| {
                format!(
                    "{}={:+.2} ({:.1} {})",
                    item.term.label(),
                    item.value,
                    item.measure,
                    item.term.unit()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {label}: {details}");
    }
}

#[test]
fn calibration_table_prints_synthetic_archetype_scores_and_breakdowns() {
    println!("Synthetic coaster archetypes; selected SCORING weights");
    let mut measured = [(0.0, 0.0, 0.0); 6];
    let mut band_failures = Vec::new();
    for (index, (archetype, expected)) in archetypes()
        .into_iter()
        .zip([
            ExpectedScores {
                excitement: score_band(1.5, 3.0),
                intensity: Some(score_band(2.0, 3.5)),
                nausea: Some(score_band(0.3, 1.5)),
            },
            ExpectedScores {
                excitement: score_band(4.0, 5.0),
                intensity: Some(score_band(6.0, 7.5)),
                nausea: Some(score_band(4.5, 7.0)),
            },
            ExpectedScores {
                excitement: score_band(6.0, 7.5),
                intensity: Some(score_band(5.0, 6.5)),
                nausea: None,
            },
            ExpectedScores {
                excitement: score_band(9.5, 11.0),
                intensity: Some(score_band(7.0, 8.5)),
                nausea: None,
            },
            ExpectedScores {
                excitement: score_band(5.5, 7.0),
                intensity: Some(score_band(6.5, 8.0)),
                nausea: None,
            },
            ExpectedScores {
                excitement: score_band(9.0, 11.5),
                intensity: None,
                nausea: None,
            },
        ])
        .enumerate()
    {
        let scores = score(&archetype.features, &archetype.bands);
        println!(
            "  excitement target: {:.1}–{:.1}",
            expected.excitement.minimum, expected.excitement.maximum,
        );
        print_contributions(archetype.name, &scores);
        measured[index] = (scores.excitement, scores.intensity, scores.nausea);
        for (label, actual, band) in [
            ("excitement", scores.excitement, Some(expected.excitement)),
            ("intensity", scores.intensity, expected.intensity),
            ("nausea", scores.nausea, expected.nausea),
        ] {
            if let Some(band) = band
                && !(band.minimum..=band.maximum).contains(&actual)
            {
                band_failures.push(format!(
                    "{} {label} score {actual:.2} is outside {:.1}–{:.1}",
                    archetype.name, band.minimum, band.maximum
                ));
            }
        }
        for (total, contributions) in [
            (scores.excitement, &scores.breakdown.excitement),
            (scores.intensity, &scores.breakdown.intensity),
            (scores.nausea, &scores.breakdown.nausea),
        ] {
            assert!(
                (total - contributions.iter().map(|item| item.value).sum::<f64>()).abs() < 1e-12,
                "{} score contributions must sum to the rating",
                archetype.name
            );
        }
    }
    let [
        (family, _, _),
        (arrow, arrow_intensity, _),
        (hyper, hyper_intensity, _),
        (rmc, rmc_intensity, _),
        (accelerator, accelerator_intensity, _),
        (blitz, _, _),
    ] = measured;
    if !(rmc > hyper
        && blitz > hyper
        && hyper > accelerator
        && accelerator > arrow
        && arrow > family)
    {
        band_failures
            .push("archetype excitement order must put the airtime rich hybrid highest".to_owned());
    }
    if !(rmc_intensity > arrow_intensity
        && accelerator_intensity > arrow_intensity
        && arrow_intensity > hyper_intensity)
    {
        band_failures.push(
            "intensity order must put the high force Arrow below the RMC and accelerator"
                .to_owned(),
        );
    }
    assert!(
        band_failures.is_empty(),
        "selected SCORING weights miss the archetype target bands:\n{}",
        band_failures.join("\n")
    );
}

fn read_player_circuits() -> BTreeMap<String, Vec<String>> {
    let mut circuits = BTreeMap::new();
    let mut current_name: Option<String> = None;
    for line in include_str!("../../../../web/e2e/player/circuits.txt").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].to_owned();
            assert!(circuits.insert(name.clone(), Vec::new()).is_none());
            current_name = Some(name);
            continue;
        }
        let name = current_name
            .as_ref()
            .expect("circuit entries follow a header");
        circuits
            .get_mut(name)
            .expect("current circuit was inserted")
            .push(line.to_owned());
    }
    circuits
}

fn model_for_circuit(name: &str) -> CoasterModel {
    coaster_models()
        .into_iter()
        .find(|model| model.name == name)
        .unwrap_or_else(|| panic!("no coaster model for {name}"))
}

fn piece_id_for_name(name: &str) -> PieceId {
    catalog()
        .iter()
        .find(|piece| format!("{:?}", piece.id) == name)
        .unwrap_or_else(|| panic!("circuits.txt references unknown piece {name}"))
        .id
}

fn build_player_circuit(name: &str, entries: &[String]) -> (CoasterModel, super::Circuit) {
    let model = model_for_circuit(name);
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let mut construction = Construction::new(
        TrackState {
            x: 482,
            y: 498,
            z: 6,
            heading: Heading::North,
            ..TrackState::origin()
        },
        model.clone(),
    );
    construction
        .append(PieceId::Station, &world, &[])
        .expect("station begins player circuit");
    let mut chain_enabled = false;
    for entry in entries {
        match entry.as_str() {
            "+chain" => {
                chain_enabled = true;
                construction.set_chain(true);
            }
            "-chain" => {
                chain_enabled = false;
                construction.set_chain(false);
            }
            _ => construction
                .append(piece_id_for_name(entry), &world, &[])
                .unwrap_or_else(|error| panic!("{name} cannot place {entry}: {error:?}")),
        }
    }
    assert!(!chain_enabled, "{name} must end with chain disabled");
    construction.close().unwrap_or_else(|error| {
        panic!(
            "{name} circuit does not close from {:?}: {error:?}",
            construction.end()
        )
    });
    (
        model,
        construction
            .circuit()
            .expect("completed construction makes a circuit"),
    )
}

#[test]
fn calibration_table_recorder_uses_each_model_force_limits() {
    let circuit = demo_circuit();
    let scores = [hybrid_model(), wooden_twister_model()].map(|model| {
        let mut recorder = calibration_recorder(&circuit, &model);
        for _ in 0..400 {
            let reading = CarReading {
                distance: circuit.starts[10],
                speed: 10.0,
                g: GForces {
                    vertical: 4.2,
                    lateral: 0.0,
                    longitudinal: 0.0,
                },
                airtime: false,
                height_m: 0.0,
                inverted: false,
                roll: 0.0,
                piece_index: 10,
            };
            recorder.record(&[vec![reading; usize::from(model.train.cars)]]);
        }
        recorder.finish(false, false)
    });
    let excessive = |report: &super::TestRun| {
        report
            .breakdown
            .nausea
            .iter()
            .find(|item| item.term == Term::ExcessiveForces)
            .unwrap()
            .value
    };
    assert_eq!(excessive(&scores[0]), 0.0);
    assert!(excessive(&scores[1]) > 0.0);
}

fn calibration_recorder(
    circuit: &super::Circuit,
    model: &CoasterModel,
) -> ConstructionTestRunRecorder {
    ConstructionTestRunRecorder::for_circuit_with_train_and_limits(
        circuit,
        &model.train,
        model.comfortable_force_limits,
    )
}

fn run_lap(circuit: &super::Circuit, model: &CoasterModel, initial_speed: f64) -> super::TestRun {
    let mut train = Train::new(
        circuit,
        &model.train,
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        },
        0.0,
        initial_speed,
    );
    let mut recorder = calibration_recorder(circuit, model);
    let mut completed = false;
    for _ in 0..60_000 {
        train.step(circuit);
        recorder.record(&train.take_readings());
        if train.distance() >= circuit.length() {
            completed = true;
            break;
        }
        if !train.notes().is_empty() {
            break;
        }
    }
    assert!(
        completed,
        "{} did not complete: {:?}",
        model.name,
        train.notes()
    );
    recorder.finish(completed, !train.notes().is_empty())
}

#[test]
fn calibration_table_prints_demo_and_player_circuit_scores() {
    let demo = run_lap(&demo_circuit(), &hybrid_model(), 2.0);
    println!(
        "Circuit | Excitement | Intensity | Nausea | Peak +g | Peak -g | Peak lateral g | Peak speed m/s"
    );
    print_circuit("Demo", &demo);

    let circuits = read_player_circuits();
    assert_eq!(circuits.len(), 3, "calibration parses all player circuits");
    for model in coaster_models() {
        let name = model.name;
        let entries = circuits.get(name).expect("known player circuit");
        let (model, circuit) = build_player_circuit(name, entries);
        let report = run_lap(&circuit, &model, 2.0);
        print_circuit(name, &report);
    }
}

fn print_circuit(name: &str, report: &super::TestRun) {
    println!(
        "{name} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2}",
        report.excitement,
        report.intensity,
        report.nausea,
        report.statistics.max_positive_vertical_g,
        report.statistics.max_negative_vertical_g,
        report.statistics.max_lateral_g,
        report.statistics.max_speed,
    );
    print_contributions(
        name,
        &Scores {
            excitement: report.excitement,
            intensity: report.intensity,
            nausea: report.nausea,
            breakdown: report.breakdown.clone(),
        },
    );
}
