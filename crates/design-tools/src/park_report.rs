use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use theme_park_sim::park::tick::COASTER_PLACE_ID;
use theme_park_sim::physics::TestRun;
use theme_park_sim::track::ride_file::RideFile;
use theme_park_sim::world::PathKind;
use theme_park_sim_wasm::WasmSimulation;

use crate::commands::{
    agreement_json, check_expectations, compare_estimates, inline_park_ride_files,
    replay_with_estimates, resolve_park_ride_path, validate_model,
};

#[derive(Clone, Copy)]
struct ParkReportOptions {
    guests: usize,
    minutes: u32,
    seed: u64,
}

impl Default for ParkReportOptions {
    fn default() -> Self {
        Self {
            guests: 2_000,
            minutes: 20,
            seed: 42,
        }
    }
}

#[derive(Clone, Copy)]
struct ShowcaseTargets {
    excitement: (f64, f64),
    sustained_pullout: Option<(f64, f64)>,
    sustained_airtime: Option<(f64, f64)>,
    maximum_lateral: f64,
}

pub(crate) fn run_park_report(arguments: &[String], json_mode: bool) -> Result<String, String> {
    if arguments.len() < 2 {
        return Err("usage: tools/design park report <park file> [--guests 2000] [--minutes 20] [--seed 42]".into());
    }
    let park_path = Path::new(&arguments[1]);
    let options = parse_park_report_options(&arguments[2..])?;
    let park_source = fs::read_to_string(park_path)
        .map_err(|error| format!("read park description {}: {error}", park_path.display()))?;
    let park_value: Value = serde_json::from_str(&park_source)
        .map_err(|error| format!("parse park description {}: {error}", park_path.display()))?;
    let park_name = park_value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "park description field `name` must be a string".to_owned())?;
    let coaster_descriptions = park_value
        .get("coasters")
        .and_then(Value::as_array)
        .ok_or_else(|| "park description field `coasters` must be an array".to_owned())?;
    if coaster_descriptions.len() > 1 {
        return Err("this build supports one player coaster per park".into());
    }

    let runtime_description = inline_park_ride_files(park_path, &park_source)?;
    let mut simulation =
        WasmSimulation::from_park_description(options.seed, options.guests, &runtime_description)
            .map_err(|error| error.to_string())?;

    let mut coaster_json = Vec::new();
    let mut coaster_text = String::new();
    let mut coaster_name = None;
    let ticks = options
        .minutes
        .checked_mul(60 * 30)
        .ok_or_else(|| "park report duration exceeds the supported tick count".to_owned())?;
    if let Some(coaster_description) = coaster_descriptions.first() {
        let ride_reference = coaster_description
            .get("ride")
            .and_then(Value::as_str)
            .ok_or_else(|| "park coaster field `ride` must be a ride file path".to_owned())?;
        let ride_path = resolve_park_ride_path(0, ride_reference)?;
        let ride_source = fs::read_to_string(&ride_path)
            .map_err(|error| format!("read park coaster ride {}: {error}", ride_path.display()))?;
        let mut ride: RideFile = serde_json::from_str(&ride_source)
            .map_err(|error| format!("parse park coaster ride {}: {error}", ride_path.display()))?;
        coaster_name = Some(ride.name.clone());
        ride.station = serde_json::from_value(
            coaster_description
                .get("station")
                .cloned()
                .ok_or_else(|| "park coaster field `station` is required".to_owned())?,
        )
        .map_err(|error| format!("parse park coaster station: {error}"))?;

        let error = simulation.ride_test("player");
        if !error.is_empty() {
            return Err(format!("park coaster {} test failed: {error}", ride.name));
        }
        let test_run = simulation
            .construction_report
            .as_ref()
            .cloned()
            .ok_or_else(|| "park coaster test completed without a native report".to_owned())?;
        let (circuit, _) = simulation
            .construction_circuit_and_train()
            .ok_or_else(|| "park coaster test completed without a circuit".to_owned())?;
        let (_, estimates, _) = replay_with_estimates(&ride, false)?;
        let agreement = compare_estimates(&ride, &test_run, &estimates)?;
        let expectation_failures = check_expectations(&ride, &test_run, &circuit)?;
        if !simulation.construction_is_open() && !simulation.construction_set_open(true) {
            return Err(format!(
                "park coaster {} could not open: {}",
                ride.name,
                simulation.construction_message()
            ));
        }
        let throughput_report = finish_throughput_analysis(&mut simulation)?;

        let targets = showcase_targets(&ride.model)?;
        let throughput_estimate = throughput_report
            .get("theoreticalHourlyRiders")
            .and_then(Value::as_f64);
        let throughput_status = throughput_report
            .get("throughputStatus")
            .and_then(Value::as_str)
            .ok_or_else(|| "block throughput report omitted throughputStatus".to_owned())?;
        let model = validate_model(&ride.model)?;
        let seats_per_train = usize::from(ride.cars_per_train)
            * usize::from(model.train.rows_per_car)
            * usize::from(model.train.seats_per_row);
        let train_count = simulation.construction_train_count();
        let sustained = test_run.sustained_force_peaks();
        let construction = simulation
            .construction
            .as_ref()
            .ok_or_else(|| "park coaster test completed without a construction".to_owned())?;
        let piece_names: Vec<_> = construction
            .pieces()
            .iter()
            .map(|piece| piece.name)
            .collect();
        let worst_pieces = worst_pieces_by_axis(&piece_names, &test_run);
        let expectation_status = if ride.expect.is_none() {
            "not set"
        } else if expectation_failures.is_empty() {
            "pass"
        } else {
            "fail"
        };

        writeln!(coaster_text, "Coaster: {} ({})", ride.name, ride.model).unwrap();
        writeln!(
            coaster_text,
            "Scores: excitement {:.2}, intensity {:.2}, nausea {:.2}",
            test_run.excitement, test_run.intensity, test_run.nausea
        )
        .unwrap();
        writeln!(
            coaster_text,
            "Expectations: {expectation_status}{}",
            if expectation_failures.is_empty() {
                String::new()
            } else {
                format!(" ({})", expectation_failures.join("; "))
            }
        )
        .unwrap();
        writeln!(
            coaster_text,
            "{}",
            format_force_targets(
                targets,
                sustained,
                test_run.statistics.max_lateral_g,
                test_run.excitement,
            )
        )
        .unwrap();
        writeln!(
            coaster_text,
            "Worst pieces: pullout {}; airtime {}; lateral {}; longitudinal {}",
            format_worst_piece(&worst_pieces, "pullout"),
            format_worst_piece(&worst_pieces, "airtime"),
            format_worst_piece(&worst_pieces, "lateral"),
            format_worst_piece(&worst_pieces, "longitudinal")
        )
        .unwrap();
        writeln!(
            coaster_text,
            "Native agreement: {}",
            if agreement.passed { "pass" } else { "fail" }
        )
        .unwrap();
        let target_json = targets_json(
            targets,
            sustained,
            test_run.statistics.max_lateral_g,
            test_run.excitement,
        );
        coaster_json.push(json!({
            "name": ride.name,
            "model": ride.model,
            "scores": {
                "excitement": test_run.excitement,
                "intensity": test_run.intensity,
                "nausea": test_run.nausea,
            },
            "expectations": {
                "status": expectation_status,
                "failures": expectation_failures,
            },
            "targets": target_json,
            "worstPieceByAxis": worst_pieces,
            "nativeAgreement": agreement_json(&agreement),
            "trains": train_count,
            "carsPerTrain": ride.cars_per_train,
            "seatsPerTrain": seats_per_train,
            "ridesTaken": 0,
            "uniqueRiders": 0,
            "shareOfGuestsRode": 0.0,
            "averageGuestRating": null,
            "ridersPerHour": 0.0,
            "throughputEstimatePerHour": throughput_estimate,
            "throughputStatus": throughput_status,
        }));
    }

    let initial_unreachable_destinations = simulation.park_outcomes().unreachable_destinations;
    for _ in 0..ticks {
        simulation.tick();
    }

    let guest_details = load_guest_details(&simulation, options.guests)?;
    let outcomes = simulation.park_outcomes();
    if let Some(coaster_report) = coaster_json.first_mut() {
        let ride_outcome = outcomes.rides.get(&COASTER_PLACE_ID);
        let rides_taken = ride_outcome.map_or(0, |outcome| outcome.rides_taken);
        let unique_riders = ride_outcome.map_or(0, |outcome| outcome.unique_riders.len());
        let average_guest_rating = ride_outcome.and_then(|outcome| {
            (outcome.guest_rating_count > 0)
                .then_some(outcome.guest_rating_total as f64 / outcome.guest_rating_count as f64)
        });
        let riders_per_hour = rides_taken as f64 * 60.0 / f64::from(options.minutes);
        let train_count = coaster_report["trains"].as_u64().expect("train count") as usize;
        let seats_per_train = coaster_report["seatsPerTrain"]
            .as_u64()
            .expect("seats per train") as usize;
        let throughput_estimate = coaster_report["throughputEstimatePerHour"].as_f64();

        writeln!(
            coaster_text,
            "Trains: {train_count}; seats per train: {seats_per_train}; completed guest rides: {rides_taken}; unique riders: {unique_riders} ({:.1}% of guests)",
            unique_riders as f64 * 100.0 / options.guests as f64
        )
        .unwrap();
        writeln!(
            coaster_text,
            "Riders/hour: {riders_per_hour:.1} / {}",
            throughput_estimate
                .map_or_else(|| "unavailable".to_owned(), |value| format!("{value:.1}"))
        )
        .unwrap();
        writeln!(
            coaster_text,
            "Average guest rating: {}",
            average_guest_rating.map_or_else(
                || "no completed rides".to_owned(),
                |rating| format!("{rating:.1}/100 enjoyment")
            )
        )
        .unwrap();
        coaster_report["ridesTaken"] = json!(rides_taken);
        coaster_report["uniqueRiders"] = json!(unique_riders);
        coaster_report["shareOfGuestsRode"] = json!(unique_riders as f64 / options.guests as f64);
        coaster_report["averageGuestRating"] = json!(average_guest_rating);
        coaster_report["ridersPerHour"] = json!(riders_per_hour);
    }
    let guest_outcomes = build_guest_outcomes(
        &simulation,
        &guest_details,
        &outcomes,
        &initial_unreachable_destinations,
        options.guests,
        coaster_name.as_deref(),
    )?;
    let park_facts = build_park_facts(&simulation)?;
    writeln!(
        coaster_text,
        "Guests with unmet needs: {} ({:.1}% of guests)",
        outcomes.unmet_need_guests.len(),
        outcomes.unmet_need_guests.len() as f64 * 100.0 / options.guests as f64
    )
    .unwrap();
    writeln!(coaster_text, "Queue lengths at end:").unwrap();
    for queue in guest_outcomes["queueLengths"]
        .as_array()
        .ok_or_else(|| "guest report omitted queueLengths".to_owned())?
    {
        writeln!(
            coaster_text,
            "  facility {}: {}",
            queue["facilityId"], queue["guests"]
        )
        .unwrap();
    }
    writeln!(coaster_text, "Guests who could not reach a ride:").unwrap();
    writeln!(coaster_text, "Current access snapshot at report end.").unwrap();
    let unreachable_riders = guest_outcomes["unreachableRideGuests"]
        .as_array()
        .ok_or_else(|| "guest report omitted unreachableRideGuests".to_owned())?;
    if unreachable_riders.is_empty() {
        writeln!(coaster_text, "  none").unwrap();
    } else {
        for guest in unreachable_riders {
            writeln!(
                coaster_text,
                "  {} (guest-{}) could not reach {}",
                guest["name"].as_str().expect("guest report name"),
                guest["guestId"].as_u64().expect("guest report guest ID"),
                guest["destinationName"]
                    .as_str()
                    .expect("guest report destination name")
            )
            .unwrap();
        }
    }
    writeln!(coaster_text, "Unreachable destinations at report end:").unwrap();
    write_destination_rows(
        &mut coaster_text,
        guest_outcomes["unreachableDestinations"]
            .as_array()
            .ok_or_else(|| "guest report omitted unreachableDestinations".to_owned())?,
        "could not reach",
    );
    writeln!(
        coaster_text,
        "Guests who could not reach a ride at park start:"
    )
    .unwrap();
    writeln!(
        coaster_text,
        "Initial access snapshot before simulation ticks."
    )
    .unwrap();
    let initially_unreachable_riders = guest_outcomes["unreachableRideGuestsAtStart"]
        .as_array()
        .ok_or_else(|| "guest report omitted unreachableRideGuestsAtStart".to_owned())?;
    if initially_unreachable_riders.is_empty() {
        writeln!(coaster_text, "  none").unwrap();
    } else {
        for guest in initially_unreachable_riders {
            writeln!(
                coaster_text,
                "  {} (guest-{}) could not reach {}",
                guest["name"].as_str().expect("guest report name"),
                guest["guestId"].as_u64().expect("guest report guest ID"),
                guest["destinationName"]
                    .as_str()
                    .expect("guest report destination name")
            )
            .unwrap();
        }
    }
    writeln!(coaster_text, "Unreachable destinations at park start:").unwrap();
    write_destination_rows(
        &mut coaster_text,
        guest_outcomes["initialUnreachableDestinations"]
            .as_array()
            .ok_or_else(|| "guest report omitted initialUnreachableDestinations".to_owned())?,
        "could not reach",
    );
    writeln!(
        coaster_text,
        "Unreachable facility destinations now: {}; assigned routes lost after path changes: {}",
        guest_outcomes["unreachableDestinations"]
            .as_array()
            .expect("structural destinations are an array")
            .len(),
        guest_outcomes["failedRoutes"]
            .as_array()
            .expect("failed routes are an array")
            .len()
    )
    .unwrap();
    writeln!(
        coaster_text,
        "Guests whose assigned destinations were lost after path changes:"
    )
    .unwrap();
    let failed_routes = guest_outcomes["failedRoutes"]
        .as_array()
        .ok_or_else(|| "guest report omitted failedRoutes".to_owned())?;
    if failed_routes.is_empty() {
        writeln!(coaster_text, "  none").unwrap();
    } else {
        for guest in failed_routes {
            writeln!(
                coaster_text,
                "  {} (guest-{}) lost access to {}",
                guest["guestName"].as_str().expect("guest report name"),
                guest["guestId"].as_u64().expect("guest report guest ID"),
                guest["destinationName"]
                    .as_str()
                    .expect("guest report destination name")
            )
            .unwrap();
        }
    }
    writeln!(coaster_text, "Park facts:").unwrap();
    writeln!(
        coaster_text,
        "  path tiles {}; queue tiles {}; failed placements none",
        park_facts["pathTiles"], park_facts["queueTiles"]
    )
    .unwrap();
    for (kind, count) in park_facts["facilitiesByKind"]
        .as_object()
        .expect("facilitiesByKind is an object")
    {
        writeln!(coaster_text, "  {kind}: {count}").unwrap();
    }

    let root = json!({
        "park": {
            "name": park_name,
            "seed": options.seed,
            "guests": options.guests,
            "simulatedMinutes": options.minutes,
        },
        "coasters": coaster_json,
        "guests": guest_outcomes,
        "parkFacts": park_facts,
    });
    if json_mode {
        return serde_json::to_string_pretty(&root)
            .map(|json| json + "\n")
            .map_err(|error| format!("serialize park report: {error}"));
    }
    let mut text = format!(
        "Park: {} (seed {}, {} guests, {} minutes)\n",
        park_name, options.seed, options.guests, options.minutes
    );
    text.push_str(&coaster_text);
    write_guest_outcome_text(&mut text, &guest_outcomes)?;
    Ok(text)
}

