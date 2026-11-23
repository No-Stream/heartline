use super::*;
use serde::Serialize;
use theme_park_sim::physics::{
    Circuit, ForcePeakValues, PhysicsConfig, PieceForcePeaks, PreviewForceSample, PreviewStatus,
    TestRun,
};
use theme_park_sim::track::rated_windows::{RatedWindowSource, measured_rated_window};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, LoopTopFeel, ShapedConstructionError, SolveRefusal,
    TurnSizeHint, default_hill_strength, default_loop_peak_g, default_raven_turn_peak_g,
    default_zero_g_roll_peak_g,
};
use theme_park_sim::track::{
    Bank, CoasterModel, DesignForceTargets, ENVELOPE_DISPLAY_LIMITS, ForceRating, ForceRatingAxis,
    Hand, Heartlining, HillFeel, HillTwist, PieceFamily, envelope_rating_bands,
};
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionPieceWire {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub heartlined: Option<bool>,
    pub heading: &'static str,
    pub slope: &'static str,
    pub bank: &'static str,
    pub refusal: String,
    pub would_tunnel: bool,
    pub hill_estimate: Option<ConstructionHillEstimateWire>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionHillEstimateWire {
    pub entry_speed: Option<f64>,
    pub source: &'static str,
    pub rated_speed: Option<ConstructionRatedSpeedWire>,
    pub minimum_vertical_g: Option<f64>,
    pub maximum_vertical_g: Option<f64>,
    pub maximum_absolute_lateral_g: Option<f64>,
    pub airtime_seconds: Option<f64>,
    pub limits: ConstructionForceLimitsWire,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionHillOptionWire {
    id: &'static str,
    feel: &'static str,
    height: u8,
    span: u16,
    twist: &'static str,
    rated_speed: Option<ConstructionRatedSpeedWire>,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionSpeedTurnWire {
    id: &'static str,
    #[ts(type = "\"left\" | \"right\"")]
    hand: &'static str,
    design_speed: f64,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionShapeSpecWire {
    #[ts(type = "\"hill\" | \"turn\" | \"loop\" | \"zeroGRoll\" | \"ravenTurn\"")]
    pub kind: &'static str,
    pub design_speed_mps: f64,
    pub cars: u8,
    pub target_g: Option<f64>,
    pub delivered_g: Option<f64>,
    pub vertical_g: Option<f64>,
    pub lateral_g: Option<f64>,
    pub height_m: Option<f64>,
    pub span_m: Option<f64>,
    pub radius_m: Option<f64>,
    pub bank_degrees: Option<f64>,
    #[ts(type = "\"floaty\" | \"neutral\" | \"pressedIn\" | null")]
    pub top_feel: Option<&'static str>,
    pub top_g: Option<f64>,
    pub peak_roll_rate_deg_s: Option<f64>,
    pub steel_structure: bool,
    pub geometry_digest: String,
}

#[derive(Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConstructionShapeRefusalWire {
    Stall {
        entry_speed_mps: f64,
        height_m: f64,
    },
    InfeasibleTarget {
        target_g: f64,
        available_headroom_g: f64,
    },
    BelowMinimumSpeed {
        entry_speed_mps: f64,
        minimum_speed_mps: f64,
    },
    UnsupportedScope,
    PreviewTimedOut,
    InvalidContext,
    UnsupportedConnector,
    UnsupportedSizeOverride,
    InvalidGeometry,
    TopFeelOutOfReach {
        target_top_g: f64,
        average_top_g: f64,
        lowest_car_top_g: f64,
        highest_car_top_g: f64,
    },
    GeometryDigestMismatch {
        expected: u64,
        actual: u64,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionShapedElementWire {
    pub selection: String,
    #[ts(type = "\"hill\" | \"turn\" | \"loop\" | \"zeroGRoll\" | \"ravenTurn\"")]
    pub kind: &'static str,
    pub feel: Option<&'static str>,
    #[ts(type = "\"left\" | \"right\" | null")]
    pub hand: Option<&'static str>,
    pub strength_g: Option<f64>,
    pub fixed_size: Option<f64>,
    pub model_name: &'static str,
    pub exceeds_model_feel: bool,
    pub spec: Option<ConstructionShapeSpecWire>,
    pub refusal: Option<ConstructionShapeRefusalWire>,
    pub piece_shape: Option<crate::reports::PieceShapeReport>,
}

fn parse_loop_top_feel(feel: &str) -> Result<LoopTopFeel, String> {
    match feel {
        "floaty" => Ok(LoopTopFeel::Floaty),
        "neutral" => Ok(LoopTopFeel::Neutral),
        "pressedIn" => Ok(LoopTopFeel::PressedIn),
        _ => Err(format!("Unknown shaped loop top feel {feel:?}")),
    }
}

fn loop_top_feel_wire(feel: LoopTopFeel) -> &'static str {
    match feel {
        LoopTopFeel::Floaty => "floaty",
        LoopTopFeel::Neutral => "neutral",
        LoopTopFeel::PressedIn => "pressedIn",
    }
}

fn parse_peak_g(value: &str, default_peak_g: f64, label: &str) -> Result<f64, String> {
    if value == "default" {
        return Ok(default_peak_g);
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("Invalid shaped {label} peak g {value:?}"))?;
    if !parsed.is_finite() {
        return Err(format!("Invalid shaped {label} peak g {value:?}"));
    }
    Ok(parsed)
}

fn parse_shape_selection(selection: &str, model: &CoasterModel) -> Result<ElementIntent, String> {
    let parts: Vec<_> = selection.split(':').collect();
    match parts.as_slice() {
        ["shape", "hill", feel, strength, size] => {
            let feel = match *feel {
                "floater" => HillFeel::Floater,
                "strongFloater" => HillFeel::StrongFloater,
                "ejector" => HillFeel::Ejector,
                _ => return Err(format!("Unknown shaped hill feel {feel:?}")),
            };
            let strength_g = if *strength == "default" {
                default_hill_strength(feel, model)
            } else {
                let value = strength
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid shaped hill strength {strength:?}"))?;
                if !value.is_finite() {
                    return Err(format!("Invalid shaped hill strength {strength:?}"));
                }
                value
            };
            let size = parse_shape_size(size, "hill height")?
                .map_or(HillSizeHint::Auto, HillSizeHint::Height);
            Ok(ElementIntent::Hill {
                feel,
                strength_g,
                size,
            })
        }
        ["shape", "turn", hand, "fullyBanked", size] => {
            let hand = parse_shape_hand(hand)?;
            let size = parse_shape_size(size, "turn radius")?
                .map_or(TurnSizeHint::Auto, TurnSizeHint::Radius);
            Ok(ElementIntent::Turn { hand, size })
        }
        ["shape", "turn", _, feel, _] => Err(format!("Unsupported shaped turn feel {feel:?}")),
        ["shape", "loop", hand, peak_g, top_feel] => {
            let hand = parse_shape_hand(hand)?;
            let peak_g = parse_peak_g(peak_g, default_loop_peak_g(model), "loop")?;
            let top_feel = parse_loop_top_feel(top_feel)?;
            Ok(ElementIntent::Loop {
                hand,
                peak_g,
                top_feel,
            })
        }
        ["shape", "zeroGRoll", hand, peak_g] => {
            let hand = parse_shape_hand(hand)?;
            let peak_g = parse_peak_g(peak_g, default_zero_g_roll_peak_g(model), "zero-g roll")?;
            Ok(ElementIntent::ZeroGRoll { hand, peak_g })
        }
        ["shape", "ravenTurn", hand, peak_g, top_feel] => {
            let hand = parse_shape_hand(hand)?;
            let peak_g = parse_peak_g(peak_g, default_raven_turn_peak_g(model), "Raven turn")?;
            let top_feel = parse_loop_top_feel(top_feel)?;
            Ok(ElementIntent::RavenTurn {
                hand,
                peak_g,
                top_feel,
            })
        }
        _ => Err(format!("Invalid shaped element selection {selection:?}")),
    }
}

fn parse_shape_size(size: &str, label: &str) -> Result<Option<f64>, String> {
    if size == "auto" {
        return Ok(None);
    }
    let Some(value) = size.strip_prefix("fixed-") else {
        return Err(format!("Invalid shaped element {label} {size:?}"));
    };
    let value = value
        .parse::<f64>()
        .map_err(|_| format!("Invalid shaped element {label} {size:?}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("Invalid shaped element {label} {size:?}"));
    }
    Ok(Some(value))
}

fn parse_shape_hand(hand: &str) -> Result<Hand, String> {
    match hand {
        "left" => Ok(Hand::Left),
        "right" => Ok(Hand::Right),
        _ => Err(format!("Unknown shaped turn hand {hand:?}")),
    }
}

fn shape_selection_id(intent: &ElementIntent) -> String {
    match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => {
            let feel = match feel {
                HillFeel::Floater => "floater",
                HillFeel::StrongFloater => "strongFloater",
                HillFeel::Ejector => "ejector",
                other => panic!("unsupported shaped hill feel {other:?}"),
            };
            let strength = strength_g.to_string();
            let size = match size {
                HillSizeHint::Auto => "auto".into(),
                HillSizeHint::Height(height) => format!("fixed-{height}"),
            };
            format!("shape:hill:{feel}:{strength}:{size}")
        }
        ElementIntent::Turn { hand, size } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let size = match size {
                TurnSizeHint::Auto => "auto".into(),
                TurnSizeHint::Radius(radius) => format!("fixed-{radius}"),
            };
            format!("shape:turn:{hand}:fullyBanked:{size}")
        }
        ElementIntent::Loop {
            hand,
            peak_g,
            top_feel,
        } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let top_feel = loop_top_feel_wire(*top_feel);
            format!("shape:loop:{hand}:{peak_g}:{top_feel}")
        }
        ElementIntent::ZeroGRoll { hand, peak_g } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            format!("shape:zeroGRoll:{hand}:{peak_g}")
        }
        ElementIntent::RavenTurn {
            hand,
            peak_g,
            top_feel,
        } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let top_feel = loop_top_feel_wire(*top_feel);
            format!("shape:ravenTurn:{hand}:{peak_g}:{top_feel}")
        }
    }
}

fn shape_size_hint(intent: &ElementIntent) -> Option<f64> {
    match intent {
        ElementIntent::Hill { size, .. } => match size {
            HillSizeHint::Auto => None,
            HillSizeHint::Height(height) => Some(*height),
        },
        ElementIntent::Turn { size, .. } => match size {
            TurnSizeHint::Auto => None,
            TurnSizeHint::Radius(radius) => Some(*radius),
        },
        ElementIntent::Loop { .. }
        | ElementIntent::ZeroGRoll { .. }
        | ElementIntent::RavenTurn { .. } => None,
    }
}

fn shape_feel(intent: &ElementIntent) -> Option<&'static str> {
    match intent {
        ElementIntent::Hill { feel, .. } => Some(match feel {
            HillFeel::Floater => "floater",
            HillFeel::StrongFloater => "strongFloater",
            HillFeel::Ejector => "ejector",
            other => panic!("unsupported shaped hill feel {other:?}"),
        }),
        ElementIntent::Turn { .. } => Some("fullyBanked"),
        ElementIntent::Loop { top_feel, .. } | ElementIntent::RavenTurn { top_feel, .. } => {
            Some(loop_top_feel_wire(*top_feel))
        }
        ElementIntent::ZeroGRoll { .. } => None,
    }
}

fn shape_strength(intent: &ElementIntent) -> Option<f64> {
    match intent {
        ElementIntent::Hill { strength_g, .. } => Some(*strength_g),
        ElementIntent::Turn { .. } => None,
        ElementIntent::Loop { peak_g, .. }
        | ElementIntent::ZeroGRoll { peak_g, .. }
        | ElementIntent::RavenTurn { peak_g, .. } => Some(*peak_g),
    }
}

fn shape_hand(intent: &ElementIntent) -> Option<&'static str> {
    match intent {
        ElementIntent::Hill { .. } => None,
        ElementIntent::Turn { hand, .. }
        | ElementIntent::Loop { hand, .. }
        | ElementIntent::ZeroGRoll { hand, .. }
        | ElementIntent::RavenTurn { hand, .. } => Some(match hand {
            Hand::Left => "left",
            Hand::Right => "right",
        }),
    }
}

fn shaped_spec_wire(spec: &ElementSpec) -> ConstructionShapeSpecWire {
    let geometry_digest = format!("{:016x}", spec.resolved_geometry_digest());
    match spec {
        ElementSpec::Hill {
            design_speed_mps,
            cars,
            target_g,
            delivered_g,
            height_m,
            span_m,
            ..
        } => ConstructionShapeSpecWire {
            kind: "hill",
            design_speed_mps: *design_speed_mps,
            cars: *cars,
            target_g: Some(*target_g),
            delivered_g: Some(*delivered_g),
            vertical_g: None,
            lateral_g: None,
            height_m: Some(*height_m),
            span_m: Some(*span_m),
            radius_m: None,
            bank_degrees: None,
            top_feel: None,
            top_g: None,
            peak_roll_rate_deg_s: None,
            steel_structure: false,
            geometry_digest,
        },
        ElementSpec::Turn {
            design_speed_mps,
            cars,
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
            ..
        } => ConstructionShapeSpecWire {
            kind: "turn",
            design_speed_mps: *design_speed_mps,
            cars: *cars,
            target_g: None,
            delivered_g: None,
            vertical_g: Some(*vertical_g),
            lateral_g: Some(*lateral_g),
            height_m: None,
            span_m: None,
            radius_m: Some(*radius_m),
            bank_degrees: Some(*bank_degrees),
            top_feel: None,
            top_g: None,
            peak_roll_rate_deg_s: None,
            steel_structure: false,
            geometry_digest,
        },
        ElementSpec::Loop {
            design_speed_mps,
            cars,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            height_m,
            span_m,
            steel_structure,
            ..
        } => ConstructionShapeSpecWire {
            kind: "loop",
            design_speed_mps: *design_speed_mps,
            cars: *cars,
            target_g: Some(*target_peak_g),
            delivered_g: Some(*delivered_peak_g),
            vertical_g: Some(*delivered_peak_g),
            lateral_g: None,
            height_m: Some(*height_m),
            span_m: Some(*span_m),
            radius_m: None,
            bank_degrees: None,
            top_feel: Some(loop_top_feel_wire(*top_feel)),
            top_g: Some(*delivered_top_g),
            peak_roll_rate_deg_s: None,
            steel_structure: *steel_structure,
            geometry_digest,
        },
        ElementSpec::ZeroGRoll {
            design_speed_mps,
            cars,
            target_peak_g,
            delivered_peak_g,
            delivered_crest_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            ..
        } => ConstructionShapeSpecWire {
            kind: "zeroGRoll",
            design_speed_mps: *design_speed_mps,
            cars: *cars,
            target_g: Some(*target_peak_g),
            delivered_g: Some(*delivered_peak_g),
            vertical_g: Some(*delivered_peak_g),
            lateral_g: None,
            height_m: Some(*height_m),
            span_m: Some(*span_m),
            radius_m: None,
            bank_degrees: None,
            top_feel: None,
            top_g: Some(*delivered_crest_g),
            peak_roll_rate_deg_s: Some(*peak_roll_rate_deg_s),
            steel_structure: false,
            geometry_digest,
        },
        ElementSpec::RavenTurn {
            design_speed_mps,
            cars,
            top_feel,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            ..
        } => ConstructionShapeSpecWire {
            kind: "ravenTurn",
            design_speed_mps: *design_speed_mps,
            cars: *cars,
            target_g: Some(*target_peak_g),
            delivered_g: Some(*delivered_peak_g),
            vertical_g: Some(*delivered_peak_g),
            lateral_g: None,
            height_m: Some(*height_m),
            span_m: Some(*span_m),
            radius_m: None,
            bank_degrees: None,
            top_feel: Some(loop_top_feel_wire(*top_feel)),
            top_g: Some(*delivered_top_g),
            peak_roll_rate_deg_s: Some(*peak_roll_rate_deg_s),
            steel_structure: false,
            geometry_digest,
        },
    }
}

