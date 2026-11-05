use super::*;
use theme_park_sim::physics::{Contribution, Location, ScoreBreakdown, Term};

/// Each catalog piece's centreline and up vector, in metres, in a frame where the piece enters at
/// the origin heading along +x, for drawing piece icons.
#[wasm_bindgen]
pub fn piece_shapes_json() -> String {
    const SAMPLES: usize = 32;
    let shapes: Vec<PieceShapeReport> = catalog()
        .iter()
        .map(|piece| {
            let origin = piece.entry.position();
            let angle = -(piece.entry.heading as i32 as f64) * std::f64::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let local = |vector: Vec3| {
                [
                    vector.x * cos - vector.y * sin,
                    vector.x * sin + vector.y * cos,
                    vector.z,
                ]
            };
            let points = (0..=SAMPLES)
                .map(|index| {
                    let sample = piece.sample(piece.length() * (index as f64 / SAMPLES as f64));
                    let position = local(sample.position - origin);
                    let up = local(sample.up);
                    [position[0], position[1], position[2], up[0], up[1], up[2]]
                        .map(|value| (value * 1000.0).round() / 1000.0)
                })
                .collect();
            PieceShapeReport {
                id: crate::construction::wire_pieceid(piece.id).into(),
                name: piece.name.into(),
                kind: crate::construction::wire_piecekind(piece.kind).into(),
                turn: (piece.exit.heading as i32 - piece.entry.heading as i32 + 4).rem_euclid(8)
                    - 4,
                entry_slope: crate::construction::wire_slope(piece.entry.slope).into(),
                exit_slope: crate::construction::wire_slope(piece.exit.slope).into(),
                entry_bank: crate::construction::wire_bank(piece.entry.bank).into(),
                exit_bank: crate::construction::wire_bank(piece.exit.bank).into(),
                points,
            }
        })
        .collect();
    serde_json::to_string(&shapes).expect("serialize piece shapes")
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn state_hash(&self) -> u64 {
        self.park.state_hash()
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PieceShapeReport {
    id: String,
    name: String,
    kind: String,
    turn: i32,
    entry_slope: String,
    exit_slope: String,
    entry_bank: String,
    exit_bank: String,
    points: Vec<[f64; 6]>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestingSample {
    pub distance: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub speed: f64,
    pub vertical_g: f64,
    pub lateral_g: f64,
    pub longitudinal_g: f64,
    pub airtime: bool,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProximityEventReport {
    pub kind: String,
    pub piece_index: usize,
    pub distance: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub strength: f64,
}

#[derive(serde::Serialize, ts_rs::TS)]
pub(crate) struct TestingRating {
    pub excitement: f64,
    pub intensity: f64,
    pub nausea: f64,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RideStatisticsReport {
    pub ejector_airtime_seconds: f64,
    pub floater_airtime_seconds: f64,
    pub airtime_moments: usize,
    pub strong_positive_g_seconds: f64,
    pub longest_strong_positive_g_seconds: f64,
    pub strong_lateral_g_seconds: f64,
    pub max_positive_vertical_g: f64,
    pub max_negative_vertical_g: f64,
    pub max_lateral_g: f64,
    pub max_forward_g: f64,
    pub max_braking_g: f64,
    pub max_speed: f64,
    pub average_speed: f64,
    pub duration_seconds: f64,
    pub track_length: f64,
    pub max_height: f64,
    pub biggest_drop: f64,
    pub steepest_drop_angle_degrees: f64,
    pub inversions: usize,
    pub inverted_seconds: f64,
    pub drops: usize,
    pub launches: usize,
    pub launch_peak_speed: f64,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestingReport {
    pub samples: Vec<TestingSample>,
    pub launches: Vec<LaunchReport>,
    pub notes: Vec<String>,
    pub proximity_events: Vec<ProximityEventReport>,
    pub rating: TestingRating,
    pub breakdown: ScoreBreakdownReport,
    pub statistics: RideStatisticsReport,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScoreBreakdownReport {
    pub excitement: Vec<ScoreContributionReport>,
    pub intensity: Vec<ScoreContributionReport>,
    pub nausea: Vec<ScoreContributionReport>,
}

impl From<&ScoreBreakdown> for ScoreBreakdownReport {
    fn from(breakdown: &ScoreBreakdown) -> Self {
        Self {
            excitement: breakdown.excitement.iter().map(Into::into).collect(),
            intensity: breakdown.intensity.iter().map(Into::into).collect(),
            nausea: breakdown.nausea.iter().map(Into::into).collect(),
        }
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScoreContributionReport {
    pub term: String,
    pub value: f64,
    pub measure: f64,
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub location: Option<ScoreLocationReport>,
}

impl From<&Contribution> for ScoreContributionReport {
    fn from(contribution: &Contribution) -> Self {
        Self {
            term: score_term_key(contribution.term).to_owned(),
            value: contribution.value,
            measure: contribution.measure,
            unit: contribution.term.unit().to_owned(),
            location: contribution.location.map(Into::into),
        }
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScoreLocationReport {
    pub piece_index: usize,
    pub piece_name: String,
    pub distance: f64,
    pub seconds: f64,
}

impl From<Location> for ScoreLocationReport {
    fn from(location: Location) -> Self {
        Self {
            piece_index: location.piece_index,
            piece_name: location.piece_name.to_owned(),
            distance: location.distance_m,
            seconds: location.seconds_from_start,
        }
    }
}

fn score_term_key(term: Term) -> &'static str {
    match term {
        Term::Airtime => "airtime",
        Term::PositiveForces => "positiveForces",
        Term::Laterals => "laterals",
        Term::LongitudinalForces => "longitudinalForces",
        Term::ExcessiveForces => "excessiveForces",
        Term::AirtimeMoments => "airtimeMoments",
        Term::Inversions => "inversions",
        Term::TopSpeed => "topSpeed",
        Term::BiggestDrop => "biggestDrop",
        Term::Pace => "pace",
        Term::Launch => "launch",
        Term::Variety => "variety",
        Term::Whip => "whip",
        Term::RollRate => "rollRate",
        Term::FastRolls => "fastRolls",
        Term::RideTime => "rideTime",
        Term::ShortRide => "shortRide",
        Term::Proximity => "proximity",
        Term::Theming => "theming",
        Term::DeadTime => "deadTime",
        Term::Envelope => "envelope",
        Term::Sickening => "sickening",
        Term::ExcitementFloor => "excitementFloor",
        Term::PeakForce => "peakForce",
        Term::SustainedForce => "sustainedForce",
        Term::Reversals => "reversals",
        Term::GreyOut => "greyOut",
        Term::TimeInverted => "timeInverted",
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchReport {
    pub piece_index: usize,
    pub exit_speed: f64,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockSectionReport {
    pub boundary_metres: f64,
    pub brake_start_metres: Option<f64>,
    pub brake_piece_indices: Vec<usize>,
    pub powered_hold_start_metres: Option<f64>,
    pub powered_hold_piece_indices: Vec<usize>,
    pub editable: bool,
    pub editable_index: Option<usize>,
    pub reason: String,
    pub track_kind: String,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockReport {
    pub sections: Vec<BlockSectionReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
    pub train_capacity: usize,
    pub capacity_search_complete: Option<bool>,
    pub theoretical_hourly_riders: Option<f64>,
    pub throughput_status: String,
    pub placement_status: String,
    pub placement_search_complete: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub placement_work_completed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub placement_best_train_capacity: Option<usize>,
    pub reasons: Vec<String>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn generated_types_are_current() {
        use ts_rs::TS;
        let destination =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/generated");
        let output = if std::env::var_os("UPDATE_REPORT_TYPES").is_some() {
            destination.clone()
        } else {
            let unique_run_id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let output = std::env::temp_dir().join(format!(
                "theme-park-report-types-{}-{unique_run_id}",
                std::process::id()
            ));
            std::fs::create_dir(&output).unwrap();
            output
        };
        // Process ids repeat (always, inside sandboxes), so a leftover folder from an earlier run would add stale files.
        if output != destination && output.exists() {
            std::fs::remove_dir_all(&output).unwrap();
        }
        crate::TrainLayoutReport::export_all_to(&output).unwrap();
        BlockReport::export_all_to(&output).unwrap();
        TestingReport::export_all_to(&output).unwrap();
        PieceShapeReport::export_all_to(&output).unwrap();
        crate::construction::ConstructionPieceWire::export_all_to(&output).unwrap();
        crate::construction::ConstructionElementWire::export_all_to(&output).unwrap();
        crate::construction::ConstructionTrackWire::export_all_to(&output).unwrap();
        crate::construction::ConstructionColorsWire::export_all_to(&output).unwrap();
        crate::construction::ConstructionForcePreviewWire::export_all_to(&output).unwrap();
        crate::construction::ConstructionCandidateForcePreviewWire::export_all_to(&output).unwrap();
        crate::construction_speed_match::ConstructionSpeedMatchWire::export_all_to(&output)
            .unwrap();
        crate::facilities::FacilityReport::export_all_to(&output).unwrap();
        crate::scenery::SceneryReport::export_all_to(&output).unwrap();
        crate::guest_report::GuestDetailReport::export_all_to(&output).unwrap();
        if output != destination {
            for entry in std::fs::read_dir(&output).unwrap() {
                let entry = entry.unwrap();
                let expected = destination.join(entry.file_name());
                assert_eq!(
                    std::fs::read(entry.path()).unwrap(),
                    std::fs::read(&expected).unwrap_or_else(|_| panic!(
                        "missing generated type {}",
                        expected.display()
                    )),
                    "stale generated type {}",
                    expected.display()
                );
            }
            std::fs::remove_dir_all(&output).unwrap();
        }
    }

    #[test]
    fn testing_report_uses_camel_case_and_preserves_strings() {
        let report = TestingReport {
            samples: vec![TestingSample {
                distance: 1.0,
                x: 2.0,
                y: 3.0,
                z: 4.0,
                speed: 5.0,
                vertical_g: 6.0,
                lateral_g: 7.0,
                longitudinal_g: 8.0,
                airtime: true,
            }],
            launches: vec![LaunchReport {
                piece_index: 4,
                exit_speed: 42.0,
            }],
            notes: vec!["quoted \"note\"\n".to_owned()],
            proximity_events: vec![],
            rating: TestingRating {
                excitement: 1.0,
                intensity: 2.0,
                nausea: 3.0,
            },
            breakdown: ScoreBreakdownReport {
                excitement: vec![ScoreContributionReport {
                    term: "airtime".into(),
                    value: 1.25,
                    measure: 7.6,
                    unit: "s".into(),
                    location: Some(ScoreLocationReport {
                        piece_index: 6,
                        piece_name: "Loop".into(),
                        distance: 200.0,
                        seconds: 8.0,
                    }),
                }],
                intensity: vec![],
                nausea: vec![],
            },
            statistics: RideStatisticsReport {
                ejector_airtime_seconds: 0.0,
                floater_airtime_seconds: 0.0,
                airtime_moments: 0,
                strong_positive_g_seconds: 0.0,
                longest_strong_positive_g_seconds: 0.0,
                strong_lateral_g_seconds: 0.0,
                max_positive_vertical_g: 0.0,
                max_negative_vertical_g: 0.0,
                max_lateral_g: 0.0,
                max_forward_g: 0.0,
                max_braking_g: 0.0,
                max_speed: 0.0,
                average_speed: 0.0,
                duration_seconds: 0.0,
                track_length: 0.0,
                max_height: 0.0,
                biggest_drop: 0.0,
                steepest_drop_angle_degrees: 0.0,
                inversions: 0,
                inverted_seconds: 0.0,
                drops: 0,
                launches: 0,
                launch_peak_speed: 0.0,
            },
        };
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["samples"][0]["verticalG"], 6.0);
        assert_eq!(value["notes"][0], "quoted \"note\"\n");
        assert_eq!(value["launches"][0]["pieceIndex"], 4);
        assert_eq!(value["launches"][0]["exitSpeed"], 42.0);
        assert_eq!(value["proximityEvents"], serde_json::json!([]));
        assert_eq!(value["rating"]["intensity"], 2.0);
        assert_eq!(value["breakdown"]["excitement"][0]["term"], "airtime");
        assert_eq!(value["breakdown"]["excitement"][0]["measure"], 7.6);
        assert_eq!(value["breakdown"]["excitement"][0]["unit"], "s");
        assert_eq!(
            value["breakdown"]["excitement"][0]["location"]["pieceIndex"],
            6
        );
        assert_eq!(
            value["breakdown"]["excitement"][0]["location"]["pieceName"],
            "Loop"
        );
        assert_eq!(value["statistics"]["airtimeMoments"], 0);
    }

    #[test]
    fn every_scoring_term_has_a_stable_wire_key() {
        use theme_park_sim::physics::Term;

        let terms = [
            (Term::Airtime, "airtime"),
            (Term::PositiveForces, "positiveForces"),
            (Term::Laterals, "laterals"),
            (Term::LongitudinalForces, "longitudinalForces"),
            (Term::ExcessiveForces, "excessiveForces"),
            (Term::AirtimeMoments, "airtimeMoments"),
            (Term::Inversions, "inversions"),
            (Term::TopSpeed, "topSpeed"),
            (Term::BiggestDrop, "biggestDrop"),
            (Term::Pace, "pace"),
            (Term::Launch, "launch"),
            (Term::Variety, "variety"),
            (Term::Whip, "whip"),
            (Term::RollRate, "rollRate"),
            (Term::FastRolls, "fastRolls"),
            (Term::RideTime, "rideTime"),
            (Term::ShortRide, "shortRide"),
            (Term::Proximity, "proximity"),
            (Term::Theming, "theming"),
            (Term::DeadTime, "deadTime"),
            (Term::Envelope, "envelope"),
            (Term::Sickening, "sickening"),
            (Term::PeakForce, "peakForce"),
            (Term::SustainedForce, "sustainedForce"),
            (Term::Reversals, "reversals"),
            (Term::GreyOut, "greyOut"),
            (Term::TimeInverted, "timeInverted"),
        ];
        for (term, expected) in terms {
            let contribution = theme_park_sim::physics::Contribution {
                term,
                value: 0.0,
                measure: 0.0,
                location: None,
            };
            assert_eq!(ScoreContributionReport::from(&contribution).term, expected);
        }
    }
}