fn parse_park_report_options(arguments: &[String]) -> Result<ParkReportOptions, String> {
    let mut options = ParkReportOptions::default();
    let mut seen = BTreeMap::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| format!("{flag} requires a value"))?;
        if seen.insert(flag.to_owned(), ()).is_some() {
            return Err(format!("repeated park report option {flag:?}"));
        }
        match flag {
            "--guests" => {
                options.guests = value
                    .parse()
                    .map_err(|_| format!("invalid guest count {value:?}"))?;
                if options.guests == 0 {
                    return Err("--guests must be greater than zero".into());
                }
            }
            "--minutes" => {
                options.minutes = value
                    .parse()
                    .map_err(|_| format!("invalid simulation minutes {value:?}"))?;
                if options.minutes == 0 {
                    return Err("--minutes must be greater than zero".into());
                }
            }
            "--seed" => {
                options.seed = value
                    .parse()
                    .map_err(|_| format!("invalid simulation seed {value:?}"))?;
            }
            _ => return Err(format!("unknown park report option {flag:?}")),
        }
        index += 2;
    }
    Ok(options)
}

fn showcase_targets(model: &str) -> Result<ShowcaseTargets, String> {
    match model {
        "hybrid" => Ok(ShowcaseTargets {
            excitement: (8.0, 10.0),
            sustained_pullout: Some((3.0, 3.5)),
            sustained_airtime: Some((-1.2, -0.8)),
            maximum_lateral: 0.6,
        }),
        "velocity_viper" => Ok(ShowcaseTargets {
            excitement: (6.0, 9.0),
            sustained_pullout: Some((3.0, 3.5)),
            sustained_airtime: Some((-1.2, -0.8)),
            maximum_lateral: 0.6,
        }),
        "hyper" => Ok(ShowcaseTargets {
            excitement: (6.0, 8.0),
            sustained_pullout: Some((3.0, 3.5)),
            sustained_airtime: Some((-1.2, -0.8)),
            maximum_lateral: 0.6,
        }),
        "wooden_twister" => Ok(ShowcaseTargets {
            excitement: (5.0, 7.0),
            sustained_pullout: Some((2.3, 2.8)),
            sustained_airtime: Some((-0.9, -0.6)),
            maximum_lateral: 0.6,
        }),
        "family_coaster" => Ok(ShowcaseTargets {
            excitement: (2.5, 4.5),
            sustained_pullout: None,
            sustained_airtime: None,
            maximum_lateral: 0.5,
        }),
        _ => Err(format!(
            "no section 5c force targets are defined for model {model:?}"
        )),
    }
}

