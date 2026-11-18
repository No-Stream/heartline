use crate::track::catalog::catalog;
use crate::track::{
    Bank, CoasterModel, HorizontalCurvature, LaunchType, PieceId, Slope, TrackState,
    VerticalCurvature, coaster_models, hybrid_model, multi_launch_model, wooden_twister_model,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectorState {
    slope: Slope,
    bank: Bank,
    horizontal_curvature: HorizontalCurvature,
    vertical_curvature: VerticalCurvature,
}

impl From<TrackState> for ConnectorState {
    fn from(state: TrackState) -> Self {
        Self {
            slope: state.slope,
            bank: state.bank,
            horizontal_curvature: state.horizontal_curvature,
            vertical_curvature: state.vertical_curvature,
        }
    }
}

impl ConnectorState {
    fn track_state(self) -> TrackState {
        TrackState {
            slope: self.slope,
            bank: self.bank,
            horizontal_curvature: self.horizontal_curvature,
            vertical_curvature: self.vertical_curvature,
            ..TrackState::origin()
        }
    }
}

fn reachable_connectors(
    model: &CoasterModel,
    start: ConnectorState,
    chain_only: bool,
) -> Vec<ConnectorState> {
    let mut reachable = vec![start];
    let mut next_index = 0;
    while next_index < reachable.len() {
        let connector = reachable[next_index];
        next_index += 1;
        for piece in model.available_pieces(connector.track_state()) {
            if chain_only && !model.allows_chain(&piece) {
                continue;
            }
            let next = ConnectorState::from(piece.exit);
            if !reachable.contains(&next) {
                reachable.push(next);
            }
        }
    }
    reachable
}

struct CapabilityExpectations {
    model: CoasterModel,
    turns_on_gentle_slopes: bool,
    curved_chain: bool,
    maximum_lift_degrees: f64,
    launch_types: Vec<LaunchType>,
}

fn capability_expectations() -> Vec<CapabilityExpectations> {
    vec![
        CapabilityExpectations {
            model: hybrid_model(),
            turns_on_gentle_slopes: true,
            curved_chain: true,
            maximum_lift_degrees: 60.0,
            launch_types: vec![LaunchType::LinearMotor],
        },
        CapabilityExpectations {
            model: multi_launch_model(),
            turns_on_gentle_slopes: true,
            curved_chain: true,
            maximum_lift_degrees: 60.0,
            launch_types: vec![LaunchType::LinearMotor],
        },
        CapabilityExpectations {
            model: wooden_twister_model(),
            turns_on_gentle_slopes: true,
            curved_chain: true,
            maximum_lift_degrees: 45.0,
            launch_types: vec![],
        },
    ]
}

fn slope_degrees(state: TrackState) -> f64 {
    let tangent = state.tangent();
    tangent.z.atan2(tangent.x.hypot(tangent.y)).to_degrees()
}

#[test]
fn every_model_piece_variant_has_a_reachable_entry_connector() {
    let mut failures = Vec::new();
    for expectations in capability_expectations() {
        let model = expectations.model;
        let station = model
            .available_pieces(TrackState::origin())
            .into_iter()
            .find(|piece| piece.id == PieceId::Station)
            .expect("model must offer a station");
        let reachable = reachable_connectors(&model, ConnectorState::from(station.exit), false);
        for piece in catalog().iter().filter(|piece| model.allows_piece(piece)) {
            let entry = ConnectorState::from(piece.entry);
            if !reachable.contains(&entry) {
                failures.push(format!(
                    "{}: {:?} ({}) at {entry:?}",
                    model.name, piece.id, piece.name
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "unreachable piece variants:\n{}",
        failures.join("\n")
    );
}

#[test]
fn launch_capabilities_match_reachable_launch_sections() {
    let level = ConnectorState::from(TrackState::origin());
    for expectations in capability_expectations() {
        let model = expectations.model;
        assert_eq!(
            model.launch_types, expectations.launch_types,
            "{}",
            model.name
        );
        let launch_ids = [PieceId::LaunchShort, PieceId::Launch, PieceId::LaunchLong];
        if expectations.launch_types.is_empty() {
            assert!(
                model.train.launch.is_none(),
                "{} has unused launch parameters",
                model.name
            );
            for id in launch_ids {
                assert!(
                    !model.allowed_pieces.contains(&id),
                    "{} allows {id:?}",
                    model.name
                );
            }
            continue;
        }

        let launch = model
            .train
            .launch
            .expect("launch capability requires train parameters");
        assert!(launch.target_speed.is_finite() && launch.target_speed > 0.0);
        assert!(launch.acceleration.is_finite() && launch.acceleration > 0.0);
        let connectors = reachable_connectors(&model, level, false);
        for id in launch_ids {
            assert!(
                model.allowed_pieces.contains(&id),
                "{} does not allow {id:?}",
                model.name
            );
            assert!(
                connectors.iter().any(|connector| {
                    model
                        .available_pieces(connector.track_state())
                        .iter()
                        .any(|piece| piece.id == id)
                }),
                "{} cannot reach {id:?}",
                model.name
            );
        }
    }
}

#[test]
fn connector_exhaustion_has_continuations_can_level_and_crest() {
    let level = ConnectorState::from(TrackState::origin());
    for expectations in capability_expectations() {
        let model = expectations.model;
        let station = model
            .available_pieces(TrackState::origin())
            .into_iter()
            .find(|piece| piece.id == PieceId::Station)
            .unwrap_or_else(|| panic!("{} does not offer a level station", model.name));
        assert_eq!(ConnectorState::from(station.exit), level);
        let mut connectors = vec![level];
        let mut edges: Vec<Vec<usize>> = Vec::new();
        while edges.len() < connectors.len() {
            let connector = connectors[edges.len()];
            let pieces = model.available_pieces(connector.track_state());
            assert!(
                !pieces.is_empty(),
                "{} has no continuation from {connector:?}",
                model.name
            );
            let mut exits = Vec::new();
            for piece in pieces {
                let next = ConnectorState::from(piece.exit);
                let index = match connectors.iter().position(|state| *state == next) {
                    Some(index) => index,
                    None => {
                        connectors.push(next);
                        connectors.len() - 1
                    }
                };
                exits.push(index);
            }
            edges.push(exits);
        }
        let mut reaches_level = vec![false; connectors.len()];
        reaches_level[0] = true;
        let mut reaches_descent: Vec<bool> = connectors
            .iter()
            .map(|connector| slope_degrees(connector.track_state()) < 0.0)
            .collect();
        loop {
            let mut changed = false;
            for (index, exits) in edges.iter().enumerate() {
                if !reaches_level[index] && exits.iter().any(|exit| reaches_level[*exit]) {
                    reaches_level[index] = true;
                    changed = true;
                }
                if !reaches_descent[index] && exits.iter().any(|exit| reaches_descent[*exit]) {
                    reaches_descent[index] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut has_climb = false;
        for (index, connector) in connectors.iter().enumerate() {
            assert!(
                reaches_level[index],
                "{} cannot return to straight, level, unbanked track from {connector:?}",
                model.name
            );
            if slope_degrees(connector.track_state()) > 0.0 {
                has_climb = true;
                assert!(
                    reaches_descent[index],
                    "{} cannot crest from {connector:?}",
                    model.name
                );
            }
        }
        assert!(
            has_climb,
            "{} has no reachable climbing connector",
            model.name
        );
    }
}

#[test]
fn gentle_slopes_offer_heading_changes_from_every_reachable_connector() {
    let level = ConnectorState::from(TrackState::origin());
    let mut failures = Vec::new();
    for expectations in capability_expectations() {
        if !expectations.turns_on_gentle_slopes {
            continue;
        }
        let model = expectations.model;
        let connectors = reachable_connectors(&model, level, false);
        for slope in [Slope::UpGentle, Slope::DownGentle] {
            let matching: Vec<_> = connectors
                .iter()
                .filter(|connector| {
                    connector.slope == slope
                        && connector.vertical_curvature == VerticalCurvature::Straight
                })
                .collect();
            if matching.is_empty() {
                failures.push(format!(
                    "{} has no reachable constant {slope:?} connector",
                    model.name
                ));
            }
            for connector in matching {
                let state = connector.track_state();
                if !model
                    .available_pieces(state)
                    .iter()
                    .any(|piece| piece.exit.heading != state.heading && piece.exit.slope == slope)
                {
                    failures.push(format!(
                        "{} has no {slope:?} turn from {connector:?}",
                        model.name
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_model_offers_a_reachable_chain_lift() {
    let level = ConnectorState::from(TrackState::origin());
    let mut failures = Vec::new();
    for expectations in capability_expectations() {
        let model = expectations.model;
        let chain = reachable_connectors(&model, level, true);
        if !chain
            .iter()
            .any(|connector| slope_degrees(connector.track_state()) > 0.0)
        {
            failures.push(format!(
                "{} cannot reach a climbing chain lift from level",
                model.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn chain_lifts_can_turn_on_a_reachable_climb() {
    let level = ConnectorState::from(TrackState::origin());
    let mut failures = Vec::new();
    for expectations in capability_expectations() {
        if !expectations.curved_chain {
            continue;
        }
        let model = expectations.model;
        let chain = reachable_connectors(&model, level, true);
        if !chain.iter().any(|connector| {
            let state = connector.track_state();
            slope_degrees(state) > 0.0
                && model
                    .available_pieces(state)
                    .iter()
                    .any(|piece| model.allows_chain(piece) && piece.exit.heading != state.heading)
        }) {
            failures.push(format!(
                "{} cannot reach a curved chain lift from level",
                model.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn chain_lifts_reach_each_models_maximum_lift_slope() {
    let level = ConnectorState::from(TrackState::origin());
    let mut failures = Vec::new();
    for expectations in capability_expectations() {
        let model = expectations.model;
        let chain = reachable_connectors(&model, level, true);
        let maximum = chain
            .iter()
            .map(|connector| slope_degrees(connector.track_state()))
            .fold(0.0_f64, f64::max);
        if (maximum - expectations.maximum_lift_degrees).abs() > 1.0 {
            failures.push(format!(
                "{} chain reaches {maximum:.1} degrees; expected {:.1}",
                model.name, expectations.maximum_lift_degrees
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn models_offer_short_straights_full_tile_offsets_and_height_steps() {
    for model in coaster_models() {
        let state = TrackState::origin();
        let pieces = model.available_pieces(state);
        for (name, x, y, z) in [
            ("Half-tile straight", 1, 0, 0),
            ("Left full-tile offset", 12, 2, 0),
            ("Right full-tile offset", 12, -2, 0),
            ("Short rise", 6, 0, 1),
            ("Short descent", 6, 0, -1),
        ] {
            let piece = pieces
                .iter()
                .find(|piece| piece.name == name)
                .unwrap_or_else(|| panic!("{} does not offer {name}", model.name));
            assert_eq!((piece.exit.x, piece.exit.y, piece.exit.z), (x, y, z));
            assert_eq!(
                ConnectorState::from(piece.exit),
                ConnectorState::from(state)
            );
            assert_eq!(piece.exit.heading, state.heading);
        }
    }
}

#[test]
fn near_station_gaps_can_be_closed_in_a_few_pieces() {
    for model in coaster_models() {
        for (gap, names) in [
            ((3, 0, 0), vec!["Half-tile straight", "One-tile straight"]),
            (
                (13, 2, 0),
                vec!["Left full-tile offset", "Half-tile straight"],
            ),
            (
                (13, -2, 0),
                vec!["Right full-tile offset", "Half-tile straight"],
            ),
            ((9, 0, 1), vec!["Shallow rise", "Half-tile straight"]),
            ((9, 0, -1), vec!["Shallow descent", "Half-tile straight"]),
        ] {
            let mut end = TrackState::origin();
            for name in names {
                end = model
                    .available_pieces(end)
                    .into_iter()
                    .find(|piece| piece.name == name)
                    .unwrap_or_else(|| panic!("{} cannot use {name} at {end:?}", model.name))
                    .exit;
            }
            let station = TrackState {
                x: gap.0,
                y: gap.1,
                z: gap.2,
                ..TrackState::origin()
            };
            assert_eq!(end, station, "{} cannot close gap {gap:?}", model.name);
        }
    }
}

#[test]
fn models_offer_short_gentle_straights() {
    for model in coaster_models() {
        for (slope, height, name) in [
            (Slope::UpGentle, 1, "One-tile gentle climb"),
            (Slope::DownGentle, -1, "One-tile gentle descent"),
        ] {
            let state = TrackState {
                slope,
                ..TrackState::origin()
            };
            let piece = model
                .available_pieces(state)
                .into_iter()
                .find(|piece| piece.name == name)
                .unwrap_or_else(|| panic!("{} does not offer {name}", model.name));
            assert_eq!((piece.exit.x, piece.exit.y, piece.exit.z), (2, 0, height));
            assert_eq!(
                ConnectorState::from(piece.exit),
                ConnectorState::from(state)
            );
        }
    }
}
