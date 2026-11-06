use std::collections::BTreeSet;

#[path = "../examples/circuit_report.rs"]
mod circuit_report;

const HISTORICAL_TIMBER_TUMBLER: &str =
    include_str!("fixtures/timber-tumbler-calibration-39d9e51.txt");
const SPEED_MATCHED_PREFIX: &str = "+chain\nLevelToGentleUp\nGentleUp\nGentleUpToLevel\n-chain\nLevelToGentleDown\nGentleDown\nGentleDown\nGentleDownToLevel\n";

#[test]
fn timber_report_marks_its_four_out_of_window_pieces_and_extreme_lateral_force() {
    // This calibration assertion describes the layout at 39d9e51. Keep that
    // layout as a fixture so later improvements to the shipped ride do not
    // change what this report regression test measures.
    let report = circuit_report::report_for_circuit(
        HISTORICAL_TIMBER_TUMBLER,
        "Timber Tumbler",
        true,
        false,
    )
    .expect("the historical Timber Tumbler report should run");

    let rows: Vec<&str> = report.lines().filter(|line| is_piece_row(line)).collect();
    let outside_rows: Vec<&str> = rows
        .iter()
        .copied()
        .filter(|line| line.split_whitespace().any(|field| field == "OUTSIDE"))
        .collect();
    assert_eq!(
        outside_rows.len(),
        4,
        "expected the four rated-speed violations from the calibration report; flagged rows: {outside_rows:#?}\n{report}"
    );
    let outside_piece_ids: BTreeSet<&str> = outside_rows
        .iter()
        .map(|line| line.split_whitespace().nth(1).expect("piece row has an id"))
        .collect();
    assert_eq!(
        outside_piece_ids,
        BTreeSet::from([
            "SteepToSixtyUp",
            "SixtyToSteepUp",
            "SteepToSixtyDown",
            "SixtyToSteepDown",
        ]),
        "the four rated-speed flags should identify the pieces recorded in the calibration run"
    );

    let maximum_lateral_g = rows
        .iter()
        .filter_map(|line| line.split_whitespace().nth(8))
        .filter_map(|value| value.parse::<f64>().ok())
        .fold(0.0_f64, f64::max);
    assert!(
        maximum_lateral_g > 1.5,
        "expected a per-piece lateral force above 1.5 g, got {maximum_lateral_g}\n{report}"
    );
    assert!(report.contains("Score breakdown"), "{report}");
}

#[test]
fn speed_matched_turn_directive_selects_a_window_that_contains_prefix_speed() {
    let report = report_with_directive("@turn left large");

    let turn_row = report
        .lines()
        .find(|line| line.contains("SpeedTurnLarge") && line.contains("Left"))
        .expect("@turn left large should resolve to a concrete speed-matched turn id");
    assert_entry_speed_fits(turn_row, &report);
}

#[test]
fn hill_directive_selects_a_concrete_floater_inside_its_window() {
    let report = report_with_directive("@hill floater");
    let hill_row = report
        .lines()
        .find(|line| {
            line.split_whitespace()
                .nth(1)
                .is_some_and(|piece_id| piece_id.starts_with("FloaterHill"))
        })
        .expect("@hill floater should resolve to a concrete floater hill id");

    assert_entry_speed_fits(hill_row, &report);
}

#[test]
fn speed_matched_turn_directive_reports_when_no_window_fits() {
    let circuit = "[Timber Tumbler]\n@turn left large\n";
    let error = circuit_report::report_for_circuit(circuit, "Timber Tumbler", false, false)
        .expect_err("a station-only prefix should not fit a speed-matched turn");

    assert_eq!(error, "No fitting size for @turn left large at 2.00 m/s");
}

#[test]
fn close_option_reports_a_completion_search_result_for_a_nearby_prefix() {
    let circuit = "[Timber Tumbler]\nTurnSmallStandalone\nTurnSmallStandalone\nStraight\nTurnSmallStandalone\n";
    let report = circuit_report::report_for_circuit(circuit, "Timber Tumbler", true, true)
        .expect("the short prefix should produce a report with --close");

    assert!(
        report.contains("Completion: ["),
        "expected the nearby prefix to receive completion pieces\n{report}"
    );
}

fn report_with_directive(directive: &str) -> String {
    let circuit = format!("[Timber Tumbler]\n{SPEED_MATCHED_PREFIX}{directive}\n");
    circuit_report::report_for_circuit(&circuit, "Timber Tumbler", true, false)
        .expect("the directive prefix report should run")
}

fn assert_entry_speed_fits(row: &str, report: &str) {
    let fields: Vec<&str> = row.split_whitespace().collect();
    let entry_speed = fields[3]
        .split('[')
        .next()
        .expect("entry speed should have a front-car value")
        .parse::<f64>()
        .expect("entry speed should be numeric");
    let (minimum_speed, maximum_speed) = parse_window(fields[4]);
    assert!(
        (minimum_speed..=maximum_speed).contains(&entry_speed),
        "selected piece entry speed lies outside its rating window: row={row}, report=\n{report}"
    );
}

fn is_piece_row(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|field| field.parse::<usize>().is_ok())
}

fn parse_window(window: &str) -> (f64, f64) {
    let (minimum_speed, maximum_speed) = window
        .split_once("..")
        .or_else(|| window.split_once('–'))
        .or_else(|| window.split_once('-'))
        .expect("rated speed window should contain a separator");
    (
        minimum_speed
            .parse()
            .expect("minimum rated speed should be numeric"),
        maximum_speed
            .parse()
            .expect("maximum rated speed should be numeric"),
    )
}