fn targets_json(
    targets: ShowcaseTargets,
    sustained: theme_park_sim::physics::SustainedForcePeaks,
    maximum_lateral_g: f64,
    excitement: f64,
) -> Value {
    json!({
        "excitement": {
            "observed": excitement,
            "range": [targets.excitement.0, targets.excitement.1],
            "status": range_status(Some(excitement), Some(targets.excitement)),
        },
        "sustainedPulloutG": {
            "observed": sustained.maximum_vertical_g,
            "range": targets.sustained_pullout,
            "status": range_status(sustained.maximum_vertical_g, targets.sustained_pullout),
        },
        "sustainedAirtimeG": {
            "observed": sustained.minimum_vertical_g,
            "range": targets.sustained_airtime,
            "status": range_status(sustained.minimum_vertical_g, targets.sustained_airtime),
        },
        "maximumLateralG": {
            "observed": maximum_lateral_g,
            "maximum": targets.maximum_lateral,
            "status": if maximum_lateral_g <= targets.maximum_lateral { "pass" } else { "fail" },
        },
    })
}

fn range_status(observed: Option<f64>, target: Option<(f64, f64)>) -> &'static str {
    match (observed, target) {
        (_, None) => "owner input needed",
        (None, Some(_)) => "not measured",
        (Some(value), Some((minimum, maximum))) if (minimum..=maximum).contains(&value) => "pass",
        (Some(_), Some(_)) => "fail",
    }
}