fn solve_refusal_wire(refusal: SolveRefusal) -> ConstructionShapeRefusalWire {
    match refusal {
        SolveRefusal::Stall {
            entry_speed_mps,
            height_m,
        } => ConstructionShapeRefusalWire::Stall {
            entry_speed_mps,
            height_m,
        },
        SolveRefusal::InfeasibleTarget {
            target_g,
            available_headroom_g,
        } => ConstructionShapeRefusalWire::InfeasibleTarget {
            target_g,
            available_headroom_g,
        },
        SolveRefusal::BelowMinimumSpeed {
            entry_speed_mps,
            minimum_speed_mps,
        } => ConstructionShapeRefusalWire::BelowMinimumSpeed {
            entry_speed_mps,
            minimum_speed_mps,
        },
        SolveRefusal::UnsupportedScope => ConstructionShapeRefusalWire::UnsupportedScope,
        SolveRefusal::PreviewTimedOut => ConstructionShapeRefusalWire::PreviewTimedOut,
        SolveRefusal::InvalidContext => ConstructionShapeRefusalWire::InvalidContext,
        SolveRefusal::UnsupportedConnector => ConstructionShapeRefusalWire::UnsupportedConnector,
        SolveRefusal::UnsupportedSizeOverride => {
            ConstructionShapeRefusalWire::UnsupportedSizeOverride
        }
        SolveRefusal::InvalidGeometry => ConstructionShapeRefusalWire::InvalidGeometry,
        SolveRefusal::TopFeelOutOfReach {
            target_top_g,
            average_top_g,
            lowest_car_top_g,
            highest_car_top_g,
        } => ConstructionShapeRefusalWire::TopFeelOutOfReach {
            target_top_g,
            average_top_g,
            lowest_car_top_g,
            highest_car_top_g,
        },
        SolveRefusal::GeometryDigestMismatch { expected, actual } => {
            ConstructionShapeRefusalWire::GeometryDigestMismatch { expected, actual }
        }
    }
}

fn solve_refusal_message(refusal: SolveRefusal) -> String {
    match refusal {
        SolveRefusal::Stall { height_m, .. } => format!("Element stalls at {height_m:.1} m."),
        SolveRefusal::InfeasibleTarget { target_g, .. } => {
            format!("The force target {target_g:.2} g is not feasible for this model.")
        }
        SolveRefusal::BelowMinimumSpeed {
            entry_speed_mps,
            minimum_speed_mps,
        } => format!(
            "Entry speed {entry_speed_mps:.1} m/s is below the shaped-element minimum of {minimum_speed_mps:.1} m/s."
        ),
        SolveRefusal::UnsupportedScope => {
            "Entry speed or train length is outside this model's shaped-element range.".into()
        }
        SolveRefusal::PreviewTimedOut => "The train preview timed out.".into(),
        SolveRefusal::InvalidContext => "The train cannot reach this element entry.".into(),
        SolveRefusal::UnsupportedConnector => {
            "Shaped elements need a level, straight entry connector.".into()
        }
        SolveRefusal::UnsupportedSizeOverride => {
            "This fixed size override is not supported for shaped elements.".into()
        }
        SolveRefusal::InvalidGeometry => {
            "The shaped element geometry cannot be integrated for this entry speed.".into()
        }
        SolveRefusal::TopFeelOutOfReach {
            target_top_g,
            average_top_g,
            lowest_car_top_g,
            highest_car_top_g,
        } => format!(
            "The top feel is out of reach: the train averages {average_top_g:.2} g at the top (cars {lowest_car_top_g:.2} to {highest_car_top_g:.2} g) and cannot get within 0.35 g of {target_top_g:.2} g without breaking a force limit at this speed."
        ),
        SolveRefusal::GeometryDigestMismatch { .. } => {
            "The saved shaped geometry does not match its resolved parameters.".into()
        }
    }
}