fn format_force_targets(
    targets: ShowcaseTargets,
    sustained: theme_park_sim::physics::SustainedForcePeaks,
    maximum_lateral_g: f64,
    excitement: f64,
) -> String {
    format!(
        "Force targets: excitement {:.1}-{:.1} ({}); pullout {} g for 0.5 s ({}); airtime {} g for 0.3 s ({}); lateral {:.1} g / <= {:.1} g ({})",
        targets.excitement.0,
        targets.excitement.1,
        range_status(Some(excitement), Some(targets.excitement)),
        format_force_range(sustained.maximum_vertical_g, targets.sustained_pullout),
        range_status(sustained.maximum_vertical_g, targets.sustained_pullout),
        format_force_range(sustained.minimum_vertical_g, targets.sustained_airtime),
        range_status(sustained.minimum_vertical_g, targets.sustained_airtime),
        maximum_lateral_g,
        targets.maximum_lateral,
        if maximum_lateral_g <= targets.maximum_lateral {
            "pass"
        } else {
            "fail"
        },
    )
}

fn format_force_range(observed: Option<f64>, target: Option<(f64, f64)>) -> String {
    match (observed, target) {
        (_, None) => "owner input needed".to_owned(),
        (None, Some((minimum, maximum))) => {
            format!("not measured (target {minimum:.1} to {maximum:.1})")
        }
        (Some(value), Some((minimum, maximum))) => {
            format!("{value:.1}; target {minimum:.1} to {maximum:.1}")
        }
    }
}

fn worst_pieces_by_axis(piece_names: &[&str], test_run: &TestRun) -> Value {
    let peaks = test_run.piece_force_peaks();
    let mut worst_positive: Option<(usize, f64)> = None;
    let mut worst_negative: Option<(usize, f64)> = None;
    let mut worst_lateral: Option<(usize, f64)> = None;
    let mut worst_longitudinal: Option<(usize, f64)> = None;
    for peak in peaks {
        let Some(car) = peak.worst_car else {
            continue;
        };
        update_max(
            &mut worst_positive,
            peak.piece_index,
            car.maximum_vertical_g,
        );
        update_min(
            &mut worst_negative,
            peak.piece_index,
            car.minimum_vertical_g,
        );
        update_max(&mut worst_lateral, peak.piece_index, car.maximum_lateral_g);
        update_max(
            &mut worst_longitudinal,
            peak.piece_index,
            car.maximum_longitudinal_g,
        );
    }
    let piece = |value: Option<(usize, f64)>| {
        value.map(|(piece_index, force_g)| {
            let piece_name = piece_names
                .get(piece_index)
                .expect("test-run piece index must match the construction");
            json!({
                "pieceIndex": piece_index,
                "piece": piece_name,
                "forceG": force_g,
            })
        })
    };
    json!({
        "pullout": piece(worst_positive),
        "airtime": piece(worst_negative),
        "lateral": piece(worst_lateral),
        "longitudinal": piece(worst_longitudinal),
    })
}

fn update_max(current: &mut Option<(usize, f64)>, piece_index: usize, value: f64) {
    if current.is_none_or(|(_, maximum)| value > maximum) {
        *current = Some((piece_index, value));
    }
}

fn update_min(current: &mut Option<(usize, f64)>, piece_index: usize, value: f64) {
    if current.is_none_or(|(_, minimum)| value < minimum) {
        *current = Some((piece_index, value));
    }
}

fn format_worst_piece(worst_pieces: &Value, axis: &str) -> String {
    worst_pieces
        .get(axis)
        .and_then(|piece| piece.get("piece"))
        .and_then(Value::as_str)
        .map_or_else(|| "none".to_owned(), str::to_owned)
}