fn shaped_construction_error_message(error: ShapedConstructionError) -> String {
    match error {
        ShapedConstructionError::Solve(refusal) => solve_refusal_message(refusal),
        ShapedConstructionError::Construction(error) => construction_error(error),
        ShapedConstructionError::InvalidResolvedSpec => {
            "The saved shaped element does not match its resolved geometry.".into()
        }
        ShapedConstructionError::GeometryDigestMismatch(mismatch) => mismatch.to_string(),
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionElementWire {
    pub id: String,
    pub name: &'static str,
    pub piece_id: &'static str,
    pub heartlined: Option<bool>,
    pub slope: &'static str,
    pub kind: &'static str,
    pub would_tunnel: bool,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionTrackWire {
    id: usize,
    label: &'static str,
    name: &'static str,
    group: &'static str,
    slope: &'static str,
    kind: &'static str,
    launch_target_speed: Option<f64>,
    launch_minimum_speed: Option<f64>,
    launch_maximum_speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    piece_shape: Option<crate::reports::PieceShapeReport>,
}

/// The wire form of a finished test report, shared by the builder and the ride info window.
pub(crate) fn testing_report_json(report: &TestRun, circuit: &Circuit) -> String {
    serde_json::to_string(&testing_report(report, circuit)).expect("serialize testing report")
}

pub(crate) fn testing_report(report: &TestRun, circuit: &Circuit) -> crate::reports::TestingReport {
    let mut peaks_by_piece: std::collections::BTreeMap<_, _> = report
        .piece_force_peaks()
        .into_iter()
        .map(|peaks| (peaks.piece_index, peaks))
        .collect();
    let piece_force_peaks = (0..circuit.pieces().len())
        .map(|piece_index| {
            let peaks = peaks_by_piece.remove(&piece_index);
            crate::reports::PieceForcePeaksReport {
                piece_index,
                front_car: peaks.and_then(|peaks| peaks.front_car.map(Into::into)),
                worst_car: peaks.and_then(|peaks| peaks.worst_car.map(Into::into)),
            }
        })
        .collect();
    let samples = report
        .samples
        .iter()
        .map(|sample| {
            let position = circuit.sample(sample.distance).position / 4.0;
            crate::reports::TestingSample {
                distance: sample.distance,
                x: position.x,
                y: position.y,
                z: position.z,
                speed: sample.speed,
                vertical_g: sample.vertical_g,
                lateral_g: sample.lateral_g,
                longitudinal_g: sample.longitudinal_g,
                airtime: sample.airtime,
                force_limit_exceeded: sample.force_limit_exceeded,
            }
        })
        .collect();
    let proximity_events = report
        .proximity_events
        .iter()
        .map(|event| {
            let kind = match event.kind {
                ProximityKind::NearMiss => "nearMiss",
                ProximityKind::Headchopper => "headchopper",
                ProximityKind::Dueling => "dueling",
                ProximityKind::Tunnel => "tunnel",
                ProximityKind::TerrainHugging => "terrainHugging",
            };
            crate::reports::ProximityEventReport {
                kind: kind.to_owned(),
                piece_index: event.piece_index,
                distance: event.distance,
                x: event.position.x / 4.0,
                y: event.position.y / 4.0,
                z: event.position.z / 4.0,
                strength: event.strength,
            }
        })
        .collect();
    crate::reports::TestingReport {
        samples,
        piece_force_peaks,
        launches: report_launches(circuit, report),
        notes: report.notes.clone(),
        proximity_events,
        rating: crate::reports::TestingRating {
            excitement: report.excitement,
            intensity: report.intensity,
            nausea: report.nausea,
        },
        breakdown: (&report.breakdown).into(),
        statistics: {
            let stats = report.statistics;
            crate::reports::RideStatisticsReport {
                ejector_airtime_seconds: stats.ejector_airtime_seconds,
                floater_airtime_seconds: stats.floater_airtime_seconds,
                airtime_moments: stats.airtime_moments,
                strong_positive_g_seconds: stats.strong_positive_g_seconds,
                longest_strong_positive_g_seconds: stats.longest_strong_positive_g_seconds,
                strong_lateral_g_seconds: stats.strong_lateral_g_seconds,
                max_positive_vertical_g: stats.max_positive_vertical_g,
                max_negative_vertical_g: stats.max_negative_vertical_g,
                max_lateral_g: stats.max_lateral_g,
                max_forward_g: stats.max_forward_g,
                max_braking_g: stats.max_braking_g,
                max_speed: stats.max_speed,
                average_speed: stats.average_speed,
                duration_seconds: stats.duration_seconds,
                track_length: stats.track_length,
                max_height: stats.max_height,
                biggest_drop: stats.biggest_drop,
                steepest_drop_angle_degrees: stats.steepest_drop_angle_degrees,
                inversions: stats.inversions,
                inverted_seconds: stats.inverted_seconds,
                drops: stats.drops,
                launches: stats.launches,
                launch_peak_speed: stats.launch_peak_speed,
            }
        },
    }
}

fn report_launches(circuit: &Circuit, report: &TestRun) -> Vec<crate::reports::LaunchReport> {
    let mut start_distance = 0.0;
    circuit
        .pieces()
        .iter()
        .enumerate()
        .filter_map(|(piece_index, piece)| {
            let end_distance = start_distance + piece.length();
            let launch = (piece.kind == PieceKind::Launch).then(|| {
                let exit_distance = end_distance.rem_euclid(circuit.length());
                let sample = report
                    .samples
                    .iter()
                    .min_by(|first, second| {
                        let distance_to_exit = |distance: f64| {
                            let difference = (distance - exit_distance).abs();
                            difference.min(circuit.length() - difference)
                        };
                        distance_to_exit(first.distance)
                            .total_cmp(&distance_to_exit(second.distance))
                    })
                    .expect("a completed test report has samples");
                crate::reports::LaunchReport {
                    piece_index,
                    exit_speed: sample.speed,
                }
            });
            start_distance = end_distance;
            launch
        })
        .collect()
}

#[derive(Serialize, TS)]
pub struct ConstructionColorsWire {
    track: &'static str,
    supports: &'static str,
    trains: &'static str,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionDesignTargetsWire {
    pub model_name: String,
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub maximum_lateral_g: f64,
    pub maximum_longitudinal_g: f64,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionForceLimitsWire {
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub rating_bands: ConstructionForceRatingBandsWire,
    pub maximum_lateral_g: f64,
    pub maximum_longitudinal_g: f64,
    pub design_targets: ConstructionDesignTargetsWire,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionForceRatingBandWire {
    #[ts(
        type = "\"weak\" | \"good\" | \"great\" | \"fine\" | \"strong\" | \"too much\" | \"dangerous\""
    )]
    pub rating: String,
    pub up_to: Option<f64>,
    pub inclusive: bool,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionForceRatingBandsWire {
    pub airtime: Vec<ConstructionForceRatingBandWire>,
    pub vertical: Vec<ConstructionForceRatingBandWire>,
    pub lateral: Vec<ConstructionForceRatingBandWire>,
    pub longitudinal: Vec<ConstructionForceRatingBandWire>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionLastPlacedForceWire {
    pub maximum_speed: f64,
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub maximum_absolute_lateral_g: f64,
    pub maximum_absolute_longitudinal_g: f64,
    pub minimum_vertical_limit_exceeded: bool,
    pub maximum_vertical_limit_exceeded: bool,
    pub lateral_limit_exceeded: bool,
    pub longitudinal_limit_exceeded: bool,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionLaunchEstimateWire {
    pub piece_index: usize,
    pub maximum_speed: f64,
    pub peak_longitudinal_g: Option<f64>,
}

/// One ranked way to finish the circuit, as the builder shows it.
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionFinishOptionWire {
    pub rank: usize,
    pub pieces: Vec<String>,
    pub arrival_speed_mps: f64,
    pub brake_energy_kj: f64,
    pub minimum_force_headroom_g: f64,
    pub minimum_rated_margin_mps: Option<f64>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionFinishOptionsWire {
    pub previewed_index: Option<usize>,
    pub options: Vec<ConstructionFinishOptionWire>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionForcePreviewWire {
    pub samples: Vec<crate::reports::TestingSample>,
    pub force_envelope_samples: Vec<crate::reports::TestingSample>,
    pub piece_force_peaks: Vec<crate::reports::PieceForcePeaksReport>,
    pub completed: bool,
    pub status: &'static str,
    pub last_placed: Option<ConstructionLastPlacedForceWire>,
    pub launches: Vec<ConstructionLaunchEstimateWire>,
    pub limits: ConstructionForceLimitsWire,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionRatedSpeedWire {
    pub minimum_entry: f64,
    pub maximum_entry: f64,
    pub feel_minimum_entry: Option<f64>,
    pub feel_maximum_entry: Option<f64>,
    #[ts(type = "\"designTarget\" | \"airtimeFeel\" | \"designCap\" | null")]
    pub feel_maximum_source: Option<&'static str>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionCandidateForcePreviewWire {
    pub status: &'static str,
    pub outer_bank: bool,
    pub entry_speed: Option<f64>,
    pub peaks: Option<ConstructionLastPlacedForceWire>,
    pub samples: Vec<crate::reports::TestingSample>,
    pub force_envelope_samples: Vec<crate::reports::TestingSample>,
    pub piece_force_peaks: Vec<crate::reports::PieceForcePeaksReport>,
    pub limits: ConstructionForceLimitsWire,
    pub rated_speed: Option<ConstructionRatedSpeedWire>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionCandidateGeometryWire {
    pub end_tile_x: f64,
    pub end_tile_y: f64,
    pub end_height: f64,
    pub length: f64,
}

fn hill_feel_wire(feel: HillFeel) -> &'static str {
    match feel {
        HillFeel::Ejector => "ejector",
        HillFeel::Floater => "floater",
        HillFeel::StrongFloater => "strongFloater",
        HillFeel::Speed => "speed",
        HillFeel::ZeroGRoll => "zeroGRoll",
        HillFeel::Stall => "stall",
    }
}

fn hill_twist_wire(twist: Option<HillTwist>) -> &'static str {
    match twist {
        None => "none",
        Some(HillTwist::OuterBankLeft) => "outerLeft",
        Some(HillTwist::OuterBankRight) => "outerRight",
        Some(HillTwist::LeftRoll) => "left",
        Some(HillTwist::RightRoll) => "right",
    }
}

fn rated_speed_wire(model: &CoasterModel, piece: &Piece) -> Option<ConstructionRatedSpeedWire> {
    let _design_speeds = piece.design_speeds?;
    let window = measured_rated_window(model, piece)?;
    let feel_maximum_source = window.feel_maximum_source.and_then(|source| match source {
        RatedWindowSource::DesignTarget => Some("designTarget"),
        RatedWindowSource::AirtimeFeel => Some("airtimeFeel"),
        RatedWindowSource::DesignCap => Some("designCap"),
        _ => None,
    });
    Some(ConstructionRatedSpeedWire {
        minimum_entry: window.minimum_entry,
        maximum_entry: window.maximum_entry,
        feel_minimum_entry: window.feel_minimum_entry,
        feel_maximum_entry: window.feel_maximum_entry,
        feel_maximum_source,
    })
}

pub(crate) fn force_limits_wire(
    design_targets: DesignForceTargets,
    model_name: &str,
) -> ConstructionForceLimitsWire {
    let bands = |axis| {
        envelope_rating_bands(axis, model_name)
            .into_iter()
            .map(|band| ConstructionForceRatingBandWire {
                rating: match band.rating {
                    ForceRating::Weak => "weak",
                    ForceRating::Good => "good",
                    ForceRating::Great => "great",
                    ForceRating::Fine => "fine",
                    ForceRating::Strong => "strong",
                    ForceRating::TooMuch => "too much",
                    ForceRating::Dangerous => "dangerous",
                }
                .to_owned(),
                up_to: band.up_to,
                inclusive: band.inclusive,
            })
            .collect()
    };
    let display = ENVELOPE_DISPLAY_LIMITS;
    ConstructionForceLimitsWire {
        minimum_vertical_g: display.minimum_vertical_g,
        maximum_vertical_g: display.maximum_vertical_g,
        rating_bands: ConstructionForceRatingBandsWire {
            airtime: bands(ForceRatingAxis::Airtime),
            vertical: bands(ForceRatingAxis::Vertical),
            lateral: bands(ForceRatingAxis::Lateral),
            longitudinal: bands(ForceRatingAxis::Longitudinal),
        },
        maximum_lateral_g: display.maximum_lateral_g,
        maximum_longitudinal_g: display.maximum_longitudinal_g,
        design_targets: ConstructionDesignTargetsWire {
            model_name: model_name.to_owned(),
            minimum_vertical_g: design_targets.minimum_vertical_g,
            maximum_vertical_g: design_targets.maximum_vertical_g,
            maximum_lateral_g: design_targets.maximum_lateral_g,
            maximum_longitudinal_g: design_targets.maximum_longitudinal_g,
        },
    }
}

fn open_track_position(pieces: &[Piece], distance: f64) -> Vec3 {
    let mut piece_start = 0.0;
    for piece in pieces {
        let piece_end = piece_start + piece.length();
        if distance <= piece_end {
            return piece
                .sample((distance - piece_start).clamp(0.0, piece.length()))
                .position;
        }
        piece_start = piece_end;
    }
    panic!("force preview sample lies beyond the open track");
}

fn testing_sample_from_force_envelope(
    pieces: &[Piece],
    sample: PreviewForceSample,
    limits: DesignForceTargets,
) -> crate::reports::TestingSample {
    let vertical_force = if sample.minimum_vertical_g / limits.minimum_vertical_g
        > sample.maximum_vertical_g / limits.maximum_vertical_g
    {
        sample.minimum_vertical_g
    } else {
        sample.maximum_vertical_g
    };
    let position = open_track_position(pieces, sample.distance) / 4.0;
    crate::reports::TestingSample {
        distance: sample.distance,
        x: position.x,
        y: position.y,
        z: position.z,
        speed: sample.speed,
        vertical_g: vertical_force,
        lateral_g: sample.lateral_g,
        longitudinal_g: sample.longitudinal_g,
        airtime: vertical_force < 0.0,
        force_limit_exceeded: sample.force_limit_exceeded,
    }
}

fn piece_force_peaks_wire(peaks: &[PieceForcePeaks]) -> Vec<crate::reports::PieceForcePeaksReport> {
    peaks
        .iter()
        .map(|peaks| crate::reports::PieceForcePeaksReport {
            piece_index: peaks.piece_index,
            front_car: peaks.front_car.map(Into::into),
            worst_car: peaks.worst_car.map(Into::into),
        })
        .collect()
}

fn worst_car_force_peaks_for_piece_range(
    peaks: &[PieceForcePeaks],
    first_piece_index: usize,
    last_piece_index: usize,
) -> Option<ForcePeakValues> {
    let mut result: Option<ForcePeakValues> = None;
    for piece_peaks in peaks
        .iter()
        .filter(|peaks| (first_piece_index..=last_piece_index).contains(&peaks.piece_index))
    {
        let Some(candidate) = piece_peaks.worst_car else {
            continue;
        };
        result = Some(result.map_or(candidate, |current| {
            ForcePeakValues {
                minimum_vertical_g: current.minimum_vertical_g.min(candidate.minimum_vertical_g),
                maximum_vertical_g: current.maximum_vertical_g.max(candidate.maximum_vertical_g),
                maximum_lateral_g: current.maximum_lateral_g.max(candidate.maximum_lateral_g),
                maximum_longitudinal_g: current
                    .maximum_longitudinal_g
                    .max(candidate.maximum_longitudinal_g),
            }
        }));
    }
    result
}

fn last_placed_force_wire(
    samples: &[crate::reports::TestingSample],
    start_distance: f64,
    end_distance: f64,
    force_peaks: Option<ForcePeakValues>,
) -> Option<ConstructionLastPlacedForceWire> {
    let samples: Vec<_> = samples
        .iter()
        .filter(|sample| sample.distance >= start_distance && sample.distance <= end_distance)
        .collect();
    if samples.is_empty() {
        return None;
    }
    let exceeded = |axis: usize| {
        samples
            .iter()
            .any(|sample| sample.force_limit_exceeded[axis])
    };
    let measured_peaks =
        force_peaks.expect("nonempty force envelope must have per-piece peak measurements");
    let minimum_vertical_limit_exceeded = samples
        .iter()
        .any(|sample| sample.force_limit_exceeded[0] && sample.vertical_g < 0.0);
    let maximum_vertical_limit_exceeded = samples
        .iter()
        .any(|sample| sample.force_limit_exceeded[0] && sample.vertical_g > 0.0);
    Some(ConstructionLastPlacedForceWire {
        minimum_vertical_limit_exceeded,
        maximum_vertical_limit_exceeded,
        lateral_limit_exceeded: exceeded(1),
        longitudinal_limit_exceeded: exceeded(2),
        maximum_speed: samples
            .iter()
            .map(|sample| sample.speed)
            .fold(f64::NEG_INFINITY, f64::max),
        minimum_vertical_g: measured_peaks.minimum_vertical_g,
        maximum_vertical_g: measured_peaks.maximum_vertical_g,
        maximum_absolute_lateral_g: measured_peaks.maximum_lateral_g,
        maximum_absolute_longitudinal_g: measured_peaks.maximum_longitudinal_g,
    })
}

fn entry_speed(samples: &[crate::reports::TestingSample], distance: f64) -> Option<f64> {
    let next_index = samples.partition_point(|sample| sample.distance < distance);
    let next = samples.get(next_index)?;
    if next.distance == distance {
        return Some(next.speed);
    }
    let previous = samples.get(next_index.checked_sub(1)?)?;
    let fraction = (distance - previous.distance) / (next.distance - previous.distance);
    Some(previous.speed + fraction * (next.speed - previous.speed))
}

pub(crate) fn colors_json(colors: Option<CoasterColors>) -> String {
    let colors = colors.map(|colors| ConstructionColorsWire {
        track: colors.track.name(),
        supports: colors.supports.name(),
        trains: colors.trains.name(),
    });
    serde_json::to_string(&colors).expect("coaster colors serialize")
}

pub(crate) fn colors_value(colors: Option<CoasterColors>) -> serde_json::Value {
    colors.map_or(serde_json::Value::Null, |colors| {
        serde_json::json!({
            "track": colors.track.name(),
            "supports": colors.supports.name(),
            "trains": colors.trains.name(),
        })
    })
}

pub(crate) fn train_layout_value(
    model: &CoasterModel,
    train: &TrainConfiguration,
) -> serde_json::Value {
    serde_json::to_value(train_layout(model, train)).expect("serialize train layout")
}

fn element_representative(element: &ElementChoice) -> &Piece {
    element
        .geometry
        .iter()
        .find(|piece| piece.entry.heading != piece.exit.heading)
        .or_else(|| {
            element
                .geometry
                .iter()
                .max_by_key(|piece| slope_rank(piece.entry.slope).max(slope_rank(piece.exit.slope)))
        })
        .or_else(|| element.geometry.first())
        .expect("an element choice contains at least one piece")
}

fn heartlined(heartlining: Option<Heartlining>) -> Option<bool> {
    heartlining.map(|heartlining| heartlining == Heartlining::Heartlined)
}

fn slope_rank(slope: Slope) -> u8 {
    match slope {
        Slope::Level => 0,
        Slope::UpGentle | Slope::DownGentle => 1,
        Slope::UpModerate | Slope::DownModerate => 2,
        Slope::UpSteep | Slope::DownSteep => 3,
        Slope::UpSixty | Slope::DownSixty => 4,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoasterListEntry<'a> {
    id: u32,
    name: &'a str,
    model: &'a str,
    open: bool,
    price: u16,
    income: u64,
    queue_length: usize,
    complete: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoasterRideInfo<'a> {
    id: u32,
    name: &'a str,
    model: &'static str,
    model_name: String,
    manufacturer: String,
    limits: ConstructionForceLimitsWire,
    report: Option<crate::reports::TestingReport>,
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_start(&mut self, x: i32, y: i32, z: i32, heading: u8) {
        self.start_coaster(x, y, z, heading);
    }

    /// Adds a coaster and makes it the coaster being edited.
    pub fn coaster_start(&mut self, x: i32, y: i32, z: i32, heading: u8) {
        self.start_coaster(x, y, z, heading);
    }

    fn start_coaster(&mut self, x: i32, y: i32, z: i32, heading: u8) {
        self.construction_finish_invalidate();
        let heading = match heading {
            0 => Heading::East,
            1 => Heading::NorthEast,
            2 => Heading::North,
            3 => Heading::NorthWest,
            4 => Heading::West,
            5 => Heading::SouthWest,
            6 => Heading::South,
            7 => Heading::SouthEast,
            _ => {
                self.construction_message = "Invalid heading".into();
                return;
            }
        };
        let mut construction = Construction::new(
            TrackState {
                x,
                y,
                z,
                heading,
                ..TrackState::origin()
            },
            coaster_model(&self.construction_model),
        );
        let obstacles = self.facility_obstacles_excluding(None);
        let station_id = construction.next_piece_id();
        let coaster_id = CoasterId(self.park.next_coaster_id);
        let station_piece = construction
            .candidate_piece(PieceId::Station)
            .expect("station piece is in the coaster catalog");
        if let Err(error) =
            self.park
                .validate_track_clearance(&construction.model().train, &station_piece, None)
        {
            self.construction_message = construction_error(error);
            return;
        }
        let station_plan =
            match construction.tunnel_plan(PieceId::Station, &self.park.world, &obstacles) {
                Ok(plan) => plan,
                Err(error) => {
                    self.construction_message = construction_error(error);
                    return;
                }
            };
        if let Err(error) = self
            .park
            .carve_for_coaster(coaster_id, &[(station_id, station_plan)])
        {
            self.construction_message = error.to_string();
            return;
        }
        if let Err(error) = construction.append(PieceId::Station, &self.park.world, &obstacles) {
            self.park
                .restore_tunnel_piece_for_coaster(coaster_id, station_id);
            self.construction_message = construction_error(error);
            return;
        }
        let added_id = self.park.add_coaster(construction);
        assert_eq!(
            added_id, coaster_id,
            "coaster ids are assigned monotonically"
        );
        self.editing = Some(added_id);
        self.selected = None;
        self.selected_shape = None;
        self.selected_element = None;
        self.recovered_station = None;
        self.select_compatible_piece();
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
    }

    pub fn coaster_edit(&mut self, id: u32) -> bool {
        let coaster_id = CoasterId(id);
        if self.park.coaster(coaster_id).is_none() {
            return false;
        }
        self.construction_finish_invalidate();
        self.park.close_live_coaster(coaster_id);
        self.editing = Some(coaster_id);
        self.construction_model = self
            .park
            .coaster(coaster_id)
            .expect("coaster still exists")
            .construction
            .model()
            .key
            .to_owned();
        self.selected = None;
        self.selected_shape = None;
        self.selected_element = None;
        self.recovered_station = None;
        self.construction_message.clear();
        self.select_compatible_piece();
        self.refresh_construction_support_rendering();
        true
    }

    pub fn coaster_stop_editing(&mut self) {
        self.construction_finish_invalidate();
        self.editing = None;
        self.selected = None;
        self.selected_shape = None;
        self.selected_element = None;
        self.recovered_station = None;
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
    }

    pub fn coaster_demolish(&mut self, id: u32) -> bool {
        let coaster_id = CoasterId(id);
        if self.park.coaster(coaster_id).is_none() {
            return false;
        }
        if self.editing_coaster_id() == Some(coaster_id) {
            self.construction_finish_invalidate();
            self.editing = None;
            self.selected = None;
            self.selected_shape = None;
            self.selected_element = None;
            self.recovered_station = None;
        }
        let demolished = self.park.demolish_coaster(coaster_id);
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
        demolished
    }

    pub fn coaster_rename(&mut self, id: u32, name: String) -> bool {
        self.park.rename_coaster(CoasterId(id), name)
    }

    pub fn coasters_json(&self) -> String {
        let coasters: Vec<_> = self
            .park
            .coasters()
            .map(|(id, coaster)| CoasterListEntry {
                id: id.0,
                name: &coaster.name,
                model: coaster.construction.model().key,
                open: coaster.coaster_open,
                price: coaster.coaster_price,
                income: self.park.coaster_income(id),
                queue_length: self.park.simulation.queue_length(u32::MAX - id.0),
                complete: coaster.construction.circuit().is_ok(),
            })
            .collect();
        serde_json::to_string(&coasters).expect("serialize coaster list")
    }

    pub fn coaster_set_open(&mut self, id: u32, open: bool) -> bool {
        let coaster_id = CoasterId(id);
        if self.park.coaster(coaster_id).is_none() {
            return false;
        }
        match self.park.construction_set_open(coaster_id, open) {
            Ok(()) => true,
            Err(error) => {
                let message = crate::command_errors::block_error_message(error);
                if let Some(coaster) = self.park.coaster_mut(coaster_id) {
                    coaster.block_message = message.to_owned();
                }
                self.park.construction_message = message.to_owned();
                false
            }
        }
    }

    pub fn coaster_set_price(&mut self, id: u32, price: u16) -> bool {
        let coaster_id = CoasterId(id);
        if self.park.coaster(coaster_id).is_none() {
            return false;
        }
        self.park.set_coaster_price(Some(coaster_id), price);
        true
    }

    pub fn construction_active(&self) -> bool {
        self.editing_coaster().is_some()
    }

    pub fn construction_chain(&self) -> bool {
        self.editing_construction().is_some_and(Construction::chain)
    }

    pub fn construction_set_chain(&mut self, enabled: bool) {
        self.construction_finish_invalidate();
        if let Some(construction) = self.editing_construction_mut() {
            construction.set_chain(enabled);
            self.construction_message.clear();
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_set_launch_intensity(&mut self, id: usize, target_speed: f64) -> bool {
        self.construction_set_launch_intensity_result(id, target_speed)
            .is_ok()
    }
}

impl WasmSimulation {
    pub fn construction_set_launch_intensity_result(
        &mut self,
        id: usize,
        target_speed: f64,
    ) -> Result<(), String> {
        self.construction_finish_invalidate();
        let coaster = self
            .editing_coaster_mut()
            .ok_or("Construction is inactive".to_owned())?;
        coaster
            .construction
            .set_launch_intensity(id, target_speed)
            .map_err(construction_error)?;
        coaster.construction_recorder = None;
        coaster.construction_report = None;
        coaster.construction_train = None;
        coaster.construction_outcome = None;
        self.construction_message.clear();
        Ok(())
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    /// The coaster's track, supports and train colours, or null without a coaster.
    pub fn construction_colors(&self) -> String {
        colors_json(
            self.editing_construction()
                .map(|construction| construction.colors()),
        )
    }

    /// The coaster's train layout, or null without a coaster.
    pub fn construction_train_layout(&self) -> String {
        let layout = self
            .editing_construction()
            .map(|construction| train_layout(construction.model(), &construction.model().train));
        serde_json::to_string(&layout).expect("serialize train layout")
    }

    /// Estimates speed and rider forces over the pieces already placed in construction.
    pub fn construction_force_preview_json(&self) -> String {
        serde_json::to_string(&self.construction_force_preview())
            .expect("construction force preview serializes")
    }
}

impl WasmSimulation {
    pub fn construction_finish_options(&self) -> ConstructionFinishOptionsWire {
        ConstructionFinishOptionsWire {
            previewed_index: self
                .construction_completion
                .is_some()
                .then_some(self.construction_completion_index),
            options: self
                .construction_completion_available
                .iter()
                .map(|completion| ConstructionFinishOptionWire {
                    rank: completion.score as usize,
                    pieces: completion
                        .geometry
                        .iter()
                        .map(|piece| format!("{:?}", piece.id))
                        .collect(),
                    arrival_speed_mps: completion.physics.arrival_speed_mps,
                    brake_energy_kj: completion.physics.brake_energy_kj,
                    minimum_force_headroom_g: completion.physics.minimum_force_headroom_g,
                    minimum_rated_margin_mps: completion.physics.minimum_rated_margin_mps,
                })
                .collect(),
        }
    }

    /// The finish options with their full physics reports, for native tools.
    pub fn construction_finish_completions(&self) -> &[Completion] {
        &self.construction_completion_available
    }

    pub fn construction_finish_search_start_with_judge(
        &mut self,
        judge: theme_park_sim::track::construction::FinishForceJudge,
    ) {
        self.construction_completion_available.clear();
        self.construction_completion_index = 0;
        let search = self.editing_construction().map_or_else(
            || Err("Place a station before finishing the coaster.".to_owned()),
            |construction| construction.completion_search_with_judge(judge),
        );
        match search {
            Ok(search) => {
                self.construction_completion_search = Some(search);
                self.construction_completion_unavailable_reason = None;
                self.construction_completion_checked = false;
            }
            Err(reason) => {
                self.construction_completion_search = None;
                self.construction_completion_unavailable_reason = Some(reason);
                self.construction_completion_checked = true;
            }
        }
    }

    pub fn construction_force_preview(&self) -> Option<ConstructionForcePreviewWire> {
        let construction = self.editing_construction()?;
        let pieces = construction.pieces();
        let limits = ENVELOPE_DISPLAY_LIMITS;
        let preview = construction.force_preview(
            pieces,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
        )?;
        let samples: Vec<_> = preview
            .report
            .samples
            .iter()
            .map(|sample| {
                let position = open_track_position(pieces, sample.distance) / 4.0;
                crate::reports::TestingSample {
                    distance: sample.distance,
                    x: position.x,
                    y: position.y,
                    z: position.z,
                    speed: sample.speed,
                    vertical_g: sample.vertical_g,
                    lateral_g: sample.lateral_g,
                    longitudinal_g: sample.longitudinal_g,
                    airtime: sample.airtime,
                    force_limit_exceeded: sample.force_limit_exceeded,
                }
            })
            .collect();
        let force_envelope_samples: Vec<_> = preview
            .force_envelope_samples
            .iter()
            .copied()
            .map(|sample| testing_sample_from_force_envelope(pieces, sample, limits))
            .collect();
        let piece_force_peaks = piece_force_peaks_wire(&preview.piece_force_peaks);
        let last_placed = pieces.last().and_then(|piece| {
            (piece.kind != PieceKind::Station).then(|| {
                let end_distance: f64 = pieces.iter().map(Piece::length).sum();
                let piece_index = pieces.len() - 1;
                let worst_peaks = worst_car_force_peaks_for_piece_range(
                    &preview.piece_force_peaks,
                    piece_index,
                    piece_index,
                );
                last_placed_force_wire(
                    &force_envelope_samples,
                    end_distance - piece.length(),
                    end_distance,
                    worst_peaks,
                )
            })?
        });
        let mut start_distance = 0.0;
        let launches = pieces
            .iter()
            .zip(construction.piece_ids())
            .enumerate()
            .filter_map(|(piece_index, (piece, id))| {
                let end_distance = start_distance + piece.length();
                let result = (piece.kind == PieceKind::Launch).then(|| {
                    let maximum_speed = construction
                        .launch_speed_bounds(*id)
                        .expect("launch has speed bounds")
                        .maximum;
                    let peak_longitudinal_g = force_envelope_samples
                        .iter()
                        .filter(|sample| {
                            sample.distance >= start_distance && sample.distance <= end_distance
                        })
                        .map(|sample| sample.longitudinal_g.abs())
                        .reduce(f64::max);
                    ConstructionLaunchEstimateWire {
                        piece_index,
                        maximum_speed,
                        peak_longitudinal_g,
                    }
                });
                start_distance = end_distance;
                result
            })
            .collect();
        let status = match preview.status {
            PreviewStatus::ReachedEnd => "reachedEnd",
            PreviewStatus::Stalled => "stalled",
            PreviewStatus::TimedOut => "timedOut",
        };
        Some(ConstructionForcePreviewWire {
            samples,
            force_envelope_samples,
            piece_force_peaks,
            completed: preview.report.completed,
            status,
            last_placed,
            launches,
            limits: force_limits_wire(
                construction.model().design_force_targets,
                construction.model().name,
            ),
        })
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    /// Estimates the selected candidate using the placed track and the same train physics.
    pub fn construction_candidate_force_preview_json(&self) -> String {
        serde_json::to_string(&self.construction_candidate_force_preview())
            .expect("candidate force preview serializes")
    }
}

impl WasmSimulation {
    pub fn construction_candidate_force_preview(
        &self,
    ) -> Option<ConstructionCandidateForcePreviewWire> {
        if self.selected_shape.is_some() {
            return None;
        }
        let construction = self.editing_construction()?;
        // Undoing the station leaves an empty construction: there is no train to estimate from.
        if construction.pieces().is_empty() {
            return None;
        }
        let candidate = self.construction_preview();
        let first = candidate.first()?;
        let outer_bank = first.family.is_some_and(|family| {
            matches!(
                family,
                PieceFamily::Hill(theme_park_sim::track::HillFamily {
                    twist: Some(HillTwist::OuterBankLeft | HillTwist::OuterBankRight),
                    ..
                })
            )
        });
        let rated_speed = rated_speed_wire(construction.model(), first);
        let start_distance: f64 = construction.pieces().iter().map(Piece::length).sum();
        let mut pieces = construction.pieces().to_vec();
        pieces.extend(candidate);
        let end_distance: f64 = pieces.iter().map(Piece::length).sum();
        let preview = construction
            .force_preview(
                &pieces,
                PhysicsConfig {
                    lift_speed: 5.0,
                    ..PhysicsConfig::default()
                },
            )
            .expect("construction contains a station");
        let limits = ENVELOPE_DISPLAY_LIMITS;
        let samples: Vec<_> = preview
            .report
            .samples
            .iter()
            .map(|sample| {
                let position = open_track_position(&pieces, sample.distance) / 4.0;
                crate::reports::TestingSample {
                    distance: sample.distance,
                    x: position.x,
                    y: position.y,
                    z: position.z,
                    speed: sample.speed,
                    vertical_g: sample.vertical_g,
                    lateral_g: sample.lateral_g,
                    longitudinal_g: sample.longitudinal_g,
                    airtime: sample.airtime,
                    force_limit_exceeded: sample.force_limit_exceeded,
                }
            })
            .collect();
        let force_envelope_samples: Vec<_> = preview
            .force_envelope_samples
            .iter()
            .copied()
            .filter(|sample| sample.distance >= start_distance && sample.distance <= end_distance)
            .map(|sample| testing_sample_from_force_envelope(&pieces, sample, limits))
            .collect();
        let entry_speed = entry_speed(&samples, start_distance);
        let first_candidate_piece_index = construction.pieces().len();
        let last_candidate_piece_index = pieces.len() - 1;
        let worst_peaks = worst_car_force_peaks_for_piece_range(
            &preview.piece_force_peaks,
            first_candidate_piece_index,
            last_candidate_piece_index,
        );
        let status = match preview.status {
            PreviewStatus::ReachedEnd => "reachedEnd",
            PreviewStatus::Stalled => "stalled",
            PreviewStatus::TimedOut => "timedOut",
        };
        Some(ConstructionCandidateForcePreviewWire {
            status,
            outer_bank,
            entry_speed,
            peaks: last_placed_force_wire(
                &force_envelope_samples,
                start_distance,
                end_distance,
                worst_peaks,
            ),
            samples: samples
                .into_iter()
                .filter(|sample| {
                    sample.distance >= start_distance && sample.distance <= end_distance
                })
                .collect(),
            force_envelope_samples,
            piece_force_peaks: piece_force_peaks_wire(&preview.piece_force_peaks),
            limits: force_limits_wire(
                construction.model().design_force_targets,
                construction.model().name,
            ),
            rated_speed,
        })
    }

    pub fn construction_candidate_geometry(&self) -> Option<ConstructionCandidateGeometryWire> {
        let candidate = self.construction_preview();
        let end = candidate.last()?.exit.position();
        Some(ConstructionCandidateGeometryWire {
            end_tile_x: end.x / 4.0,
            end_tile_y: end.y / 4.0,
            end_height: end.z,
            length: candidate.iter().map(Piece::length).sum(),
        })
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    /// Repaints the coaster. Names outside the palette leave it unchanged and explain why in the construction message.
    pub fn construction_set_colors(&mut self, track: &str, supports: &str, trains: &str) {
        let Some(colors) = CoasterColors::from_names(track, supports, trains) else {
            self.construction_message = "Unknown coaster colour".into();
            return;
        };
        if let Some(construction) = self.editing_construction_mut() {
            construction.set_colors(colors);
            self.construction_message.clear();
        }
    }

    pub fn construction_hill_options(&self) -> String {
        let model = self
            .editing_construction()
            .map(|construction| construction.model().clone())
            .unwrap_or_else(|| coaster_model(&self.construction_model));
        let options: Vec<_> = theme_park_sim::track::catalog()
            .iter()
            .filter_map(|piece| {
                let PieceFamily::Hill(family) = piece.family? else {
                    return None;
                };
                model
                    .allowed_pieces
                    .contains(&piece.id)
                    .then(|| ConstructionHillOptionWire {
                        id: wire_pieceid(piece.id),
                        feel: hill_feel_wire(family.feel),
                        height: family.height_metres,
                        span: family.span_metres,
                        twist: hill_twist_wire(family.twist),
                        rated_speed: rated_speed_wire(&model, piece),
                    })
            })
            .collect();
        serde_json::to_string(&options).expect("hill options serialize")
    }

    /// The model's speed-matched turns, so the builder can offer "For speed" before one is selected.
    pub fn construction_speed_turns(&self) -> String {
        let Some(construction) = self.editing_construction() else {
            return "[]".into();
        };
        let turns: Vec<_> = theme_park_sim::track::catalog()
            .iter()
            .filter_map(|piece| {
                let PieceFamily::SpeedTurn {
                    hand,
                    design_speed_metres_per_second,
                    ..
                } = piece.family?
                else {
                    return None;
                };
                construction
                    .model()
                    .allowed_pieces
                    .contains(&piece.id)
                    .then(|| ConstructionSpeedTurnWire {
                        id: wire_pieceid(piece.id),
                        hand: match hand {
                            Hand::Left => "left",
                            Hand::Right => "right",
                        },
                        design_speed: design_speed_metres_per_second,
                    })
            })
            .collect();
        serde_json::to_string(&turns).expect("speed turns serialize")
    }

    pub fn construction_select(&mut self, id: &str) {
        self.selected_element = None;
        if let Some(key) = id.strip_prefix("model:") {
            self.selected_shape = None;
            let model = coaster_model(key);
            self.construction_model = model.key.to_owned();
            self.construction_message.clear();
            self.refresh_construction_support_rendering();
            return;
        }
        if let Some(element_id) = id.strip_prefix("element:") {
            self.selected_shape = None;
            let obstacles = self.facility_obstacles();
            self.selected = None;
            self.selected_element = self.editing_construction().and_then(|construction| {
                construction
                    .element_choices(&self.world, &obstacles)
                    .into_iter()
                    .find(|element| element.id == element_id)
                    .map(|element| element.id.to_owned())
            });
            self.construction_message = if self.selected_element.is_some() {
                String::new()
            } else {
                "Unknown element".into()
            };
            self.refresh_construction_support_rendering();
            return;
        }
        if id.starts_with("shape:") {
            self.selected = None;
            self.selected_element = None;
            let model = self
                .editing_construction()
                .map(|construction| construction.model().clone())
                .unwrap_or_else(|| coaster_model(&self.construction_model));
            match parse_shape_selection(id, &model) {
                Ok(intent) => {
                    self.selected_shape = Some(intent);
                    self.construction_message.clear();
                    self.refresh_construction_support_rendering();
                }
                Err(error) => {
                    self.selected_shape = None;
                    self.construction_message = error;
                    self.construction_preview_supports_cache.clear();
                }
            }
            return;
        }
        self.selected_shape = None;
        let id = if let Some(parameters) = id.strip_prefix("hill:") {
            let matching = theme_park_sim::track::catalog().iter().find(|piece| {
                piece.family.is_some_and(|family| {
                    let PieceFamily::Hill(family) = family else {
                        return false;
                    };
                    format!(
                        "{}:{}x{}:{}",
                        hill_feel_wire(family.feel),
                        family.height_metres,
                        family.span_metres,
                        hill_twist_wire(family.twist)
                    ) == parameters
                })
            });
            matching.map(|piece| wire_pieceid(piece.id)).unwrap_or("")
        } else {
            id
        };
        self.selected = pieceid_from_wire(id);
        if let Some(selected) = self.selected {
            let obstacles = self.facility_obstacles();
            if let Some(editing_id) = self.editing_coaster_id() {
                let park = &mut self.park;
                let (world, coasters) = (&park.world, &mut park.coasters);
                if let Some(coaster) = coasters.get_mut(&editing_id) {
                    if theme_park_sim::track::catalog()
                        .iter()
                        .any(|piece| piece.id == selected && piece.family.is_some())
                    {
                        let _ = coaster
                            .construction
                            .select_hill_variant(selected, world, &obstacles);
                    }
                }
            }
        }
        self.construction_message = if self.selected.is_some() {
            String::new()
        } else {
            "Unknown piece".into()
        };
        if self.selected.is_some() {
            self.refresh_construction_support_rendering();
        } else {
            self.construction_preview_supports_cache.clear();
        }
    }

    pub fn construction_selected(&self) -> String {
        if let Some(intent) = self.selected_shape.as_ref() {
            return shape_selection_id(intent);
        }
        if let Some(element_id) = self.selected_element.as_deref() {
            return format!("element:{element_id}");
        }
        self.selected
            .map(|id| wire_pieceid(id).to_owned())
            .unwrap_or_default()
    }

    pub(crate) fn construction_shape_candidate(
        &self,
        intent: &ElementIntent,
    ) -> Result<theme_park_sim::track::shaped::SolvedElement, SolveRefusal> {
        // The builder displays and validates the same calibrated shape that append confirms.
        self.construction_shape_candidate_with_mode(intent, false)
    }

    fn construction_shape_candidate_for_confirm(
        &self,
        intent: &ElementIntent,
    ) -> Result<theme_park_sim::track::shaped::SolvedElement, SolveRefusal> {
        self.construction_shape_candidate_with_mode(intent, false)
    }

    fn construction_shape_candidate_with_mode(
        &self,
        intent: &ElementIntent,
        provisional: bool,
    ) -> Result<theme_park_sim::track::shaped::SolvedElement, SolveRefusal> {
        let construction = self
            .editing_construction()
            .ok_or(SolveRefusal::InvalidContext)?;
        let prefix = construction.pieces();
        let preview = construction
            .force_preview(
                prefix,
                PhysicsConfig {
                    lift_speed: 5.0,
                    ..PhysicsConfig::default()
                },
            )
            .ok_or(SolveRefusal::PreviewTimedOut)?;
        if preview.status == PreviewStatus::TimedOut {
            return Err(SolveRefusal::PreviewTimedOut);
        }
        let entry_speed_mps = preview
            .end_speed_mps
            .filter(|speed| speed.is_finite() && *speed > 0.0)
            .ok_or(SolveRefusal::InvalidContext)?;
        let physics = PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        };
        self.construction_shape_candidate_at_speed(intent, entry_speed_mps, physics, provisional)
    }

    fn construction_shape_candidate_at_speed(
        &self,
        intent: &ElementIntent,
        entry_speed_mps: f64,
        physics: PhysicsConfig,
        provisional: bool,
    ) -> Result<theme_park_sim::track::shaped::SolvedElement, SolveRefusal> {
        let construction = self
            .editing_construction()
            .ok_or(SolveRefusal::InvalidContext)?;
        if provisional {
            construction.shape_candidate_preview(intent, entry_speed_mps, physics)
        } else {
            construction.shape_candidate(intent, entry_speed_mps, physics)
        }
    }

    pub(crate) fn construction_selected_shape_wire(
        &self,
        _include_calibration: bool,
    ) -> Option<ConstructionShapedElementWire> {
        let intent = self.selected_shape.as_ref()?;
        let model_name = self
            .editing_construction()
            .map_or("", |construction| construction.model().name);
        let selection = shape_selection_id(intent);
        let (kind, feel, hand) = match intent {
            ElementIntent::Hill { .. } => ("hill", shape_feel(intent), None),
            ElementIntent::Turn { .. } => ("turn", shape_feel(intent), shape_hand(intent)),
            ElementIntent::Loop { .. } => ("loop", shape_feel(intent), shape_hand(intent)),
            ElementIntent::ZeroGRoll { .. } => {
                ("zeroGRoll", shape_feel(intent), shape_hand(intent))
            }
            ElementIntent::RavenTurn { .. } => {
                ("ravenTurn", shape_feel(intent), shape_hand(intent))
            }
        };
        match self.construction_shape_candidate(intent) {
            Ok(solved) => Some(ConstructionShapedElementWire {
                selection,
                kind,
                feel,
                hand,
                strength_g: shape_strength(intent),
                fixed_size: shape_size_hint(intent),
                model_name,
                exceeds_model_feel: solved.exceeds_model_feel,
                spec: Some(shaped_spec_wire(&solved.spec)),
                refusal: None,
                piece_shape: Some(crate::reports::piece_shape_report(&solved.piece)),
            }),
            Err(refusal) => Some(ConstructionShapedElementWire {
                selection,
                kind,
                feel,
                hand,
                strength_g: shape_strength(intent),
                fixed_size: shape_size_hint(intent),
                model_name,
                exceeds_model_feel: false,
                spec: None,
                refusal: Some(solve_refusal_wire(refusal)),
                piece_shape: None,
            }),
        }
    }

    pub fn construction_choices(&self) -> String {
        self.construction_choices_with_preview(None, "")
    }

    pub fn construction_choices_with_preview(
        &self,
        entry_speed: Option<f64>,
        status: &str,
    ) -> String {
        serde_json::to_string(&self.construction_choices_preview(entry_speed, status))
            .expect("construction choices serialize")
    }
}

impl WasmSimulation {
    pub fn construction_choices_preview(
        &self,
        entry_speed: Option<f64>,
        status: &str,
    ) -> Vec<ConstructionPieceWire> {
        let obstacles = self.facility_obstacles();
        let choices: Vec<_> = self
            .editing_construction()
            .map(|construction| {
                construction
                    .available_choices(&self.world, &obstacles)
                    .into_iter()
                    .map(|(piece, choice)| ConstructionPieceWire {
                        id: wire_pieceid(piece.id),
                        label: piece.name,
                        group: piece_group(&piece),
                        heartlined: heartlined(piece.heartlining),
                        heading: wire_heading(piece.exit.heading),
                        slope: wire_slope(piece.exit.slope),
                        bank: wire_bank(piece.exit.bank),
                        refusal: choice
                            .result
                            .err()
                            .map(construction_error)
                            .unwrap_or_default(),
                        would_tunnel: choice.would_tunnel,
                        hill_estimate: if self.selected == Some(piece.id)
                            && matches!(piece.family, Some(PieceFamily::Hill(_)))
                        {
                            let (speed, source) = match status {
                                "reachedEnd" => (entry_speed, "simulated"),
                                "stalled" => (Some(0.0), "stalled"),
                                "timedOut" => (None, "timedOut"),
                                "" => (construction.estimated_end_speed(None).speed, "energy"),
                                _ => panic!("unknown construction preview status"),
                            };
                            let forces = speed.and_then(|speed| {
                                theme_park_sim::track::predicted_hill_forces(&piece, speed)
                            });
                            Some(ConstructionHillEstimateWire {
                                entry_speed: speed,
                                source,
                                rated_speed: rated_speed_wire(construction.model(), &piece),
                                minimum_vertical_g: forces
                                    .as_ref()
                                    .map(|force| force.minimum_vertical_g),
                                maximum_vertical_g: forces
                                    .as_ref()
                                    .map(|force| force.maximum_vertical_g),
                                maximum_absolute_lateral_g: forces.as_ref().and_then(|force| {
                                    piece.family.and_then(|family| match family {
                                        PieceFamily::Hill(family) => match family.twist {
                                            Some(
                                                HillTwist::OuterBankLeft
                                                | HillTwist::OuterBankRight,
                                            ) => Some(force.maximum_absolute_lateral_g),
                                            _ => None,
                                        },
                                        PieceFamily::SpeedTurn { .. } => None,
                                    })
                                }),
                                airtime_seconds: forces.as_ref().map(|force| force.airtime_seconds),
                                limits: force_limits_wire(
                                    construction.model().design_force_targets,
                                    construction.model().name,
                                ),
                            })
                        } else {
                            None
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        choices
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_elements(&self) -> String {
        serde_json::to_string(&self.construction_elements_typed())
            .expect("construction elements serialize")
    }
}

impl WasmSimulation {
    pub fn construction_elements_typed(&self) -> Vec<ConstructionElementWire> {
        let obstacles = self.facility_obstacles();
        let elements: Vec<_> = self
            .editing_construction()
            .map(|construction| {
                construction
                    .element_choices(&self.world, &obstacles)
                    .into_iter()
                    .map(|element| {
                        let representative = element_representative(&element);
                        ConstructionElementWire {
                            id: format!("element:{}", element.id),
                            name: element.name,
                            piece_id: wire_pieceid(representative.id),
                            heartlined: heartlined(element.heartlining),
                            slope: wire_slope(representative.exit.slope),
                            kind: wire_piecekind(representative.kind),
                            would_tunnel: element.would_tunnel,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        elements
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_track(&self) -> String {
        let track: Vec<_> = self
            .editing_construction()
            .map(|construction| {
                construction
                    .piece_ids()
                    .iter()
                    .zip(construction.pieces())
                    .chain(
                        construction
                            .disconnected_segments()
                            .iter()
                            .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
                    )
                    .map(|(id, piece)| ConstructionTrackWire {
                        id: *id,
                        label: wire_pieceid(piece.id),
                        name: piece.name,
                        group: piece_group(piece),
                        slope: wire_slope(piece.exit.slope),
                        kind: wire_piecekind(piece.kind),
                        launch_target_speed: (piece.kind == PieceKind::Launch)
                            .then(|| {
                                piece.launch_target_speed.or_else(|| {
                                    construction
                                        .launch_speed_bounds(*id)
                                        .map(|bounds| bounds.minimum)
                                })
                            })
                            .flatten(),
                        launch_minimum_speed: construction
                            .launch_speed_bounds(*id)
                            .map(|bounds| bounds.minimum),
                        launch_maximum_speed: construction
                            .launch_speed_bounds(*id)
                            .map(|bounds| bounds.maximum),
                        piece_shape: piece
                            .element_spec()
                            .map(|_| crate::reports::piece_shape_report(piece)),
                    })
                    .collect()
            })
            .unwrap_or_default();
        serde_json::to_string(&track).expect("construction track serializes")
    }

    #[cfg(feature = "shaped-benchmark")]
    pub fn construction_last_piece_arc_table_json(&self) -> String {
        let piece = self
            .editing_construction()
            .and_then(|construction| construction.pieces().last())
            .expect("construction has a placed shaped turn");
        assert!(matches!(
            piece.element_spec(),
            Some(theme_park_sim::track::shaped::ElementSpec::Turn { .. })
        ));
        piece.length();
        let serialized_piece = serde_json::to_value(piece).expect("piece serializes");
        let serialized_arc_table = serialized_piece
            .get("arc_table")
            .and_then(serde_json::Value::as_array)
            .and_then(|fields| fields.get(1))
            .filter(|table| !table.is_null())
            .expect("piece arc table is populated");
        serde_json::to_string(serialized_arc_table).expect("arc table serializes")
    }

    pub fn construction_can_append(&self) -> bool {
        let Some(construction) = self.editing_construction() else {
            return false;
        };
        let Some(editing_id) = self.editing_coaster_id() else {
            return false;
        };
        let train = &construction.model().train;
        let obstacles = self.facility_obstacles();
        if let Some(intent) = self.selected_shape.as_ref() {
            let Ok(solved) = self.construction_shape_candidate(intent) else {
                return false;
            };
            return self
                .park
                .validate_track_clearance(train, &solved.piece, Some(editing_id))
                .is_ok()
                && construction
                    .validate_shaped_piece(&solved.piece, &self.world, &obstacles)
                    .is_ok();
        }
        if let Some(element_id) = self.selected_element.as_deref() {
            return construction
                .element_choices(&self.world, &obstacles)
                .iter()
                .find(|element| element.id == element_id)
                .is_some_and(|element| {
                    element.geometry.iter().all(|piece| {
                        self.park
                            .validate_track_clearance(train, piece, Some(editing_id))
                            .is_ok()
                    })
                });
        }
        self.selected.is_some_and(|selected| {
            let valid_choice = construction
                .choices(&self.world, &obstacles)
                .iter()
                .any(|choice| choice.id == selected && choice.result.is_ok());
            valid_choice
                && construction.candidate_piece(selected).is_some_and(|piece| {
                    self.park
                        .validate_track_clearance(train, &piece, Some(editing_id))
                        .is_ok()
                })
        })
    }

    pub fn construction_preview_valid(&self) -> bool {
        self.construction_completion.is_some() || self.construction_can_append()
    }

    pub fn construction_preview_reason(&self) -> String {
        if let Some(intent) = self.selected_shape.as_ref() {
            let solved = match self.construction_shape_candidate(intent) {
                Ok(solved) => solved,
                Err(refusal) => return solve_refusal_message(refusal),
            };
            let editing_id = self
                .editing_coaster_id()
                .expect("shaped preview requires an edited coaster");
            let train = &self
                .editing_construction()
                .expect("shaped preview requires construction")
                .model()
                .train;
            if let Err(error) =
                self.park
                    .validate_track_clearance(train, &solved.piece, Some(editing_id))
            {
                return construction_error(error);
            }
            return self
                .editing_construction()
                .expect("solving shaped intent requires construction")
                .validate_shaped_piece(&solved.piece, &self.world, &self.facility_obstacles())
                .err()
                .map(construction_error)
                .unwrap_or_default();
        }
        if let Some(element_id) = self.selected_element.as_deref() {
            let Some((construction, editing_id)) =
                self.editing_construction().zip(self.editing_coaster_id())
            else {
                return "Construction is inactive".into();
            };
            let Some(element) = construction
                .element_choices(&self.world, &self.facility_obstacles())
                .into_iter()
                .find(|element| element.id == element_id)
            else {
                return "Element is no longer available here.".into();
            };
            for piece in &element.geometry {
                if let Err(error) = self.park.validate_track_clearance(
                    &construction.model().train,
                    piece,
                    Some(editing_id),
                ) {
                    return construction_error(error);
                }
            }
            return String::new();
        }
        let Some((construction, editing_id, selected)) = self
            .editing_construction()
            .zip(self.editing_coaster_id())
            .zip(self.selected)
            .map(|((construction, id), selected)| (construction, id, selected))
        else {
            return String::new();
        };
        let choice_result = construction
            .choices(&self.world, &self.facility_obstacles())
            .into_iter()
            .find(|choice| choice.id == selected)
            .expect("selected piece is in catalog")
            .result
            .map_err(construction_error);
        if let Err(message) = choice_result {
            return message;
        }
        let piece = construction
            .candidate_piece(selected)
            .expect("selected piece is available");
        self.park
            .validate_track_clearance(&construction.model().train, &piece, Some(editing_id))
            .err()
            .map(construction_error)
            .unwrap_or_default()
    }

    pub fn construction_can_close(&self) -> bool {
        self.editing_construction().is_some_and(|construction| {
            !construction.is_closed() && Circuit::new(construction.pieces().to_vec()).is_ok()
        })
    }

    pub fn construction_can_test(&self) -> bool {
        self.editing_coaster_id()
            .is_some_and(|id| self.park.construction_can_test(id))
    }

    pub fn construction_is_open(&self) -> bool {
        self.editing_coaster()
            .is_some_and(|coaster| coaster.coaster_open)
    }

    pub fn construction_coaster_price(&self) -> u16 {
        self.park.coaster_price(self.editing_coaster_id())
    }

    pub fn construction_coaster_income(&self) -> u64 {
        self.park
            .coaster_income(self.editing_coaster_id().unwrap_or(CoasterId(0)))
    }

    pub fn construction_set_coaster_price(&mut self, price: u16) {
        self.park
            .set_coaster_price(self.editing_coaster_id(), price);
    }

    /// Queue entrance and exit tiles as x, y, height triples.
    pub fn construction_station_positions(&self) -> Vec<i32> {
        self.editing_construction()
            .and_then(|construction| {
                construction
                    .pieces()
                    .iter()
                    .find(|piece| piece.kind == PieceKind::Station)
            })
            .and_then(station_positions)
            .map_or_else(Vec::new, |(entrance, exit)| {
                [entrance, exit]
                    .into_iter()
                    .flat_map(|position| {
                        [
                            i32::from(position.x),
                            i32::from(position.y),
                            i32::from(position.height),
                        ]
                    })
                    .collect()
            })
    }

    pub fn construction_train_count(&self) -> usize {
        self.editing_coaster_id()
            .map_or(0, |id| self.park.construction_train_count(id))
    }

    pub fn construction_set_train_count(&mut self, count: usize) -> bool {
        self.construction_set_train_count_result(count).is_ok()
    }
}

impl WasmSimulation {
    pub fn construction_set_train_count_result(&mut self, count: usize) -> Result<(), String> {
        let result = self.editing_coaster_id().map_or(
            Err(theme_park_sim::park::block_analysis::BlockCommandError::InvalidTrainCount),
            |id| self.park.construction_set_train_count(id, count),
        );
        result.map_err(|error| match error {
            theme_park_sim::park::block_analysis::BlockCommandError::InvalidTrainCount => {
                "Train count must be positive and within the available block capacity.".to_owned()
            }
            error => crate::command_errors::block_error_message(error).to_owned(),
        })
    }
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_min_cars_per_train(&self) -> u8 {
        self.editing_construction().map_or(0, |construction| {
            *construction.model().cars_per_train.start()
        })
    }

    pub fn construction_max_cars_per_train(&self) -> u8 {
        self.editing_construction()
            .map_or(0, |construction| *construction.model().cars_per_train.end())
    }

    pub fn construction_set_cars_per_train(&mut self, cars: u8) -> bool {
        self.construction_finish_invalidate();
        let result = self
            .editing_coaster_id()
            .ok_or(theme_park_sim::park::block_analysis::BlockCommandError::NoConstruction);
        match result.and_then(|id| self.park.construction_set_cars_per_train(id, cars)) {
            Ok(()) => {
                self.refresh_construction_support_rendering();
                self.park.construction_message.clear();
                true
            }
            Err(error) => {
                self.park.construction_message =
                    crate::command_errors::block_error_message(error).to_owned();
                false
            }
        }
    }

    pub fn construction_set_open(&mut self, open: bool) -> bool {
        let result = match self.editing_coaster_id() {
            Some(id) => self.park.construction_set_open(id, open),
            None if open => Err(
                theme_park_sim::park::block_analysis::BlockCommandError::CompleteCircuitRequired,
            ),
            None => {
                self.park.construction_message.clear();
                Ok(())
            }
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                if matches!(
                    error,
                    theme_park_sim::park::block_analysis::BlockCommandError::Validation(_)
                ) {
                    self.record_block_error(error);
                } else {
                    self.park.construction_message =
                        crate::command_errors::block_error_message(error).to_owned();
                }
                false
            }
        }
    }

    /// Car index, seat index, and guest ID for each occupied seat.
    pub fn construction_riders(&self) -> Vec<u32> {
        let Some(construction) = self.editing_construction() else {
            return Vec::new();
        };
        let Some(coaster) = self.editing_coaster() else {
            return Vec::new();
        };
        let seats_per_car = u32::from(construction.model().train.rows_per_car)
            * u32::from(construction.model().train.seats_per_row);
        let mut trains: Vec<_> = coaster.coaster_riders.iter().collect();
        trains.sort_by_key(|(train_id, _)| **train_id);
        trains
            .into_iter()
            .flat_map(|(train_id, guests)| {
                guests.iter().enumerate().flat_map(move |(seat, guest)| {
                    let car_index = rider_car_index(
                        *train_id,
                        construction.model().train.cars,
                        seat / seats_per_car as usize,
                    );
                    [car_index, (seat as u32) % seats_per_car, guest.0]
                })
            })
            .collect()
    }

    pub fn construction_append(&mut self) {
        if let Some(intent) = self.selected_shape.clone() {
            let result = self
                .construction_shape_candidate_for_confirm(&intent)
                .map_err(solve_refusal_message)
                .and_then(|solved| self.construction_append_resolved_typed(&intent, &solved.spec));
            self.construction_message = result.err().unwrap_or_default();
            return;
        }
        self.construction_finish_invalidate();
        let obstacles = self.facility_obstacles();
        let selected = self.selected;
        let selected_element = self.selected_element.clone();
        let Some(editing_id) = self.editing_coaster_id() else {
            self.construction_message = "Construction is inactive".into();
            return;
        };
        let undo_group_size = selected_element
            .as_deref()
            .and_then(|element_id| {
                self.editing_construction().and_then(|construction| {
                    construction
                        .element_choices(&self.park.world, &obstacles)
                        .into_iter()
                        .find(|element| element.id == element_id)
                        .map(|element| element.pieces.len())
                })
            })
            .unwrap_or(1);
        let result: Result<(), String> = (|| {
            let construction = self
                .editing_construction()
                .ok_or_else(|| "Construction is inactive".to_string())?;
            if let Some(element_id) = selected_element.as_deref() {
                let element = construction
                    .element_choices(&self.park.world, &obstacles)
                    .into_iter()
                    .find(|element| element.id == element_id)
                    .ok_or_else(|| "Selected element is no longer available here.".to_string())?;
                for piece in &element.geometry {
                    self.park
                        .validate_track_clearance(
                            &construction.model().train,
                            piece,
                            Some(editing_id),
                        )
                        .map_err(construction_error)?;
                }
                let first_id = construction.next_piece_id();
                let tunnel_plans = construction
                    .element_tunnel_plans(&element, &self.park.world, &obstacles)
                    .map_err(construction_error)?;
                let piece_ids: Vec<_> = (0..tunnel_plans.len())
                    .map(|offset| first_id + offset)
                    .collect();
                let ownership: Vec<_> = piece_ids.iter().copied().zip(tunnel_plans).collect();
                if let Err(error) = self.park.carve_for_coaster(editing_id, &ownership) {
                    return Err(error.to_string());
                }
                let apply_result = {
                    let park = &mut self.park;
                    let (world, coasters) = (&park.world, &mut park.coasters);
                    coasters
                        .get_mut(&editing_id)
                        .expect("editing coaster exists")
                        .construction
                        .apply_element(&element, world, &obstacles)
                };
                if let Err(error) = apply_result {
                    for piece_id in piece_ids {
                        self.park
                            .restore_tunnel_piece_for_coaster(editing_id, piece_id);
                    }
                    return Err(construction_error(error));
                }
                return Ok(());
            }

            let selected = selected.ok_or_else(|| "Select a piece first".to_string())?;
            let piece = construction
                .candidate_piece(selected)
                .ok_or_else(|| "Selected piece is no longer available here.".to_string())?;
            self.park
                .validate_track_clearance(&construction.model().train, &piece, Some(editing_id))
                .map_err(construction_error)?;
            let piece_id = construction.next_piece_id();
            let tunnel_plan = construction
                .tunnel_plan(selected, &self.park.world, &obstacles)
                .map_err(construction_error)?;
            if let Err(error) = self
                .park
                .carve_for_coaster(editing_id, &[(piece_id, tunnel_plan)])
            {
                return Err(error.to_string());
            }
            let append_result = {
                let park = &mut self.park;
                let (world, coasters) = (&park.world, &mut park.coasters);
                coasters
                    .get_mut(&editing_id)
                    .expect("editing coaster exists")
                    .construction
                    .append(selected, world, &obstacles)
            };
            if let Err(error) = append_result {
                self.park
                    .restore_tunnel_piece_for_coaster(editing_id, piece_id);
                return Err(construction_error(error));
            }
            Ok(())
        })();
        let changed = result.is_ok();
        if changed {
            self.editing_coaster_mut()
                .expect("construction remains active")
                .construction_undo_groups
                .push(undo_group_size);
            self.selected_element = None;
            self.close_editing_live_coaster();
            self.invalidate_editing_track_state();
            self.select_compatible_piece();
        }
        self.construction_message = result.err().unwrap_or_default();
        if changed {
            self.refresh_construction_support_rendering();
        }
    }

    pub(crate) fn construction_append_resolved_typed(
        &mut self,
        intent: &ElementIntent,
        spec: &ElementSpec,
    ) -> Result<(), String> {
        self.construction_finish_invalidate();
        let obstacles = self.facility_obstacles();
        let Some(editing_id) = self.editing_coaster_id() else {
            return Err("Construction is inactive".to_owned());
        };
        let (first_id, tunnel_plan) = {
            let construction = self
                .editing_construction()
                .ok_or_else(|| "Construction is inactive".to_string())?;
            let piece = construction
                .piece_from_resolved_spec(spec)
                .map_err(solve_refusal_message)?;
            self.park
                .validate_track_clearance(&construction.model().train, &piece, Some(editing_id))
                .map_err(construction_error)?;
            let first_id = construction.next_piece_id();
            let tunnel_plans = construction
                .tunnel_plans_for_geometry(std::slice::from_ref(&piece), &self.park.world)
                .map_err(construction_error)?;
            let tunnel_plan = tunnel_plans
                .into_iter()
                .next()
                .expect("one shaped piece has one tunnel plan");
            (first_id, tunnel_plan)
        };
        let ownership = [(first_id, tunnel_plan)];
        self.park
            .carve_for_coaster(editing_id, &ownership)
            .map_err(|error| error.to_string())?;
        let result = {
            let park = &mut self.park;
            let (world, coasters) = (&park.world, &mut park.coasters);
            coasters
                .get_mut(&editing_id)
                .expect("editing coaster exists")
                .construction
                .append_resolved(intent, spec, world, &obstacles)
        }
        .map_err(shaped_construction_error_message);
        if let Err(error) = result {
            self.park
                .restore_tunnel_piece_for_coaster(editing_id, first_id);
            return Err(error);
        }
        self.editing_coaster_mut()
            .expect("construction remains active")
            .construction_undo_groups
            .push(1);
        self.selected_shape = None;
        self.selected_element = None;
        self.close_editing_live_coaster();
        self.invalidate_editing_track_state();
        self.select_compatible_piece();
        self.refresh_construction_support_rendering();
        Ok(())
    }

    pub fn construction_append_resolved(&mut self, intent_json: &str, spec_json: &str) -> String {
        let intent = match serde_json::from_str::<ElementIntent>(intent_json) {
            Ok(intent) => intent,
            Err(error) => {
                self.construction_message = format!("Invalid saved shaped intent: {error}");
                return self.construction_message.clone();
            }
        };
        let spec = match serde_json::from_str::<ElementSpec>(spec_json) {
            Ok(spec) => spec,
            Err(error) => {
                self.construction_message = format!("Invalid saved shaped geometry: {error}");
                return self.construction_message.clone();
            }
        };
        self.construction_message = self
            .construction_append_resolved_typed(&intent, &spec)
            .err()
            .unwrap_or_default();
        self.construction_message.clone()
    }

    pub fn construction_undo(&mut self) {
        if self
            .editing_construction()
            .is_none_or(|construction| construction.piece_ids().is_empty())
        {
            self.construction_message = "There is no active track to undo.".into();
            return;
        }
        let mut undo_groups = std::mem::take(
            &mut self
                .editing_coaster_mut()
                .expect("construction remains active")
                .construction_undo_groups,
        );
        let undo_group_size = undo_groups.pop().unwrap_or(1);
        for _ in 0..undo_group_size {
            let Some(id) = self
                .editing_construction()
                .and_then(|construction| construction.piece_ids().last())
                .copied()
            else {
                break;
            };
            self.construction_delete_piece(id);
        }
        self.editing_coaster_mut()
            .expect("construction remains active")
            .construction_undo_groups = undo_groups;
    }

    pub fn construction_delete(&mut self, index: usize) {
        if let Some(coaster) = self.editing_coaster_mut() {
            coaster.construction_undo_groups.clear();
        }
        self.construction_delete_piece(index);
    }

    fn construction_delete_piece(&mut self, index: usize) {
        self.construction_finish_invalidate();
        let result = self
            .editing_construction_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.delete_from(index).map_err(construction_error));
        let changed = result.is_ok();
        if changed {
            let editing_id = self.editing_coaster_id().expect("construction active");
            self.park
                .restore_tunnel_piece_for_coaster(editing_id, index);
            self.close_editing_live_coaster();
            self.clear_editing_block_layout();
            let editing_id = self.editing_coaster_id().expect("construction active");
            self.park.clear_support_render_cache(editing_id);
            let (station, train_configuration) = {
                let construction = self.editing_construction().expect("successful deletion");
                let station = construction
                    .pieces()
                    .iter()
                    .chain(
                        construction
                            .disconnected_segments()
                            .iter()
                            .flat_map(|segment| &segment.pieces),
                    )
                    .find(|piece| piece.kind == PieceKind::Station)
                    .cloned();
                (station, construction.model().train.clone())
            };
            let recovered_station = {
                let coaster = self.editing_coaster_mut().expect("construction active");
                coaster.construction_recorder = None;
                coaster.construction_report = None;
                let recovered_station = if let (Some((circuit, train)), Some(station)) =
                    (&mut coaster.construction_train, station)
                {
                    *train = Train::new(
                        circuit,
                        &train_configuration,
                        PhysicsConfig::default(),
                        station.length(),
                        0.0,
                    );
                    coaster.construction_outcome =
                        Some("Train recovered to the station after track deletion.".into());
                    Some(station)
                } else {
                    coaster.construction_train = None;
                    coaster.construction_outcome = None;
                    None
                };
                coaster.construction_slow_steps = 0;
                recovered_station
            };
            self.recovered_station = recovered_station;
            self.select_compatible_piece();
        }
        self.construction_message = result.err().unwrap_or_default();
        if changed {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_close(&mut self) {
        self.construction_finish_invalidate();
        let result = self
            .editing_construction_mut()
            .ok_or("Construction is inactive".to_string())
            .and_then(|construction| construction.close().map_err(construction_error));
        let changed = result.is_ok();
        if changed {
            self.clear_editing_block_layout();
        }
        self.construction_message = result.err().unwrap_or_default();
        if changed {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_finish_preview_active(&self) -> bool {
        self.construction_completion.is_some()
    }

    pub fn construction_finish_invalidate(&mut self) {
        let had_preview = self.construction_completion.is_some();
        self.construction_completion = None;
        self.construction_completion_available.clear();
        self.construction_completion_index = 0;
        self.construction_completion_search = None;
        self.construction_completion_checked = false;
        self.construction_completion_unavailable_reason = None;
        if had_preview {
            self.refresh_construction_support_rendering();
        }
    }

    pub fn construction_finish_search_start(&mut self) {
        self.construction_finish_search_start_with_judge(
            theme_park_sim::track::construction::FinishForceJudge::Envelope,
        );
    }

    pub fn construction_finish_search_advance(&mut self) -> bool {
        let obstacles = self.facility_obstacles();
        let Some(search) = &mut self.construction_completion_search else {
            return true;
        };
        match search.advance(&self.park.world, &obstacles) {
            CompletionSearchProgress::Pending => false,
            CompletionSearchProgress::Finished(result) => {
                match result {
                    Ok(completions) => self.construction_completion_available = completions,
                    Err(reason) => self.construction_completion_unavailable_reason = Some(reason),
                }
                self.construction_completion_search = None;
                self.construction_completion_checked = true;
                true
            }
        }
    }

    pub fn construction_can_finish(&self) -> bool {
        !self.construction_completion_available.is_empty()
    }

    pub fn construction_finish_reason(&self) -> String {
        if self.construction_can_finish() {
            String::new()
        } else if self.construction_completion_checked {
            self.construction_completion_unavailable_reason
                .clone()
                .expect("a finished search without completions has a reason")
        } else {
            "Checking for a connection to the station.".into()
        }
    }

    /// Ranked finish options with their physics, and the option being previewed.
    pub fn construction_finish_options_json(&self) -> String {
        serde_json::to_string(&self.construction_finish_options())
            .expect("finish options serialize")
    }

    pub fn construction_finish_preview(&mut self) {
        self.construction_completion_index = 0;
        self.construction_completion = self.construction_completion_available.first().cloned();
        self.construction_message = if self.construction_completion.is_some() {
            "Review the connection, then confirm to finish the coaster.".into()
        } else {
            self.construction_finish_reason()
        };
        self.refresh_construction_support_rendering();
    }

    /// Previews the next (`step` 1) or previous (`step` -1) ranked finish option.
    pub fn construction_finish_cycle(&mut self, step: i32) {
        let count = self.construction_completion_available.len();
        if self.construction_completion.is_none() || count == 0 {
            self.construction_message = "Preview a connection first.".into();
            return;
        }
        let index = (self.construction_completion_index as i64 + i64::from(step))
            .rem_euclid(count as i64) as usize;
        self.construction_completion_index = index;
        self.construction_completion = Some(self.construction_completion_available[index].clone());
        self.refresh_construction_support_rendering();
    }

    pub fn construction_finish_cancel(&mut self) {
        self.construction_completion = None;
        self.construction_message.clear();
        self.refresh_construction_support_rendering();
    }

    pub fn construction_finish_confirm(&mut self) {
        let Some(completion) = self.construction_completion.take() else {
            self.construction_message = "Preview a connection first.".into();
            return;
        };
        let Some(editing_id) = self.editing_coaster_id() else {
            self.construction_message = "Construction is inactive".into();
            return;
        };
        let obstacles = self.facility_obstacles();
        let Some(construction) = self.editing_construction() else {
            self.construction_message = "Construction is inactive".into();
            return;
        };
        let train = construction.model().train.clone();
        for piece in &completion.geometry {
            if let Err(error) = self
                .park
                .validate_track_clearance(&train, piece, Some(editing_id))
            {
                self.construction_message = construction_error(error);
                return;
            }
        }
        let first_id = construction.next_piece_id();
        let tunnel_plans =
            match construction.tunnel_plans_for_geometry(&completion.geometry, &self.park.world) {
                Ok(plans) => plans,
                Err(error) => {
                    self.construction_message = construction_error(error);
                    self.refresh_construction_support_rendering();
                    return;
                }
            };
        let piece_ids: Vec<_> = (0..tunnel_plans.len())
            .map(|offset| first_id + offset)
            .collect();
        let ownership: Vec<_> = piece_ids.iter().copied().zip(tunnel_plans).collect();
        let result = self.park.carve_for_coaster(editing_id, &ownership);
        let message = match result {
            Err(error) => error.to_string(),
            Ok(()) => {
                let apply_result = {
                    let park = &mut self.park;
                    let (world, coasters) = (&park.world, &mut park.coasters);
                    coasters
                        .get_mut(&editing_id)
                        .expect("editing coaster exists")
                        .construction
                        .apply_completion(&completion, world, &obstacles)
                };
                match apply_result {
                    Ok(()) => String::new(),
                    Err(error) => {
                        for piece_id in &piece_ids {
                            self.park
                                .restore_tunnel_piece_for_coaster(editing_id, *piece_id);
                        }
                        construction_error(error)
                    }
                }
            }
        };
        if message.is_empty() {
            // The circuit is closed now; this records that without searching.
            self.construction_finish_search_start();
            self.close_editing_live_coaster();
            self.invalidate_editing_track_state();
            self.select_compatible_piece();
        }
        self.construction_message = message;
        self.refresh_construction_support_rendering();
    }

    pub fn construction_test(&mut self) {
        if self
            .editing_coaster()
            .is_some_and(|coaster| coaster.coaster_open)
        {
            self.construction_message = "Close the coaster before testing it.".into();
            return;
        }
        if !self.construction_can_test() {
            self.construction_message = "A closed circuit longer than the train is required".into();
            return;
        }
        let construction = self.editing_construction().expect("checked above");
        let circuit = construction.circuit().expect("checked above");
        let train = Train::new(
            &circuit,
            &construction.model().train,
            PhysicsConfig {
                lift_speed: 5.0,
                ..PhysicsConfig::default()
            },
            0.0,
            2.0,
        );
        let recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_model(
            &circuit,
            &construction.model().train,
            construction.model(),
        );
        let coaster = self
            .editing_coaster_mut()
            .expect("construction remains active");
        coaster.construction_recorder = Some(recorder);
        coaster.construction_report = None;
        coaster.construction_train = Some((circuit, train));
        coaster.construction_slow_steps = 0;
        coaster.construction_outcome = None;
        coaster.construction_message = "Test train running.".into();
        self.recovered_station = None;
        self.park.construction_message.clear();
    }

    /// Test report and model facts for one coaster, addressed by its stable park id.
    pub fn coaster_info_json(&self, id: u32) -> String {
        let coaster = self.park.coaster(CoasterId(id)).expect("coaster id exists");
        let model = coaster.construction.model();
        let report = coaster
            .construction_report
            .as_ref()
            .zip(coaster.construction.circuit().ok())
            .map(|(report, circuit)| testing_report(report, &circuit));
        serde_json::to_string(&CoasterRideInfo {
            id,
            name: &coaster.name,
            model: model.key,
            model_name: model.name.to_owned(),
            manufacturer: model.manufacturer.to_owned(),
            limits: force_limits_wire(model.design_force_targets, model.name),
            report,
        })
        .expect("serialize coaster ride info")
    }

    /// Rates one coaster immediately and stores its report.
    pub fn coaster_test(&mut self, id: u32) -> String {
        if self.park.coaster(CoasterId(id)).is_none() {
            return "No coaster has that id.".into();
        }
        self.park
            .run_ride_test(CoasterId(id))
            .err()
            .unwrap_or_default()
    }

    /// Test report and model facts for one coaster, addressed by its stable park id.
    pub fn coaster_ride_info_json(&self, id: u32) -> String {
        let coaster = self.park.coaster(CoasterId(id)).expect("coaster id exists");
        let construction = &coaster.construction;
        let report = coaster
            .construction_report
            .as_ref()
            .zip(construction.circuit().ok())
            .map(|(report, circuit)| testing_report(report, &circuit));
        let model = construction.model();
        serde_json::to_string(&crate::reports::RideInfoReport {
            ride: id,
            model_name: model.name.to_owned(),
            manufacturer: model.manufacturer.to_owned(),
            limits: force_limits_wire(model.design_force_targets, model.name),
            report,
        })
        .expect("serialize ride info")
    }

    pub fn construction_test_report_json(&self) -> String {
        let Some(coaster) = self.editing_coaster() else {
            return "null".into();
        };
        let (Some(report), Some((circuit, _))) =
            (&coaster.construction_report, &coaster.construction_train)
        else {
            return "null".into();
        };
        testing_report_json(report, circuit)
    }

    pub fn construction_rails(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_rails(&self.construction_preview(), &self.world),
            layouts::RAIL.len(),
        )
    }

    pub fn construction_ties(&self) -> Vec<f32> {
        self.encode_construction_geometry(
            render_ties(&self.construction_preview(), &self.world),
            layouts::TIE.len(),
        )
    }

    pub fn construction_supports(&self) -> Vec<f32> {
        self.construction_preview_supports_cache.clone()
    }

    pub fn construction_placed_rails(&self) -> Vec<f32> {
        self.construction_render(render_rails, layouts::RAIL.len())
    }

    pub fn construction_rail_distances(&self) -> Vec<f32> {
        let Some(construction) = self.editing_construction() else {
            return Vec::new();
        };
        let pieces = construction.pieces().iter().chain(
            construction
                .disconnected_segments()
                .iter()
                .flat_map(|segment| &segment.pieces),
        );
        let mut distances = Vec::new();
        let mut piece_start = 0.0;
        for piece in pieces {
            let length = piece.length();
            let intervals = (length / 2.0).ceil() as usize;
            for index in 0..intervals {
                distances.push((piece_start + length * index as f64 / intervals as f64) as f32);
                distances
                    .push((piece_start + length * (index + 1) as f64 / intervals as f64) as f32);
            }
            piece_start += length;
        }
        distances
    }

    pub fn construction_placed_ties(&self) -> Vec<f32> {
        self.construction_render(render_ties, layouts::TIE.len())
    }

    pub fn construction_placed_supports(&self) -> Vec<f32> {
        self.construction_placed_supports_cache.clone()
    }

    pub fn construction_train_states(&self) -> Vec<f32> {
        let mut result = Vec::new();
        let Some(coaster) = self.editing_coaster() else {
            return result;
        };
        if let Some(operation) = &coaster.coaster_operation {
            let circuit = coaster
                .block_layout
                .as_ref()
                .expect("operation has a block layout")
                .circuit();
            let mut trains: Vec<_> = operation.trains().collect();
            trains.sort_by_key(|(train_id, _)| *train_id);
            for (train_id, train) in trains {
                let occupied = coaster
                    .coaster_riders
                    .get(&train_id)
                    .is_some_and(|riders| !riders.is_empty());
                append_train_state(&mut result, circuit, train, train_id, occupied);
            }
        } else if let Some((circuit, train)) = &coaster.construction_train {
            append_train_state(&mut result, circuit, train, 0, false);
            if let Some(station) = &self.recovered_station {
                let sample = station.sample(station.length());
                result[2] = 4.0;
                result[3] = sample.up.z as f32;
                result[4] = sample.up.cross(sample.tangent).z as f32;
            }
        }
        result
    }

    pub fn construction_train_poses(&self) -> Vec<f32> {
        let coaster = self.editing_coaster().expect("train requires construction");
        let config = &self
            .editing_construction()
            .expect("train requires construction")
            .model()
            .train;
        let mut result = Vec::new();
        if let Some(operation) = &coaster.coaster_operation {
            let circuit = coaster
                .block_layout
                .as_ref()
                .expect("operation has a block layout")
                .circuit();
            let mut trains: Vec<_> = operation.trains().collect();
            trains.sort_by_key(|(train_id, _)| *train_id);
            for (train_id, train) in trains {
                let riders = coaster
                    .coaster_riders
                    .get(&train_id)
                    .map_or(&[][..], Vec::as_slice);
                append_train_poses(&mut result, circuit, train, config, train_id, riders, None);
            }
            return result;
        }

        let Some((circuit, train)) = &coaster.construction_train else {
            return result;
        };
        append_train_poses(
            &mut result,
            circuit,
            train,
            config,
            0,
            &[],
            self.recovered_station.as_ref(),
        );
        result
    }
}

impl WasmSimulation {
    fn construction_support_entries(&self) -> Option<Vec<(&usize, &Piece)>> {
        let construction = self.editing_construction()?;
        Some(
            construction
                .piece_ids()
                .iter()
                .zip(construction.pieces())
                .chain(
                    construction
                        .disconnected_segments()
                        .iter()
                        .flat_map(|segment| segment.ids.iter().zip(&segment.pieces)),
                )
                .collect(),
        )
    }

    fn construction_preview_supports_geometry(&self) -> Result<Vec<f32>, SupportError> {
        if !self.construction_preview_valid() {
            return Ok(Vec::new());
        }
        let construction = self
            .editing_construction()
            .expect("valid preview requires construction");
        let preview = self.construction_preview();
        if preview.is_empty() {
            return Ok(Vec::new());
        }
        let tunnel_plans = construction
            .tunnel_plans_for_geometry(&preview, &self.world)
            .map_err(|_| SupportError::NoRoute)?;
        let carve_cells: Vec<_> = tunnel_plans
            .into_iter()
            .flat_map(|plan| {
                plan.carve_cells
                    .into_iter()
                    .map(|cell| (cell.x, cell.y, cell.height))
            })
            .collect();
        let carved_world;
        let preview_world = if carve_cells.is_empty() {
            &self.park.world
        } else {
            (carved_world, _) = self
                .park
                .world
                .with_cells_carved(&carve_cells)
                .map_err(|_| SupportError::NoRoute)?;
            &carved_world
        };
        let mut obstacles: Vec<_> = self
            .construction_support_entries()
            .expect("valid preview requires construction")
            .into_iter()
            .flat_map(|(_, placed_piece)| {
                track_support_obstacles(
                    std::slice::from_ref(placed_piece),
                    &construction.model().train,
                )
            })
            .collect();
        obstacles.extend(self.facility_obstacles());
        let mut supports = Vec::new();
        for piece in &preview {
            supports.extend(render_preview_supports(
                piece,
                &obstacles,
                preview_world,
                construction.model().support_style,
            )?);
            obstacles.extend(track_support_obstacles(
                std::slice::from_ref(piece),
                &construction.model().train,
            ));
        }
        Ok(self.encode_construction_geometry(supports, 7))
    }

    pub(crate) fn refresh_construction_support_rendering(&mut self) {
        let placed_supports = self.editing_coaster_id().map_or_else(
            || Ok(Vec::new()),
            |id| self.park.refresh_construction_supports(id),
        );
        let preview_supports = self.construction_preview_supports_geometry();
        let placed_supports_failed = placed_supports.is_err();
        let preview_supports_failed = preview_supports.is_err();

        self.construction_placed_supports_cache = placed_supports
            .map(|supports| self.encode_construction_geometry(supports, 7))
            .unwrap_or_default();
        self.construction_preview_supports_cache = preview_supports.unwrap_or_default();

        if placed_supports_failed {
            self.park.construction_message =
                "Could not route supports for the placed track.".into();
        } else if preview_supports_failed {
            self.park.construction_message = "Could not route preview supports.".into();
        } else if matches!(
            self.park.construction_message.as_str(),
            "Could not route supports for the placed track." | "Could not route preview supports."
        ) {
            self.park.construction_message.clear();
        }
    }
}

macro_rules! define_piece_id_wire_names {
    ($($variant:ident),+ $(,)?) => {
        pub(crate) fn wire_pieceid(value: PieceId) -> &'static str {
            match value {
                $(PieceId::$variant => stringify!($variant),)+
            }
        }

        pub(crate) fn pieceid_from_wire(value: &str) -> Option<PieceId> {
            match value {
                $(stringify!($variant) => Some(PieceId::$variant),)+
                _ => None,
            }
        }
    };
}

define_piece_id_wire_names!(
    Straight,
    GentleUp,
    GentleDown,
    SteepUp,
    SteepDown,
    SteepToSixtyUp,
    SixtyUp,
    SixtyToSteepUp,
    SteepToSixtyDown,
    SixtyDown,
    SixtyToSteepDown,
    LevelToGentleUp,
    GentleUpToLevel,
    GentleToSteepUp,
    SteepToGentleUp,
    LevelToGentleDown,
    GentleDownToLevel,
    LongGentleDownToLevel,
    GentleToSteepDown,
    SteepToGentleDown,
    TurnSmallEntry,
    TurnSmall,
    TurnSmallExit,
    TurnLargeEntry,
    TurnLarge,
    TurnLargeExit,
    BankedTurnEntry,
    BankedTurnExit,
    TurnSmallStandalone,
    TurnLargeStandalone,
    BankedTurnStandalone,
    LargeLeftBankedTurnStandalone,
    SmallRightBankedTurnStandalone,
    LargeRightBankedTurnStandalone,
    LargeLeftBankedTurnEntry,
    LargeLeftBankedTurn,
    LargeLeftBankedTurnExit,
    SmallRightBankedTurnEntry,
    SmallRightBankedTurn,
    SmallRightBankedTurnExit,
    LargeRightBankedTurnEntry,
    LargeRightBankedTurn,
    LargeRightBankedTurnExit,
    VerticalUpEntry,
    VerticalUpContinue,
    VerticalUpExit,
    VerticalDownEntry,
    VerticalDownContinue,
    VerticalDownExit,
    BankLeft,
    UnbankLeft,
    BankRight,
    UnbankRight,
    BankedStraightLeft,
    BankedStraightRight,
    RolloutLeft,
    RolloutRight,
    BankedTurn,
    OffsetLeft,
    OffsetRight,
    ShortOffsetLeft,
    ShortOffsetRight,
    Loop,
    Corkscrew,
    HeartlineRoll,
    Brakes,
    Station,
    TurnSmallRightEntry,
    TurnSmallRight,
    TurnSmallRightExit,
    TurnSmallRightStandalone,
    TurnLargeRightEntry,
    TurnLargeRight,
    TurnLargeRightExit,
    TurnLargeRightStandalone,
    CorkscrewRight,
    HeartlineRollRight,
    LaunchShort,
    Launch,
    LaunchLong,
    TerrainRise,
    TerrainFall,
    SteelLoop,
    TwisterFanLeft,
    TwisterFanRight,
    TwisterHelixLeft,
    TwisterHelixRight,
    AirtimeHill,
    TightTwisterLeft,
    TightTwisterRight,
    HybridOverbankLeft,
    HybridOverbankRight,
    WaveTurnLeft,
    WaveTurnRight,
    SpeedTurnLarge16Left,
    SpeedTurnLarge16Right,
    SpeedTurnLarge20Left,
    SpeedTurnLarge20Right,
    SpeedTurnLarge24Left,
    SpeedTurnLarge24Right,
    SpeedTurnLarge28Left,
    SpeedTurnLarge28Right,
    SpeedTurnLarge32Left,
    SpeedTurnLarge32Right,
    SpeedTurnSmall12Left,
    SpeedTurnSmall12Right,
    SpeedTurnSmall16Left,
    SpeedTurnSmall16Right,
    SpeedTurnSmall20Left,
    SpeedTurnSmall20Right,
    SpeedTurnSmall24Left,
    SpeedTurnSmall24Right,
    HalfTileStraight,
    FullOffsetLeft,
    FullOffsetRight,
    OneTileStraight,
    OneTileGentleUp,
    OneTileGentleDown,
    ShortRise,
    ShortFall,
    LoopRight,
    SteelLoopRight,
    HalfLoopUp,
    HalfLoopDown,
    HalfCorkscrewLeftUp,
    HalfCorkscrewRightUp,
    HalfCorkscrewLeftDown,
    HalfCorkscrewRightDown,
    TopHat,
    LaunchedTopHat,
    LevelToSixtyUp,
    SixtyUpToLevel,
    LevelToSixtyDown,
    SixtyDownToLevel,
    BankedDropLeft,
    BankedDropRight,
    TwistedDropLeft,
    TwistedDropRight,
    BarrelDropLeft,
    BarrelDropRight,
    RavenRollLeftUp,
    RavenRollRightUp,
    RavenRollLeftDown,
    RavenRollRightDown,
    LargeLoop,
    LargeLoopRight,
    EjectorHill8x48,
    EjectorHill12x64,
    EjectorHill8x64,
    EjectorHill12x80,
    EjectorHill16x96,
    FloaterHill4x48,
    FloaterHill8x64,
    FloaterHill12x80,
    FloaterHill6x64,
    FloaterHill12x96,
    FloaterHill16x112,
    SpeedHill4x64,
    SpeedHill6x80,
    WoodenEjectorHill6x48,
    WoodenEjectorHill8x64,
    WoodenEjectorHill16x80,
    EjectorHill8x48OuterLeft,
    EjectorHill8x48OuterRight,
    EjectorHill12x64OuterLeft,
    EjectorHill12x64OuterRight,
    EjectorHill8x64OuterLeft,
    EjectorHill8x64OuterRight,
    EjectorHill12x80OuterLeft,
    EjectorHill12x80OuterRight,
    EjectorHill16x96OuterLeft,
    EjectorHill16x96OuterRight,
    FloaterHill4x48OuterLeft,
    FloaterHill4x48OuterRight,
    FloaterHill8x64OuterLeft,
    FloaterHill8x64OuterRight,
    FloaterHill12x80OuterLeft,
    FloaterHill12x80OuterRight,
    FloaterHill6x64OuterLeft,
    FloaterHill6x64OuterRight,
    FloaterHill12x96OuterLeft,
    FloaterHill12x96OuterRight,
    FloaterHill16x112OuterLeft,
    FloaterHill16x112OuterRight,
    ZeroGRollFastLeft,
    ZeroGRollFastRight,
    ZeroGRollCompactLeft,
    ZeroGRollCompactRight,
    StallLeft,
    StallRight,
    FamilyHill3x32,
    FamilyHill4x40,
    FloaterHill20x128,
    FloaterHill24x160,
    WideLevelToSixtyDown,
    WideSixtyDownToLevel,
    GigaSixtyDownToLevel,
    ShapedHill,
    ShapedTurnLeft,
    ShapedTurnRight,
    ShapedLoopLeft,
    ShapedLoopRight,
    ShapedZeroGRollLeft,
    ShapedZeroGRollRight,
    ShapedRavenTurnLeft,
    ShapedRavenTurnRight,
);

fn wire_heading(value: Heading) -> &'static str {
    match value {
        Heading::East => "East",
        Heading::NorthEast => "NorthEast",
        Heading::North => "North",
        Heading::NorthWest => "NorthWest",
        Heading::West => "West",
        Heading::SouthWest => "SouthWest",
        Heading::South => "South",
        Heading::SouthEast => "SouthEast",
    }
}

pub(crate) fn wire_slope(value: Slope) -> &'static str {
    match value {
        Slope::DownSteep => "DownSteep",
        Slope::DownSixty => "DownSixty",
        Slope::DownModerate => "DownModerate",
        Slope::DownGentle => "DownGentle",
        Slope::Level => "Level",
        Slope::UpGentle => "UpGentle",
        Slope::UpModerate => "UpModerate",
        Slope::UpSteep => "UpSteep",
        Slope::UpSixty => "UpSixty",
    }
}

pub(crate) fn wire_bank(value: Bank) -> &'static str {
    match value {
        Bank::Right => "Right",
        Bank::Level => "Level",
        Bank::Left => "Left",
        Bank::Inverted => "Inverted",
    }
}

pub(crate) fn wire_piecekind(value: PieceKind) -> &'static str {
    match value {
        PieceKind::Track => "Track",
        PieceKind::Lift => "Lift",
        PieceKind::Launch => "Launch",
        PieceKind::Brakes => "Brakes",
        PieceKind::Station => "Station",
    }
}

#[cfg(test)]
mod force_wire_tests {
    use super::{ForcePeakValues, last_placed_force_wire};

    fn sample(vertical_g: f64) -> crate::reports::TestingSample {
        crate::reports::TestingSample {
            distance: 4.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            speed: 12.0,
            vertical_g,
            lateral_g: 0.0,
            longitudinal_g: 0.0,
            airtime: vertical_g < 0.0,
            force_limit_exceeded: [true, false, false],
        }
    }

    fn peaks(vertical_g: f64) -> ForcePeakValues {
        ForcePeakValues {
            minimum_vertical_g: vertical_g.min(0.0),
            maximum_vertical_g: vertical_g.max(0.0),
            maximum_lateral_g: 0.0,
            maximum_longitudinal_g: 0.0,
        }
    }

    #[test]
    fn vertical_limit_flags_keep_minimum_and_maximum_directions_separate() {
        let negative = last_placed_force_wire(&[sample(-1.2)], 0.0, 8.0, Some(peaks(-1.2)))
            .expect("sample lies on selected piece");
        let positive = last_placed_force_wire(&[sample(3.2)], 0.0, 8.0, Some(peaks(3.2)))
            .expect("sample lies on selected piece");

        assert!(negative.minimum_vertical_limit_exceeded);
        assert!(!negative.maximum_vertical_limit_exceeded);
        assert!(!positive.minimum_vertical_limit_exceeded);
        assert!(positive.maximum_vertical_limit_exceeded);
    }

    #[test]
    #[should_panic(expected = "nonempty force envelope must have per-piece peak measurements")]
    fn nonempty_force_envelope_requires_piece_peak_measurements() {
        let _ = last_placed_force_wire(&[sample(-1.2)], 0.0, 8.0, None);
    }
}

#[cfg(test)]
mod getter_tests {
    use super::WasmSimulation;

    #[test]
    fn support_render_getters_do_not_mutate_park_state() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        simulation.construction_select("Straight");
        let message_before = simulation.park.construction_message.clone();
        let preview_supports_before = simulation.construction_preview_supports_cache.clone();
        let placed_supports_before = simulation.construction_placed_supports_cache.clone();
        let cache_entry_count = simulation
            .park
            .support_render_cache_len(simulation.editing_coaster_id().unwrap());

        assert!(!simulation.construction_supports().is_empty());
        assert!(!simulation.construction_placed_supports().is_empty());

        assert_eq!(simulation.park.construction_message, message_before);
        assert_eq!(
            simulation.construction_preview_supports_cache,
            preview_supports_before
        );
        assert_eq!(
            simulation.construction_placed_supports_cache,
            placed_supports_before
        );
        assert_eq!(
            simulation
                .park
                .support_render_cache_len(simulation.editing_coaster_id().unwrap()),
            cache_entry_count
        );
    }

    #[test]
    fn support_render_errors_are_derived_without_mutating_park_state() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        for y in 95..105 {
            for x in 95..110 {
                simulation.set_world_surface(x, y, vec![100; 4], None);
            }
        }
        let message_before = simulation.park.construction_message.clone();
        let placed_supports_before = simulation.construction_placed_supports_cache.clone();
        let cache_entry_count = simulation
            .park
            .support_render_cache_len(simulation.editing_coaster_id().unwrap());

        assert!(simulation.construction_placed_supports().is_empty());
        assert_eq!(
            simulation.construction_message(),
            "Could not route supports for the placed track."
        );

        assert_eq!(simulation.park.construction_message, message_before);
        assert_eq!(
            simulation.construction_placed_supports_cache,
            placed_supports_before
        );
        assert_eq!(
            simulation
                .park
                .support_render_cache_len(simulation.editing_coaster_id().unwrap()),
            cache_entry_count
        );
    }

    #[test]
    fn newer_construction_command_error_overrides_cached_support_error() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("demo"));
        simulation.construction_start(200, 200, 20, 0);
        for y in 95..105 {
            for x in 95..110 {
                simulation.set_world_surface(x, y, vec![100; 4], None);
            }
        }
        assert_eq!(
            simulation.construction_message(),
            "Could not route supports for the placed track."
        );

        simulation.construction_select("unknown-piece");

        assert_eq!(simulation.construction_message(), "Unknown piece");
    }
}

#[cfg(test)]
mod shaped_candidate_mode_tests {
    use super::{CoasterId, WasmSimulation};
    use crate::builder_session::BuilderSession;
    use theme_park_sim::physics::PhysicsConfig;
    use theme_park_sim::track::construction::Construction;
    use theme_park_sim::track::ride_file::{RideFile, RideStep};
    use theme_park_sim::track::shaped::{ElementIntent, ElementSpec, HillSizeHint};
    use theme_park_sim::track::{Heading, HillFeel, PieceId, TrackState, hyper_model};

    fn hyper_hill_simulation(cars: u8, speed_mps: f64) -> WasmSimulation {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        let mut model = hyper_model();
        model.train.length_metres *= f64::from(cars) / f64::from(model.train.cars);
        model.train.cars = cars;
        let mut construction = Construction::new(
            TrackState {
                x: 200,
                y: 200,
                z: 20,
                heading: Heading::East,
                ..TrackState::origin()
            },
            model,
        );
        let physics = PhysicsConfig {
            lift_speed: speed_mps,
            ..PhysicsConfig::default()
        };
        let straight_length_m = theme_park_sim::track::catalog()
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .expect("catalog includes straight track")
            .length();
        let lift_length_m = (theme_park_sim::math::powi(physics.lift_speed, 2)
            - theme_park_sim::math::powi(physics.station_speed, 2))
            / (2.0 * physics.drive_acceleration)
            + construction.model().train.length_metres
            + 40.0;
        let lift_piece_count = (lift_length_m / straight_length_m).ceil() as usize;
        let world = &simulation.park.world;
        construction
            .append(PieceId::Station, world, &[])
            .expect("hyper construction accepts a station");
        construction.set_chain(true);
        for _ in 0..lift_piece_count {
            construction
                .append(PieceId::Straight, world, &[])
                .expect("hyper construction accepts chain lift track");
        }
        construction.set_chain(false);
        let coaster_id = CoasterId(0);
        simulation.park.replace_coaster(coaster_id, construction);
        simulation.editing = Some(coaster_id);
        simulation
    }

    #[test]
    fn browse_uses_one_preview_and_confirmation_does_not_reuse_its_inaccurate_result() {
        let simulation = hyper_hill_simulation(4, 34.0);
        let physics = PhysicsConfig {
            lift_speed: 34.0,
            ..PhysicsConfig::default()
        };

        let intent = ElementIntent::Hill {
            feel: HillFeel::StrongFloater,
            strength_g: -0.85,
            size: HillSizeHint::Auto,
        };
        let prefix_piece_count = simulation
            .editing_construction()
            .expect("test construction exists")
            .pieces()
            .len();

        let browse = simulation
            .construction_shape_candidate_at_speed(&intent, 34.0, physics, true)
            .expect("browse produces a shaped candidate");
        assert_eq!(browse.calibration_runs, 1);
        assert_eq!(
            simulation
                .editing_construction()
                .expect("test construction exists")
                .pieces()
                .len(),
            prefix_piece_count
        );
        let browse_spec = browse.spec.clone();
        let ElementSpec::Hill {
            target_g,
            delivered_g: browse_delivery_g,
            ..
        } = browse.spec
        else {
            panic!("hill intent resolves to a hill");
        };
        let browse_geometry_digest = browse_spec.resolved_geometry_digest();
        assert!(
            (browse_delivery_g - target_g).abs() > 0.1,
            "one-preview browse delivered {browse_delivery_g:.6} g for {target_g:.6} g target"
        );

        let confirmed = simulation
            .construction_shape_candidate_at_speed(&intent, 34.0, physics, false)
            .expect("confirmation calibrates the browsed hill");
        assert!((2..=4).contains(&confirmed.calibration_runs));
        assert_ne!(
            confirmed.spec.resolved_geometry_digest(),
            browse_geometry_digest,
            "confirmation should resolve the provisional geometry again"
        );
        let ElementSpec::Hill {
            target_g,
            delivered_g: confirmed_delivery_g,
            ..
        } = confirmed.spec
        else {
            panic!("hill intent resolves to a hill");
        };
        assert!((confirmed_delivery_g - target_g).abs() <= 0.1);

        let rebrowsed = simulation
            .construction_shape_candidate_at_speed(&intent, 34.0, physics, true)
            .expect("browse recomputes a one-preview candidate after confirmation");
        assert_eq!(rebrowsed.calibration_runs, 1);
        assert_eq!(
            rebrowsed.spec.resolved_geometry_digest(),
            browse_geometry_digest,
            "browsing after confirmation must return the one-preview candidate"
        );
        let ElementSpec::Hill {
            target_g,
            delivered_g: rebrowsed_delivery_g,
            ..
        } = rebrowsed.spec
        else {
            panic!("hill intent resolves to a hill");
        };
        assert!((rebrowsed_delivery_g - target_g).abs() > 0.1);
        assert_eq!(
            simulation
                .editing_construction()
                .expect("test construction exists")
                .pieces()
                .len(),
            prefix_piece_count
        );
    }

    #[test]
    fn horizon_hawk_first_shaped_turn_uses_confirmed_geometry_for_validation() {
        let mut ride: RideFile = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .expect("Horizon Hawk ride file parses");
        let first_shape_index = ride
            .steps
            .iter()
            .position(|step| matches!(step, RideStep::Shape(_)))
            .expect("Horizon Hawk has a shaped element");
        let RideStep::Shape(first_shape) = ride.steps[first_shape_index].clone() else {
            unreachable!("first shaped step was located above");
        };
        ride.steps.truncate(first_shape_index);

        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        assert!(simulation.set_world_surface(241, 249, vec![6; 4], None));
        let mut session = BuilderSession::new(simulation);
        session
            .replay_ride(&ride)
            .expect("Horizon Hawk prefix replays before its first shaped turn");
        session
            .select("shape:turn:left:fullyBanked:fixed-68", true)
            .expect("Horizon Hawk first turn is offered");
        let simulation = session.simulation();
        let construction = simulation
            .editing_construction()
            .expect("ride prefix leaves construction active");
        let preview = simulation
            .construction_shape_candidate_with_mode(&first_shape.intent, true)
            .expect("provisional shape solves");
        let confirmed = simulation
            .construction_shape_candidate_for_confirm(&first_shape.intent)
            .expect("confirmed shape solves");
        let obstacles = simulation.facility_obstacles();
        let preview_validation =
            construction.validate_shaped_piece(&preview.piece, &simulation.world, &obstacles);
        let confirmed_validation =
            construction.validate_shaped_piece(&confirmed.piece, &simulation.world, &obstacles);

        let ElementSpec::Turn {
            radius_m: preview_radius_m,
            bank_degrees: preview_bank_degrees,
            ..
        } = preview.spec
        else {
            panic!("first Horizon Hawk shape is a turn");
        };
        let ElementSpec::Turn {
            radius_m: confirmed_radius_m,
            bank_degrees: confirmed_bank_degrees,
            ..
        } = confirmed.spec
        else {
            panic!("first Horizon Hawk shape is a turn");
        };

        assert_eq!(preview_radius_m, 68.0);
        assert_eq!(confirmed_radius_m, 68.0);
        assert!((preview_bank_degrees - 69.83).abs() < 0.02);
        assert!((confirmed_bank_degrees - 71.532594).abs() < 0.02);
        assert!(
            preview_validation.is_err(),
            "provisional geometry should reproduce the disabled Place state"
        );
        assert!(
            confirmed_validation.is_ok(),
            "confirmed geometry should be placeable: {confirmed_validation:?}"
        );
        assert!(
            simulation.construction_can_append(),
            "Place should use the confirmed geometry shown to the player: {}",
            simulation.construction_preview_reason()
        );
        assert!(simulation.construction_preview_reason().is_empty());
        let selected_shape = simulation
            .construction_selected_shape_wire(true)
            .expect("builder state includes the selected shaped turn");
        let selected_bank_degrees = selected_shape
            .spec
            .expect("selected shape has a resolved specification")
            .bank_degrees
            .expect("selected turn reports its bank");
        assert!((selected_bank_degrees - confirmed_bank_degrees).abs() < 0.001);
    }
}