fn finish_throughput_analysis(simulation: &mut WasmSimulation) -> Result<Value, String> {
    loop {
        let report: Value = serde_json::from_str(&simulation.construction_block_report_json())
            .map_err(|error| format!("parse coaster throughput report: {error}"))?;
        let status = report
            .get("throughputStatus")
            .and_then(Value::as_str)
            .ok_or_else(|| "coaster throughput report omitted throughputStatus".to_owned())?;
        if status != "pending" {
            return Ok(report);
        }
        let complete = simulation.construction_advance_block_analysis(1_000);
        if complete {
            let report: Value = serde_json::from_str(&simulation.construction_block_report_json())
                .map_err(|error| format!("parse completed coaster throughput report: {error}"))?;
            if report["throughputStatus"] == "pending" {
                return Err("block analysis completed without a throughput verdict".into());
            }
            return Ok(report);
        }
    }
}

fn load_guest_details(
    simulation: &WasmSimulation,
    guest_count: usize,
) -> Result<BTreeMap<u32, Value>, String> {
    (0..guest_count)
        .map(|guest_id| {
            let guest_id = u32::try_from(guest_id)
                .map_err(|_| format!("guest index {guest_id} exceeds supported ID range"))?;
            let detail: Option<Value> =
                serde_json::from_str(&simulation.guest_detail_json(guest_id))
                    .map_err(|error| format!("parse guest {guest_id} detail: {error}"))?;
            let detail = detail.ok_or_else(|| format!("guest {guest_id} detail is unavailable"))?;
            Ok((guest_id, detail))
        })
        .collect()
}

fn build_guest_outcomes(
    simulation: &WasmSimulation,
    guest_details: &BTreeMap<u32, Value>,
    outcomes: &theme_park_sim::guests::ParkOutcomes,
    initial_unreachable_destinations: &BTreeSet<theme_park_sim::guests::GuestDestination>,
    guest_count: usize,
    coaster_name: Option<&str>,
) -> Result<Value, String> {
    let active_guest_ids: BTreeMap<_, _> = simulation
        .guest_ids()
        .into_iter()
        .map(|guest_id| (guest_id, ()))
        .collect();
    let mut queue_lengths = BTreeMap::<u32, usize>::new();
    let mut total_spending = 0_u64;
    let mut spending_by_guest = Vec::with_capacity(guest_details.len());
    for (&guest_id, detail) in guest_details {
        let lifetime_money_spent = detail["lifetimeMoneySpent"]
            .as_u64()
            .ok_or_else(|| format!("guest {guest_id} omitted lifetimeMoneySpent"))?;
        let name = detail["name"]
            .as_str()
            .ok_or_else(|| format!("guest {guest_id} omitted name"))?;
        total_spending += lifetime_money_spent;
        spending_by_guest.push(json!({
            "guestId": guest_id,
            "name": name,
            "lifetimeMoneySpent": lifetime_money_spent,
        }));
        if !active_guest_ids.contains_key(&guest_id) {
            continue;
        }
        if detail["activity"]["kind"] == "queuing" {
            let facility_id = detail["activity"]["placeId"]
                .as_u64()
                .ok_or_else(|| format!("queuing guest {guest_id} omitted placeId"))?
                as u32;
            *queue_lengths.entry(facility_id).or_default() += 1;
        }
    }

    let facilities: Vec<Value> = serde_json::from_str(&simulation.facilities_json())
        .map_err(|error| format!("parse facility report: {error}"))?;
    let mut revenue_by_facility = BTreeMap::<u32, Value>::new();
    let mut facilities_by_kind = BTreeMap::<String, usize>::new();
    let mut destination_names_by_id = BTreeMap::<u32, String>::new();
    for facility in &facilities {
        let facility_id = facility["id"]
            .as_u64()
            .ok_or_else(|| "facility report omitted an integer ID".to_owned())?
            as u32;
        let kind = facility["kind"]
            .as_str()
            .ok_or_else(|| format!("facility {facility_id} omitted kind"))?;
        destination_names_by_id.insert(facility_id, format!("{kind} #{facility_id}"));
        *facilities_by_kind.entry(kind.to_owned()).or_default() += 1;
        revenue_by_facility.insert(
            facility_id,
            json!({
                "facilityId": facility_id,
                "kind": kind,
                "revenue": simulation.simulation.facility_income(facility_id),
            }),
        );
    }
    if let Some(coaster_name) = coaster_name {
        destination_names_by_id.insert(COASTER_PLACE_ID, coaster_name.to_owned());
    }
    if simulation.construction_is_open() {
        facilities_by_kind.insert("coaster".into(), 1);
        revenue_by_facility.insert(
            COASTER_PLACE_ID,
            json!({
                "facilityId": COASTER_PLACE_ID,
                "kind": "coaster",
                "revenue": simulation.construction_coaster_income(),
            }),
        );
    }
    let facility_ids: BTreeMap<_, _> = revenue_by_facility.keys().map(|id| (*id, ())).collect();
    let queue_lengths: Vec<_> = facility_ids
        .keys()
        .map(|&facility_id| {
            json!({"facilityId": facility_id, "guests": queue_lengths.get(&facility_id).copied().unwrap_or(0)})
        })
        .collect();

    let mut unreachable_destinations = Vec::new();
    let mut failed_routes = Vec::new();
    let mut unreachable_ride_guests = BTreeMap::<u32, Value>::new();
    let destination_json = |destination: &theme_park_sim::guests::GuestDestination| {
        let guest_id = destination.guest_id.0;
        let detail = guest_details.get(&guest_id).ok_or_else(|| {
            format!("unreachable destination references missing guest {guest_id}")
        })?;
        let guest_name = detail["name"]
            .as_str()
            .ok_or_else(|| format!("guest {guest_id} omitted name"))?;
        let destination_name = destination_names_by_id
            .get(&destination.facility_id)
            .ok_or_else(|| {
                format!(
                    "destination outcome references unknown facility {}",
                    destination.facility_id
                )
            })?;
        let record = json!({
            "guestId": guest_id,
            "guestName": guest_name,
            "facilityId": destination.facility_id,
            "destinationName": destination_name,
            "destination": {
                "x": destination.position.x,
                "y": destination.position.y,
                "height": destination.position.height,
            },
        });
        Ok::<_, String>((guest_id, guest_name, destination_name.as_str(), record))
    };
    for destination in &outcomes.unreachable_destinations {
        let (guest_id, guest_name, destination_name, record) = destination_json(destination)?;
        if destination.facility_id == COASTER_PLACE_ID {
            unreachable_ride_guests.insert(
                guest_id,
                json!({
                    "guestId": guest_id,
                    "name": guest_name,
                    "destinationName": destination_name,
                }),
            );
        }
        unreachable_destinations.push(record);
    }
    let mut initial_unreachable_destinations_json = Vec::new();
    let mut initial_unreachable_ride_guests = BTreeMap::<u32, Value>::new();
    for destination in initial_unreachable_destinations {
        let (guest_id, guest_name, destination_name, record) = destination_json(destination)?;
        if destination.facility_id == COASTER_PLACE_ID {
            initial_unreachable_ride_guests.insert(
                guest_id,
                json!({
                    "guestId": guest_id,
                    "name": guest_name,
                    "destinationName": destination_name,
                }),
            );
        }
        initial_unreachable_destinations_json.push(record);
    }
    let mut failed_ride_routes = Vec::new();
    for destination in &outcomes.failed_routes {
        let (_, _, _, record) = destination_json(destination)?;
        if destination.facility_id == COASTER_PLACE_ID {
            failed_ride_routes.push(record.clone());
        }
        failed_routes.push(record);
    }
    let unmet_need_guests: Vec<_> = outcomes
        .unmet_need_guests
        .iter()
        .map(|guest_id| {
            let guest_id = guest_id.0;
            let detail = guest_details
                .get(&guest_id)
                .ok_or_else(|| format!("unmet need outcome references missing guest {guest_id}"))?;
            let guest_name = detail["name"]
                .as_str()
                .ok_or_else(|| format!("guest {guest_id} omitted name"))?;
            Ok(json!({"guestId": guest_id, "name": guest_name}))
        })
        .collect::<Result<_, String>>()?;
    let ride_outcome = outcomes.rides.get(&COASTER_PLACE_ID);
    let rides_taken = ride_outcome.map_or(0, |outcome| outcome.rides_taken);
    let unique_riders = ride_outcome.map_or(0, |outcome| outcome.unique_riders.len());
    let average_guest_rating = ride_outcome.and_then(|outcome| {
        (outcome.guest_rating_count > 0)
            .then_some(outcome.guest_rating_total as f64 / outcome.guest_rating_count as f64)
    });
    let completed_rides_by_coaster = coaster_name.map_or_else(Vec::new, |_| {
        vec![json!({"facilityId": COASTER_PLACE_ID, "ridesTaken": rides_taken})]
    });
    let unique_riders_by_coaster = coaster_name.map_or_else(Vec::new, |_| {
        vec![json!({
            "facilityId": COASTER_PLACE_ID,
            "uniqueRiders": unique_riders,
            "shareOfGuests": unique_riders as f64 / guest_count as f64,
        })]
    });
    let guest_ratings_by_coaster = coaster_name.map_or_else(Vec::new, |_| {
        vec![json!({
            "facilityId": COASTER_PLACE_ID,
            "averageEnjoyment": average_guest_rating,
        })]
    });

    Ok(json!({
        "completedRidesByCoaster": completed_rides_by_coaster,
        "uniqueRidersByCoaster": unique_riders_by_coaster,
        "guestRatingsByCoaster": guest_ratings_by_coaster,
        "queueLengths": queue_lengths,
        "averageSpendingPerGuest": total_spending as f64 / guest_count as f64,
        "spendingByGuest": spending_by_guest,
        "totalRevenue": simulation.facility_revenue(),
        "revenueByFacility": revenue_by_facility.into_values().collect::<Vec<_>>(),
        "unmetNeedGuestCount": outcomes.unmet_need_guests.len(),
        "unmetNeedShare": outcomes.unmet_need_guests.len() as f64 / guest_count as f64,
        "unmetNeedGuests": unmet_need_guests,
        "unreachableDestinations": unreachable_destinations,
        "initialUnreachableDestinations": initial_unreachable_destinations_json,
        "failedRoutes": failed_routes,
        "failedRideRoutes": failed_ride_routes,
        "unreachableRideGuests": unreachable_ride_guests.into_values().collect::<Vec<_>>(),
        "unreachableRideGuestsAtStart": initial_unreachable_ride_guests.into_values().collect::<Vec<_>>(),
        "facilitiesByKind": facilities_by_kind,
    }))
}

fn build_park_facts(simulation: &WasmSimulation) -> Result<Value, String> {
    let mut path_tiles = 0;
    let mut queue_tiles = 0;
    for path in simulation.world.paths() {
        path_tiles += 1;
        if simulation
            .world
            .path_metadata(path.position)
            .ok_or_else(|| format!("path at {:?} has no path metadata", path.position))?
            .kind
            == PathKind::Queue
        {
            queue_tiles += 1;
        }
    }
    let facilities: Vec<Value> = serde_json::from_str(&simulation.facilities_json())
        .map_err(|error| format!("parse facility report: {error}"))?;
    let mut facilities_by_kind = BTreeMap::<String, usize>::new();
    for facility in facilities {
        let kind = facility["kind"]
            .as_str()
            .ok_or_else(|| "facility report omitted kind".to_owned())?;
        *facilities_by_kind.entry(kind.to_owned()).or_default() += 1;
    }
    if simulation.construction_is_open() {
        facilities_by_kind.insert("coaster".into(), 1);
    }
    Ok(json!({
        "pathTiles": path_tiles,
        "queueTiles": queue_tiles,
        "facilitiesByKind": facilities_by_kind,
        "failedPlacements": [],
    }))
}

fn write_destination_rows(output: &mut String, destinations: &[Value], action: &str) {
    if destinations.is_empty() {
        writeln!(output, "  none").unwrap();
        return;
    }
    for destination in destinations {
        writeln!(
            output,
            "  {} (guest-{}) {action} {}",
            destination["guestName"]
                .as_str()
                .expect("destination guest name"),
            destination["guestId"]
                .as_u64()
                .expect("destination guest ID"),
            destination["destinationName"]
                .as_str()
                .expect("destination facility name"),
        )
        .unwrap();
    }
}

fn write_guest_outcome_text(output: &mut String, outcomes: &Value) -> Result<(), String> {
    let average_spending = outcomes["averageSpendingPerGuest"]
        .as_f64()
        .ok_or_else(|| "guest report omitted average spending".to_owned())?;
    let total_revenue = outcomes["totalRevenue"]
        .as_u64()
        .ok_or_else(|| "guest report omitted total revenue".to_owned())?;
    writeln!(output, "Guest outcomes:").unwrap();
    writeln!(
        output,
        "  average spending per guest: {average_spending:.2}"
    )
    .unwrap();
    writeln!(output, "  total revenue: {total_revenue}").unwrap();
    writeln!(output, "  revenue per facility:").unwrap();
    for facility in outcomes["revenueByFacility"]
        .as_array()
        .ok_or_else(|| "guest report omitted revenueByFacility".to_owned())?
    {
        writeln!(
            output,
            "    {} #{}: {}",
            facility["kind"].as_str().expect("facility kind"),
            facility["facilityId"].as_u64().expect("facility ID"),
            facility["revenue"].as_u64().expect("facility revenue")
        )
        .unwrap();
    }
    writeln!(
        output,
        "  guests with unmet needs: {} ({:.1}%)",
        outcomes["unmetNeedGuestCount"]
            .as_u64()
            .expect("unmet need count"),
        outcomes["unmetNeedShare"]
            .as_f64()
            .expect("unmet need share")
            * 100.0
    )
    .unwrap();
    Ok(())
}
