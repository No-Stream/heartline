use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use theme_park_sim::physics::{ForcePeakValues, TestRun};
use theme_park_sim::track::ride_file::{
    RideChainStep, RideCloseStep, RideFile, RideFileFormat, RideHeading, RidePlaceStep,
    RideStation, RideStep,
};
use theme_park_sim::track::{CoasterModel, coaster_models};
use theme_park_sim_wasm::builder_session::BuilderSession;
use theme_park_sim_wasm::construction::{
    ConstructionCandidateForcePreviewWire, ConstructionForceRatingBandWire,
    ConstructionLastPlacedForceWire,
};
use theme_park_sim_wasm::WasmSimulation;

use crate::agreement::{AgreementInput, AgreementReport, ForcePeaks, evaluate_agreement};
use crate::report::{ForceRatings, RidePieceEstimate, render_ride_report};

#[derive(Debug)]
struct CommandReply {
    text: String,
    json: Value,
    changed_ride: bool,
}

#[derive(Debug)]
enum RideCommand {
    New {
        model: String,
        station: RideStation,
        cars: u8,
    },
    State,
    Choices(Option<String>),
    Estimate(Vec<String>),
    Place {
        id: String,
        matched: bool,
        launch_speed: Option<f64>,
        note: Option<String>,
    },
    Chain(bool),
    LaunchSpeed {
        piece_index: usize,
        target_speed: f64,
    },
    Cars(u8),
    Trains(u8),
    AutoBlocks,
    Undo(usize),
    Truncate(usize),
    Finish,
    Close,
    Test,
    Report(bool),
    Agreement,
    Map,
    ExportSave(PathBuf),
    ExportSession(PathBuf),
}

const COMMAND_NAMES: &[&str] = &[
    "new",
    "state",
    "choices",
    "estimate",
    "place",
    "chain",
    "launch-speed",
    "cars",
    "trains",
    "auto-blocks",
    "undo",
    "truncate",
    "finish",
    "close",
    "test",
    "report",
    "agreement",
    "map",
    "export",
];

/// Runs the command line interface and returns exactly what the binary prints to standard output.
pub fn run_cli(arguments: &[String], stdin: &str) -> Result<String, String> {
    let json_mode = arguments.iter().any(|argument| argument == "--json");
    let arguments: Vec<_> = arguments
        .iter()
        .filter(|argument| argument.as_str() != "--json")
        .cloned()
        .collect();
    if arguments.is_empty() {
        return Err(help_text().to_owned());
    }

    match arguments[0].as_str() {
        "ride" => {
            if arguments.len() < 2 {
                return Err("usage: tools/design ride <file> [command ...]".into());
            }
            let mut command_tokens = arguments[2..].to_vec();
            if command_tokens.is_empty() {
                command_tokens.extend(stdin.lines().map(str::to_owned));
            }
            let commands = parse_commands(&command_tokens)?;
            run_ride(Path::new(&arguments[1]), commands, json_mode)
        }
        "check" => run_check(&arguments[1..], json_mode),
        "park" => Err("park build/report require the park description implementation (work items 8–9)".into()),
        _ => Err(help_text().to_owned()),
    }
}

fn help_text() -> &'static str {
    "usage: tools/design ride <file> [command ...]\n       tools/design check <ride file> ...\n       tools/design park build|report <park file> (available after work items 8–9)\n"
}

fn parse_commands(arguments: &[String]) -> Result<Vec<RideCommand>, String> {
    let mut commands = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let inline_words: Vec<_> = arguments[index].split_whitespace().collect();
        let Some(command) = inline_words.first().copied() else {
            return Err("empty ride command".into());
        };
        if !COMMAND_NAMES.contains(&command) {
            return Err(format!("unknown ride command {command:?}"));
        }
        let mut parameters: Vec<String> = inline_words[1..].iter().map(|word| (*word).to_owned()).collect();
        let mut end = index + 1;
        while end < arguments.len() {
            let next_words: Vec<_> = arguments[end].split_whitespace().collect();
            if next_words.first().is_some_and(|next| COMMAND_NAMES.contains(next)) {
                break;
            }
            parameters.push(arguments[end].clone());
            end += 1;
        }
        if command == "place"
            && arguments[index].contains("--note")
            && inline_words.iter().any(|word| *word == "--note")
        {
            let command_text = arguments[index].as_str();
            let (before_note, note) = command_text.split_once("--note").unwrap();
            let mut inline_parameters: Vec<String> = before_note
                .split_whitespace()
                .skip(2)
                .map(str::to_owned)
                .collect();
            inline_parameters.push("--note".to_owned());
            if !note.trim().is_empty() {
                inline_parameters.push(note.trim().to_owned());
            }
            inline_parameters.extend(arguments[index + 1..end].iter().cloned());
            parameters = inline_parameters;
        }
        commands.push(parse_command(command, &parameters)?);
        index = end;
    }
    Ok(commands)
}

fn parse_command(command: &str, parameters: &[String]) -> Result<RideCommand, String> {
    let usage = || format!("invalid arguments for {command}");
    match command {
        "new" => {
            let mut model = None;
            let mut station = None;
            let mut cars = 6_u8;
            let mut index = 0;
            while index < parameters.len() {
                match parameters[index].as_str() {
                    "--model" => {
                        index += 1;
                        model = Some(parameters.get(index).ok_or_else(usage)?.clone());
                    }
                    "--station" => {
                        index += 1;
                        station = Some(parse_station(parameters.get(index).ok_or_else(usage)?)?);
                    }
                    "--cars" => {
                        index += 1;
                        cars = parse_number(parameters.get(index).ok_or_else(usage)?, "cars")?;
                    }
                    flag => return Err(format!("unknown new option {flag:?}")),
                }
                index += 1;
            }
    Ok(RideCommand::New {
                model: model.ok_or_else(|| "new requires --model <key>".to_owned())?,
                station: station.ok_or_else(|| "new requires --station x,y,z,heading".to_owned())?,
                cars,
            })
        }
        "state" if parameters.is_empty() => Ok(RideCommand::State),
        "choices" if parameters.len() <= 1 => Ok(RideCommand::Choices(parameters.first().cloned())),
        "estimate" if !parameters.is_empty() => Ok(RideCommand::Estimate(parameters.to_vec())),
        "place" => parse_place(parameters),
        "chain" if parameters.len() == 1 => match parameters[0].as_str() {
            "on" => Ok(RideCommand::Chain(true)),
            "off" => Ok(RideCommand::Chain(false)),
            _ => Err("chain must be 'on' or 'off'".into()),
        },
        "launch-speed" if parameters.len() == 2 => Ok(RideCommand::LaunchSpeed {
            piece_index: parse_number(&parameters[0], "piece index")?,
            target_speed: parse_number(&parameters[1], "launch speed")?,
        }),
        "cars" if parameters.len() == 1 => Ok(RideCommand::Cars(parse_number(&parameters[0], "cars")?)),
        "trains" if parameters.len() == 1 => Ok(RideCommand::Trains(parse_number(&parameters[0], "trains")?)),
        "auto-blocks" if parameters.is_empty() => Ok(RideCommand::AutoBlocks),
        "undo" if parameters.len() <= 1 => Ok(RideCommand::Undo(
            parameters
                .first()
                .map_or(Ok(1), |value| parse_number(value, "undo count"))?,
        )),
        "truncate" if parameters.len() == 1 => Ok(RideCommand::Truncate(parse_number(
            &parameters[0],
            "step count",
        )?)),
        "finish" if parameters.is_empty() => Ok(RideCommand::Finish),
        "close" if parameters.is_empty() => Ok(RideCommand::Close),
        "test" if parameters.is_empty() => Ok(RideCommand::Test),
        "report" if parameters.is_empty() || parameters == ["--samples"] => {
            Ok(RideCommand::Report(parameters == ["--samples"]))
        }
        "agreement" if parameters.is_empty() => Ok(RideCommand::Agreement),
        "map" if parameters.is_empty() => Ok(RideCommand::Map),
        "export" if parameters.len() == 2 => match parameters[0].as_str() {
            "--save" => Ok(RideCommand::ExportSave(PathBuf::from(&parameters[1]))),
            "--session" => Ok(RideCommand::ExportSession(PathBuf::from(&parameters[1]))),
            _ => Err("export requires --save <path> or --session <path>".into()),
        },
        known if COMMAND_NAMES.contains(&known) => Err(usage()),
        _ => Err(format!("unknown ride command {command:?}")),
    }
}

fn parse_place(parameters: &[String]) -> Result<RideCommand, String> {
    let Some(id) = parameters.first() else {
        return Err("place requires a piece id".into());
    };
    let (mut matched, mut launch_speed, mut note) = (false, None, None);
    let mut index = 1;
    while index < parameters.len() {
        match parameters[index].as_str() {
            "--matched" => matched = true,
            "--launch-speed" => {
                index += 1;
                launch_speed = Some(parse_number(
                    parameters.get(index).ok_or("--launch-speed requires a value")?,
                    "launch speed",
                )?);
            }
            "--note" => {
                index += 1;
                let start = index;
                while index < parameters.len() && !parameters[index].starts_with("--") {
                    index += 1;
                }
                if start == index {
                    return Err("--note requires text".into());
                }
                note = Some(parameters[start..index].join(" "));
                continue;
            }
            option => return Err(format!("unknown place option {option:?}")),
        }
        index += 1;
    }
    Ok(RideCommand::Place {
        id: id.clone(),
        matched,
        launch_speed,
        note,
    })
}

fn parse_station(value: &str) -> Result<RideStation, String> {
    let values: Vec<_> = value.split(',').collect();
    if values.len() != 4 {
        return Err("station must be x,y,z,heading".into());
    }
    let heading = match values[3] {
        "east" => RideHeading::East,
        "north" => RideHeading::North,
        "west" => RideHeading::West,
        "south" => RideHeading::South,
        value => return Err(format!("unknown station heading {value:?}")),
    };
    Ok(RideStation {
        x: parse_number(values[0], "station x")?,
        y: parse_number(values[1], "station y")?,
        z: parse_number(values[2], "station z")?,
        heading,
    })
}

fn parse_number<T: std::str::FromStr>(value: &str, description: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid {description}: {value:?}"))
}

fn run_ride(path: &Path, commands: Vec<RideCommand>, json_mode: bool) -> Result<String, String> {
    let path = path.to_path_buf();
    let file_exists = path.exists();
    let mut commands = commands.into_iter();
    let mut ride = if file_exists {
        let bytes = fs::read(&path).map_err(|error| format!("read ride {}: {error}", path.display()))?;
        serde_json::from_slice::<RideFile>(&bytes)
            .map_err(|error| format!("parse ride {}: {error}", path.display()))?
    } else {
        match commands.next() {
            Some(RideCommand::New { model, station, cars }) => {
                validate_model(&model)?;
                RideFile {
                    format: RideFileFormat::HeartlineRide,
                    version: 1,
                    name: path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("new ride")
                        .replace(['-', '_'], " "),
                    model,
                    station,
                    cars_per_train: cars,
                    trains: 1,
                    steps: Vec::new(),
                    expect: None,
                }
            }
            Some(_) => return Err(format!("ride file {} does not exist; start with new", path.display())),
            None => return Err(format!("ride file {} does not exist", path.display())),
        }
    };

    let mut session = create_session(&ride)?;
    if !file_exists {
        let mut state = session.estimates();
        let _ = &mut state;
        session
            .set_cars_per_train(ride.cars_per_train)
            .map_err(|error| error.to_string())?;
        session.estimates();
    }

    let mut replies = Vec::new();
    let mut changed_ride = !file_exists;
    for command in commands {
        let result = execute_command(command, &mut ride, &mut session);
        match result {
            Ok(reply) => {
                changed_ride |= reply.changed_ride;
                replies.push(reply);
            }
            Err(error) => {
                let current_state = render_state(&mut session).map(|reply| reply.text).unwrap_or_default();
                return Err(format!("{error}\nState at failure:\n{current_state}"));
            }
        }
    }
    if replies.is_empty() {
        replies.push(render_state(&mut session)?);
    }
    if changed_ride {
        write_ride_atomic(&path, &ride)?;
    }
    if json_mode {
        let values: Vec<_> = replies.into_iter().map(|reply| reply.json).collect();
        return serde_json::to_string_pretty(&if values.len() == 1 {
            values.into_iter().next().expect("one reply")
        } else {
            Value::Array(values)
        })
        .map(|value| value + "\n")
        .map_err(|error| error.to_string());
    }
    Ok(replies.into_iter().map(|reply| reply.text).collect())
}

fn create_session(ride: &RideFile) -> Result<BuilderSession, String> {
    let simulation = WasmSimulation::new(42, 0, "new".into());
    let mut session = BuilderSession::new(simulation);
    session
        .replay_ride(ride)
        .map_err(|error| error.to_string())?;
    Ok(session)
}

fn validate_model(model: &str) -> Result<CoasterModel, String> {
    coaster_models()
        .into_iter()
        .find(|candidate| candidate.key == model)
        .ok_or_else(|| format!("unknown coaster model {model:?}"))
}

fn execute_command(
    command: RideCommand,
    ride: &mut RideFile,
    session: &mut BuilderSession,
) -> Result<CommandReply, String> {
    match command {
        RideCommand::New { .. } => Err("new must be the first command for a new ride file".into()),
        RideCommand::State => render_state(session),
        RideCommand::Choices(group) => render_choices(session, group.as_deref()),
        RideCommand::Estimate(ids) => render_estimates(session, &ids),
        RideCommand::Place {
            id,
            matched,
            launch_speed,
            note,
        } => {
            let selection = session.select(&id, !matched).map_err(|error| error.to_string())?;
            let estimates = session.estimates();
            let candidate = estimates
                .candidate_preview
                .as_ref()
                .ok_or_else(|| format!("candidate preview is unavailable for {}", selection.selected_id))?;
            let placement_json = candidate_value(&selection.selected_id, candidate, None);
            let _placed_estimates = session.place().map_err(|error| error.to_string())?;
            let track = construction_track(session)?;
            let piece_id = track
                .last()
                .and_then(|piece| piece["id"].as_u64())
                .map(|id| id as usize)
                .ok_or_else(|| "placed piece has no bridge id".to_owned())?;
            if let Some(target_speed) = launch_speed {
                session
                    .set_launch_speed(piece_id, target_speed)
                    .map_err(|error| error.to_string())?;
                session.estimates();
            }
            let saved_id = if selection.selected_id.starts_with("element:") {
                selection.selected_id
            } else {
                track.last().unwrap()["label"]
                    .as_str()
                    .expect("track label is a string")
                    .to_owned()
            };
            ride.steps.push(RideStep::Place(RidePlaceStep {
                place: saved_id.clone(),
                launch_speed,
                note: note.clone(),
            }));
            let state = state_value(session)?;
            let mut text = format!(
                "Placed {saved_id}{}\n",
                if matched && id != saved_id {
                    format!(" (speed matched from {id})")
                } else {
                    String::new()
                }
            );
            write!(text, "Estimate: {} m/s entry, {}\n", optional_number(candidate.entry_speed), format_peaks(candidate.peaks.as_ref())).unwrap();
            text.push_str(&format_state_text(&state));
            Ok(CommandReply {
                text,
                json: json!({"placed": saved_id, "requested": id, "estimate": placement_json, "state": state}),
                changed_ride: true,
            })
        }
        RideCommand::Chain(enabled) => {
            session.set_chain(enabled).map_err(|error| error.to_string())?;
            ride.steps.push(RideStep::Chain(RideChainStep { chain: enabled }));
            let state = state_value(session)?;
            Ok(reply_state(format!("Chain lift: {}\n", if enabled { "on" } else { "off" }), state, true))
        }
        RideCommand::LaunchSpeed { piece_index, target_speed } => {
            if piece_index == 0 {
                return Err("launch-speed piece index is one-based".into());
            }
            let place_step_index = ride
                .steps
                .iter()
                .enumerate()
                .filter_map(|(step_index, step)| matches!(step, RideStep::Place(_)).then_some(step_index))
                .nth(piece_index - 1)
                .ok_or_else(|| format!("ride has no placed piece {piece_index}"))?;
            let piece = ride.steps[place_step_index]
                .clone();
            let RideStep::Place(mut place) = piece else {
                unreachable!();
            };
            let track = construction_track(session)?;
            let bridge_id = track
                .get(piece_index)
                .and_then(|piece| piece["id"].as_u64())
                .map(|id| id as usize)
                .ok_or_else(|| format!("bridge has no placed piece {piece_index}"))?;
            session
                .set_launch_speed(bridge_id, target_speed)
                .map_err(|error| error.to_string())?;
            place.launch_speed = Some(target_speed);
            ride.steps[place_step_index] = RideStep::Place(place);
            let state = state_value(session)?;
            Ok(reply_state(format!("Launch speed for piece {piece_index}: {target_speed:.1} m/s\n"), state, true))
        }
        RideCommand::Cars(cars) => {
            session.set_cars_per_train(cars).map_err(|error| error.to_string())?;
            ride.cars_per_train = cars;
            let state = state_value(session)?;
            Ok(reply_state(format!("Cars per train: {cars}\n"), state, true))
        }
        RideCommand::Trains(trains) => {
            session
                .set_train_count(usize::from(trains))
                .map_err(|error| error.to_string())?;
            ride.trains = trains;
            let state = state_value(session)?;
            Ok(reply_state(format!("Train count: {trains}\n"), state, true))
        }
        RideCommand::AutoBlocks => {
            let block_report = session.auto_blocks().map_err(|error| error.to_string())?;
            Ok(CommandReply {
                text: format!("Automatic block report:\n{}\n", serde_json::to_string_pretty(&block_report).unwrap()),
                json: block_report,
                changed_ride: false,
            })
        }
        RideCommand::Undo(count) => {
            let mut removed = 0;
            for _ in 0..count {
                session.undo().map_err(|error| error.to_string())?;
                if let Some(close_index) = ride.steps.iter().position(|step| matches!(step, RideStep::Close(_))) {
                    ride.steps.remove(close_index);
                }
                let place_index = ride.steps.iter().rposition(|step| matches!(step, RideStep::Place(_)))
                    .ok_or_else(|| "undo removed a bridge piece with no matching ride step".to_owned())?;
                ride.steps.remove(place_index);
                removed += 1;
            }
            let state = state_value(session)?;
            Ok(reply_state(format!("Undid {removed} placement(s).\n"), state, removed > 0))
        }
        RideCommand::Truncate(step_count) => {
            if step_count > ride.steps.len() {
                return Err(format!("ride has {} steps; cannot keep {step_count}", ride.steps.len()));
            }
            ride.steps.truncate(step_count);
            *session = create_session(ride)?;
            let state = state_value(session)?;
            Ok(reply_state(format!("Kept {step_count} ride step(s).\n"), state, true))
        }
        RideCommand::Finish => {
            let track_before = construction_track(session)?.len();
            session.finish().map_err(|error| error.to_string())?;
            let track_after = construction_track(session)?;
            let added = track_after.len().saturating_sub(track_before);
            for piece in track_after.iter().skip(track_before) {
                let id = piece["label"]
                    .as_str()
                    .ok_or_else(|| "finished piece has no label".to_owned())?;
                ride.steps.push(RideStep::Place(RidePlaceStep {
                    place: id.to_owned(),
                    launch_speed: None,
                    note: None,
                }));
            }
            let state = state_value(session)?;
            Ok(reply_state(format!("Finish search added {added} piece(s).\n"), state, added > 0))
        }
        RideCommand::Close => {
            session.close().map_err(|error| error.to_string())?;
            ride.steps.push(RideStep::Close(RideCloseStep { close: true }));
            let state = state_value(session)?;
            Ok(reply_state("Circuit closed.\n".to_owned(), state, true))
        }
        RideCommand::Test => {
            let (test_run, estimates) = test_ride(ride)?;
            let report = render_ride_report(ride, &test_run, &estimates, false)?;
            Ok(CommandReply {
                text: report.clone(),
                json: json!({"ride": ride, "testRun": test_run, "estimates": estimates, "text": report}),
                changed_ride: false,
            })
        }
        RideCommand::Report(include_samples) => {
            let (test_run, estimates) = test_ride(ride)?;
            let report = render_ride_report(ride, &test_run, &estimates, include_samples)?;
            Ok(CommandReply {
                text: report.clone(),
                json: json!({"ride": ride, "testRun": test_run, "estimates": estimates, "text": report}),
                changed_ride: false,
            })
        }
        RideCommand::Agreement => {
            let (test_run, estimates) = test_ride(ride)?;
            let agreement = compare_estimates(ride, &test_run, &estimates)?;
            let text = format_agreement(&agreement);
            Ok(CommandReply {
                text,
                json: agreement_json(&agreement),
                changed_ride: false,
            })
        }
        RideCommand::Map => render_map(session),
        RideCommand::ExportSave(save_path) => {
            let bytes = session
                .simulation()
                .save_state_native()
                .map_err(|error| error.to_string())?;
            write_bytes_atomic(&save_path, &bytes)?;
            Ok(CommandReply {
                text: format!("Saved park to {}\n", save_path.display()),
                json: json!({"save": save_path, "bytes": bytes.len()}),
                changed_ride: false,
            })
        }
        RideCommand::ExportSession(session_path) => {
            let session_file = export_session(ride, session.simulation())?;
            write_bytes_atomic(
                &session_path,
                serde_json::to_string_pretty(&session_file)
                    .map_err(|error| error.to_string())?
                    .as_bytes(),
            )?;
            Ok(CommandReply {
                text: format!("Saved session to {}\n", session_path.display()),
                json: session_file,
                changed_ride: false,
            })
        }
    }
}

fn reply_state(prefix: String, state: Value, changed_ride: bool) -> CommandReply {
    let text = prefix + &format_state_text(&state);
    CommandReply {
        text,
        json: state,
        changed_ride,
    }
}

fn render_state(session: &mut BuilderSession) -> Result<CommandReply, String> {
    let state = state_value(session)?;
    Ok(CommandReply {
        text: format_state_text(&state),
        json: state,
        changed_ride: false,
    })
}

fn state_value(session: &mut BuilderSession) -> Result<Value, String> {
    let estimates = session.estimates();
    let simulation = session.simulation();
    let construction = simulation
        .construction
        .as_ref()
        .ok_or_else(|| "there is no active construction".to_owned())?;
    let end = construction.end();
    let start = construction.start();
    let end_position = end.position();
    let start_position = start.position();
    let offset_to_station = ((end_position.x - start_position.x).powi(2)
        + (end_position.y - start_position.y).powi(2)
        + (end_position.z - start_position.z).powi(2))
    .sqrt()
        / 4.0;
    let ground_height = simulation
        .world
        .surface(end.x as u16, end.y as u16)
        .corners
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let track = construction_track(session)?;
    let force_preview = estimates.force_preview;
    let preview_status = force_preview.as_ref().map(|preview| preview.status);
    let end_speed = estimates.end_speed;
    let can_finish = simulation.construction_can_finish();
    let finish_reason = simulation.construction_finish_reason();
    Ok(json!({
        "pieceCount": construction.pieces().len().saturating_sub(1),
        "track": track,
        "openEnd": {
            "positionMetres": {"x": end_position.x / 4.0, "y": end_position.y / 4.0, "z": end_position.z / 4.0},
            "heading": format!("{:?}", end.heading),
            "slope": format!("{:?}", end.slope),
            "bank": format!("{:?}", end.bank),
            "heightAboveGroundSteps": end.z - i32::from(ground_height),
            "offsetToStationEntryMetres": offset_to_station,
        },
        "chain": construction.chain(),
        "predictedEndSpeed": end_speed,
        "previewStatus": preview_status,
        "canClose": simulation.construction_can_close(),
        "canFinish": can_finish,
        "finishReason": if can_finish { "" } else { &finish_reason },
        "canTest": simulation.construction_can_test(),
    }))
}

fn format_state_text(state: &Value) -> String {
    let end = &state["openEnd"];
    let position = &end["positionMetres"];
    format!(
        "Pieces: {}\nOpen end: ({:.1}, {:.1}, {:.1}) m, heading {}, slope {}, bank {}\nHeight above ground: {} steps; offset to station entry: {:.1} m\nChain: {}; predicted end speed: {} m/s ({})\nCan close: {}; can finish: {}{}; can test: {}\n",
        state["pieceCount"].as_u64().unwrap_or_default(),
        position["x"].as_f64().unwrap_or_default(),
        position["y"].as_f64().unwrap_or_default(),
        position["z"].as_f64().unwrap_or_default(),
        end["heading"].as_str().unwrap_or("unknown"),
        end["slope"].as_str().unwrap_or("unknown"),
        end["bank"].as_str().unwrap_or("unknown"),
        end["heightAboveGroundSteps"].as_i64().unwrap_or_default(),
        end["offsetToStationEntryMetres"].as_f64().unwrap_or_default(),
        state["chain"].as_bool().unwrap_or(false),
        optional_number(state["predictedEndSpeed"].as_f64()),
        state["previewStatus"].as_str().unwrap_or("unavailable"),
        state["canClose"].as_bool().unwrap_or(false),
        state["canFinish"].as_bool().unwrap_or(false),
        state["finishReason"].as_str().filter(|reason| !reason.is_empty()).map_or_else(String::new, |reason| format!(" ({reason})")),
        state["canTest"].as_bool().unwrap_or(false),
    )
}

fn construction_track(session: &BuilderSession) -> Result<Vec<Value>, String> {
    serde_json::from_str(&session.simulation().construction_track())
        .map_err(|error| format!("parse construction track: {error}"))
}

fn render_choices(session: &mut BuilderSession, group: Option<&str>) -> Result<CommandReply, String> {
    let current_selection = session.simulation().construction_selected();
    let base = session.estimates();
    let end_speed = base.end_speed;
    let choices = base.choices;
    let elements = base.elements;
    let mut rows = Vec::new();
    for choice in choices {
        if group.is_some_and(|group| group != choice.group) {
            continue;
        }
        let recommended = session
            .simulation()
            .construction_speed_match(choice.id, end_speed)
            .and_then(|result| result.recommended_id.map(|id| id == choice.id))
            .unwrap_or(false);
        let preview = candidate_for(session, choice.id)?;
        let hill = choice.hill_estimate.as_ref().map(|estimate| json!({
            "entrySpeed": estimate.entry_speed,
            "ratedWindow": estimate.rated_speed.as_ref().map(|window| [window.minimum_entry, window.maximum_entry]),
            "minimumVerticalG": estimate.minimum_vertical_g,
            "maximumVerticalG": estimate.maximum_vertical_g,
            "maximumLateralG": estimate.maximum_absolute_lateral_g,
            "source": estimate.source,
        }));
        rows.push(json!({
            "id": choice.id,
            "name": choice.label,
            "group": choice.group,
            "refusal": choice.refusal,
            "wouldTunnel": choice.would_tunnel,
            "ratedWindow": preview["ratedWindow"],
            "entrySpeedMargin": preview["margin"],
            "speedMatchedRecommendation": recommended,
            "hillPrediction": hill,
        }));
    }
    for element in elements {
        if group.is_some_and(|group| group != "element") {
            continue;
        }
        rows.push(json!({
            "id": element.id,
            "name": element.name,
            "group": "element",
            "refusal": if element.would_tunnel { "would tunnel" } else { "" },
            "wouldTunnel": element.would_tunnel,
            "ratedWindow": null,
            "entrySpeedMargin": null,
            "speedMatchedRecommendation": false,
        }));
    }
    restore_selection(session, &current_selection)?;
    let mut text = String::from("id                         name                         group       refusal                 tunnel window     margin matched\n");
    for row in &rows {
        writeln!(
            text,
            "{:<26} {:<28} {:<11} {:<23} {:<6} {:<12} {:>6} {}",
            row["id"].as_str().unwrap_or(""),
            row["name"].as_str().unwrap_or(""),
            row["group"].as_str().unwrap_or(""),
            row["refusal"].as_str().unwrap_or(""),
            if row["wouldTunnel"].as_bool().unwrap_or(false) { "yes" } else { "no" },
            format_window(&row["ratedWindow"]),
            row["entrySpeedMargin"].as_f64().map_or_else(|| "—".to_owned(), |value| format!("{value:+.1}")),
            if row["speedMatchedRecommendation"].as_bool().unwrap_or(false) { "yes" } else { "no" },
        )
        .unwrap();
    }
    Ok(CommandReply {
        text,
        json: Value::Array(rows),
        changed_ride: false,
    })
}

fn render_estimates(session: &mut BuilderSession, ids: &[String]) -> Result<CommandReply, String> {
    let current_selection = session.simulation().construction_selected();
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        rows.push(candidate_for(session, id)?);
    }
    restore_selection(session, &current_selection)?;
    let mut text = String::from("piece                      entry m/s window      margin  vertical g      lateral g long g status\n");
    for row in &rows {
        let peaks = &row["peaks"];
        writeln!(
            text,
            "{:<26} {:>8} {:<11} {:>6} {:>14} {:>11} {:>7} {}",
            row["id"].as_str().unwrap_or(""),
            optional_number(row["entrySpeed"].as_f64()),
            format_window(&row["ratedWindow"]),
            row["margin"].as_f64().map_or_else(|| "—".to_owned(), |value| format!("{value:+.1}")),
            format!("{}/{}", optional_number(peaks["maximumVerticalG"].as_f64()), optional_number(peaks["minimumVerticalG"].as_f64())),
            optional_number(peaks["maximumLateralG"].as_f64()),
            optional_number(peaks["maximumLongitudinalG"].as_f64()),
            row["status"].as_str().unwrap_or("unavailable"),
        )
        .unwrap();
        if let Some(hill) = row.get("hillPrediction") {
            if !hill.is_null() {
                writeln!(text, "  hill prediction: {hill}").unwrap();
            }
        }
    }
    Ok(CommandReply {
        text,
        json: Value::Array(rows),
        changed_ride: false,
    })
}

fn candidate_for(session: &mut BuilderSession, id: &str) -> Result<Value, String> {
    session.select(id, true).map_err(|error| error.to_string())?;
    let estimates = session.estimates();
    let candidate = estimates
        .candidate_preview
        .as_ref()
        .ok_or_else(|| format!("candidate preview is unavailable for {id}"))?;
    let hill = estimates
        .choices
        .iter()
        .find(|choice| choice.id == id)
        .and_then(|choice| choice.hill_estimate.as_ref())
        .map(|estimate| json!({
            "entrySpeed": estimate.entry_speed,
            "ratedWindow": estimate.rated_speed.as_ref().map(|window| [window.minimum_entry, window.maximum_entry]),
            "minimumVerticalG": estimate.minimum_vertical_g,
            "maximumVerticalG": estimate.maximum_vertical_g,
            "maximumLateralG": estimate.maximum_absolute_lateral_g,
            "source": estimate.source,
        }));
    Ok(candidate_value(id, candidate, hill))
}

fn candidate_value(
    id: &str,
    candidate: &ConstructionCandidateForcePreviewWire,
    hill: Option<Value>,
) -> Value {
    let peaks = candidate.peaks.as_ref().map(|peaks| json!({
        "maximumSpeed": peaks.maximum_speed,
        "minimumVerticalG": peaks.minimum_vertical_g,
        "maximumVerticalG": peaks.maximum_vertical_g,
        "maximumLateralG": peaks.maximum_absolute_lateral_g,
        "maximumLongitudinalG": peaks.maximum_absolute_longitudinal_g,
    }));
    let rated_window = candidate
        .rated_speed
        .as_ref()
        .map(|window| [window.minimum_entry, window.maximum_entry]);
    let margin = candidate.entry_speed.zip(candidate.rated_speed.as_ref()).map(|(speed, window)| {
        (speed - window.minimum_entry).min(window.maximum_entry - speed)
    });
    let ratings = candidate.peaks.as_ref().map(|peaks| {
        json!({
            "airtime": classify_rating(&candidate.limits.rating_bands.airtime, -peaks.minimum_vertical_g),
            "vertical": classify_rating(&candidate.limits.rating_bands.vertical, peaks.maximum_vertical_g),
            "lateral": classify_rating(&candidate.limits.rating_bands.lateral, peaks.maximum_absolute_lateral_g),
            "longitudinal": classify_rating(&candidate.limits.rating_bands.longitudinal, peaks.maximum_absolute_longitudinal_g),
        })
    });
    let mut value = json!({
        "id": id,
        "entrySpeed": candidate.entry_speed,
        "ratedWindow": rated_window,
        "margin": margin,
        "peaks": peaks,
        "ratings": ratings,
        "status": candidate.status,
        "samples": candidate.samples,
    });
    if let Some(hill) = hill {
        value["hillPrediction"] = hill;
    }
    value
}

fn classify_rating(bands: &[ConstructionForceRatingBandWire], value: f64) -> String {
    bands
        .iter()
        .find(|band| match band.up_to {
            None => true,
            Some(limit) if band.inclusive => value <= limit,
            Some(limit) => value < limit,
        })
        .map(|band| band.rating.clone())
        .expect("last force rating band covers every value")
}

fn restore_selection(session: &mut BuilderSession, id: &str) -> Result<(), String> {
    if !id.is_empty() {
        session.select(id, true).map_err(|error| error.to_string())?;
        session.estimates();
    }
    Ok(())
}

fn format_window(value: &Value) -> String {
    value.as_array().map_or_else(
        || "—".to_owned(),
        |window| format!("{:.1}-{:.1}", window[0].as_f64().unwrap_or_default(), window[1].as_f64().unwrap_or_default()),
    )
}

fn format_peaks(peaks: Option<&ConstructionLastPlacedForceWire>) -> String {
    peaks.map_or_else(|| "preview unavailable".to_owned(), |peaks| {
        format!("v+ {:.2} g, v- {:.2} g, lateral {:.2} g, longitudinal {:.2} g", peaks.maximum_vertical_g, peaks.minimum_vertical_g, peaks.maximum_absolute_lateral_g, peaks.maximum_absolute_longitudinal_g)
    })
}

fn render_map(session: &BuilderSession) -> Result<CommandReply, String> {
    let construction = session
        .simulation()
        .construction
        .as_ref()
        .ok_or_else(|| "there is no active construction".to_owned())?;
    let mut samples = Vec::new();
    for (piece_index, piece) in construction.pieces().iter().enumerate() {
        let sample_count = (piece.length() / 8.0).ceil().max(1.0) as usize;
        for sample_index in 0..=sample_count {
            let position = piece.sample(piece.length() * sample_index as f64 / sample_count as f64).position / 4.0;
            samples.push((piece_index, position.x, position.y, position.z));
        }
    }
    if samples.is_empty() {
        return Err("construction has no track samples".into());
    }
    let scale = 4.0;
    let min_x = samples.iter().map(|sample| sample.1.floor() as i32).min().unwrap();
    let max_x = samples.iter().map(|sample| sample.1.floor() as i32).max().unwrap();
    let min_y = samples.iter().map(|sample| sample.2.floor() as i32).min().unwrap();
    let max_y = samples.iter().map(|sample| sample.2.floor() as i32).max().unwrap();
    let mut cells = BTreeMap::new();
    for (piece_index, x, y, _) in &samples {
        cells.insert(((*x / scale).floor() as i32, (*y / scale).floor() as i32), if *piece_index == 0 { 'S' } else { '#' });
    }
    let mut text = String::from("Footprint (each cell is about 4 m; S is the station):\n");
    for y in (min_y / scale as i32..=max_y / scale as i32).rev() {
        for x in min_x / scale as i32..=max_x / scale as i32 {
            text.push(*cells.get(&(x, y)).unwrap_or(&'.'));
        }
        text.push('\n');
    }
    text.push_str("Height profile (piece index, end height in metres):\n");
    let mut profile = Vec::new();
    for (piece_index, piece) in construction.pieces().iter().enumerate() {
        let height = piece.exit.position().z / 4.0;
        writeln!(text, "{piece_index:>3} {height:>7.1}").unwrap();
        profile.push(json!({"pieceIndex": piece_index, "heightMetres": height}));
    }
    let footprint: Vec<_> = text
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with("Height profile"))
        .collect();
    let json = json!({"scaleMetres": scale, "footprint": footprint, "heightProfile": profile});
    Ok(CommandReply {
        text,
        json,
        changed_ride: false,
    })
}

fn test_ride(ride: &RideFile) -> Result<(TestRun, Vec<RidePieceEstimate>), String> {
    let (mut session, estimates) = replay_with_estimates(ride)?;
    session.test().map_err(|error| error.to_string())?;
    let test_run = session
        .simulation()
        .construction_report
        .as_ref()
        .cloned()
        .ok_or_else(|| "ride test completed without a native report".to_owned())?;
    Ok((test_run, estimates))
}

fn replay_with_estimates(ride: &RideFile) -> Result<(BuilderSession, Vec<RidePieceEstimate>), String> {
    let simulation = WasmSimulation::new(42, 0, "new".into());
    let mut session = BuilderSession::new(simulation);
    let heading = match ride.station.heading {
        RideHeading::East => 0,
        RideHeading::North => 1,
        RideHeading::West => 2,
        RideHeading::South => 3,
    };
    session
        .start(&ride.model, ride.station.x, ride.station.y, ride.station.z, heading)
        .map_err(|error| error.to_string())?;
    session.estimates();
    session
        .set_cars_per_train(ride.cars_per_train)
        .map_err(|error| error.to_string())?;
    session.estimates();
    let mut estimates = Vec::new();
    let mut piece_index = 1;
    for (step_index, step) in ride.steps.iter().enumerate() {
        match step {
            RideStep::Place(place) => {
                session.select(&place.place, true).map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                let placement = session.estimates();
                let candidate = placement
                    .candidate_preview
                    .as_ref()
                    .ok_or_else(|| format!("ride step {}: candidate preview unavailable", step_index + 1))?;
                estimates.push(ride_piece_estimate(piece_index, &place.place, candidate, place.note.clone()));
                session.place().map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                if let Some(speed) = place.launch_speed {
                    let track = construction_track(&session)?;
                    let bridge_id = track.last().and_then(|piece| piece["id"].as_u64()).map(|id| id as usize)
                        .ok_or_else(|| format!("ride step {}: launch has no bridge piece id", step_index + 1))?;
                    session.set_launch_speed(bridge_id, speed).map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                    session.estimates();
                }
                piece_index += 1;
            }
            RideStep::Chain(chain) => {
                session.set_chain(chain.chain).map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                session.estimates();
            }
            RideStep::Close(close) => {
                if !close.close {
                    return Err(format!("ride step {}: close must be true", step_index + 1));
                }
                session.close().map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                if session.simulation().construction_train_count() != usize::from(ride.trains) {
                    session.set_train_count(usize::from(ride.trains)).map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                    session.estimates();
                }
            }
        }
    }
    Ok((session, estimates))
}

fn ride_piece_estimate(
    piece_index: usize,
    piece_id: &str,
    candidate: &ConstructionCandidateForcePreviewWire,
    note: Option<String>,
) -> RidePieceEstimate {
    let estimated = candidate.peaks.as_ref().map(|peaks| ForcePeakValues {
        minimum_vertical_g: peaks.minimum_vertical_g,
        maximum_vertical_g: peaks.maximum_vertical_g,
        maximum_lateral_g: peaks.maximum_absolute_lateral_g,
        maximum_longitudinal_g: peaks.maximum_absolute_longitudinal_g,
    });
    let ratings = candidate.peaks.as_ref().map(|peaks| ForceRatings {
        airtime: classify_rating(&candidate.limits.rating_bands.airtime, -peaks.minimum_vertical_g),
        vertical: classify_rating(&candidate.limits.rating_bands.vertical, peaks.maximum_vertical_g),
        lateral: classify_rating(&candidate.limits.rating_bands.lateral, peaks.maximum_absolute_lateral_g),
        longitudinal: classify_rating(&candidate.limits.rating_bands.longitudinal, peaks.maximum_absolute_longitudinal_g),
    });
    RidePieceEstimate {
        piece_index,
        piece_id: piece_id.to_owned(),
        entry_speed: candidate.entry_speed,
        rated_window: candidate.rated_speed.as_ref().map(|window| (window.minimum_entry, window.maximum_entry)),
        estimated,
        ratings,
        preview_status: candidate.status.to_owned(),
        notes: note.into_iter().collect(),
    }
}

fn compare_estimates(ride: &RideFile, test_run: &TestRun, estimates: &[RidePieceEstimate]) -> Result<AgreementReport, String> {
    let model = validate_model(&ride.model)?;
    let measured: BTreeMap<_, _> = test_run.piece_force_peaks().into_iter().map(|peaks| (peaks.piece_index, peaks)).collect();
    let estimate_by_piece: BTreeMap<_, _> = estimates.iter().map(|estimate| (estimate.piece_index, estimate)).collect();
    let finished_track = finished_track_estimates(ride, estimates)?;
    let mut inputs = Vec::new();
    for (piece_index, (piece_name, finished_entry, finished_peak)) in finished_track {
        let placement = estimate_by_piece.get(&piece_index).copied();
        let measured_peaks = measured.get(&piece_index);
        let entry_measured = test_run.samples.iter().find(|sample| sample.piece_index == piece_index).map(|sample| sample.speed);
        inputs.push(AgreementInput {
            piece_index,
            piece_name,
            model_name: model.name.to_string(),
            entry_speed_estimate: finished_entry,
            entry_speed_measured_front: entry_measured,
            placement_estimate: placement.and_then(|estimate| estimate.estimated).map(force_peaks),
            finished_track_estimate: finished_peak,
            measured_front: measured_peaks.and_then(|peaks| peaks.front_car).map(force_peaks),
            measured_worst: measured_peaks.and_then(|peaks| peaks.worst_car).map(force_peaks),
            limits: model.comfortable_force_limits,
        });
    }
    Ok(evaluate_agreement(&inputs))
}

fn finished_track_estimates(ride: &RideFile, placement: &[RidePieceEstimate]) -> Result<BTreeMap<usize, (String, Option<f64>, Option<ForcePeaks>)>, String> {
    let mut session = create_session(ride)?;
    let preview = session
        .estimates()
        .force_preview
        .ok_or_else(|| "finished-track preview is unavailable".to_owned())?;
    let construction = session
        .simulation()
        .construction
        .as_ref()
        .ok_or_else(|| "there is no active construction".to_owned())?;
    let pieces = construction.pieces();
    let mut piece_starts = Vec::with_capacity(pieces.len());
    let mut distance = 0.0;
    for piece in pieces {
        piece_starts.push((distance, distance + piece.length(), piece.name.to_owned()));
        distance += piece.length();
    }
    let mut result = BTreeMap::new();
    for (piece_index, (start, end, name)) in piece_starts.into_iter().enumerate().skip(1) {
        let samples: Vec<_> = preview.samples.iter().filter(|sample| sample.distance >= start && sample.distance <= end).collect();
        let peaks: Option<ForcePeakValues> = samples.iter().fold(None, |peaks, sample| {
            let current = ForcePeakValues {
                minimum_vertical_g: sample.vertical_g,
                maximum_vertical_g: sample.vertical_g,
                maximum_lateral_g: sample.lateral_g.abs(),
                maximum_longitudinal_g: sample.longitudinal_g.abs(),
            };
            Some(match peaks {
                None => current,
                Some(mut peaks) => {
                    peaks.minimum_vertical_g = peaks.minimum_vertical_g.min(current.minimum_vertical_g);
                    peaks.maximum_vertical_g = peaks.maximum_vertical_g.max(current.maximum_vertical_g);
                    peaks.maximum_lateral_g = peaks.maximum_lateral_g.max(current.maximum_lateral_g);
                    peaks.maximum_longitudinal_g = peaks.maximum_longitudinal_g.max(current.maximum_longitudinal_g);
                    peaks
                }
            })
        });
        let entry_speed = preview.samples.iter().find(|sample| sample.distance >= start).map(|sample| sample.speed);
        let measured_name = placement.iter().find(|estimate| estimate.piece_index == piece_index).map_or(name.clone(), |estimate| estimate.piece_id.clone());
        result.insert(piece_index, (measured_name, entry_speed, peaks.map(force_peaks)));
    }
    Ok(result)
}

fn force_peaks(peaks: ForcePeakValues) -> ForcePeaks {
    ForcePeaks {
        minimum_vertical_g: peaks.minimum_vertical_g,
        maximum_vertical_g: peaks.maximum_vertical_g,
        maximum_lateral_g: peaks.maximum_lateral_g,
        maximum_longitudinal_g: peaks.maximum_longitudinal_g,
    }
}

fn format_agreement(report: &AgreementReport) -> String {
    let mut text = String::from("piece check 2 check 3\n");
    for row in &report.rows {
        writeln!(text, "{} {:<24} {:?} {:?}", row.piece_index, row.piece_name, row.check_2, row.check_3).unwrap();
    }
    for failure in &report.failures {
        writeln!(text, "{} {:?}: {} ({})", failure.piece_index, failure.check, failure.metric, failure.reason).unwrap();
    }
    writeln!(text, "Agreement: {}", if report.passed { "pass" } else { "fail" }).unwrap();
    text
}

fn agreement_json(report: &AgreementReport) -> Value {
    let rows: Vec<_> = report.rows.iter().map(|row| json!({
        "pieceIndex": row.piece_index,
        "pieceName": row.piece_name,
        "check2": format!("{:?}", row.check_2),
        "check3": format!("{:?}", row.check_3),
    })).collect();
    let failures: Vec<_> = report.failures.iter().map(|failure| json!({
        "pieceIndex": failure.piece_index,
        "pieceName": failure.piece_name,
        "check": format!("{:?}", failure.check),
        "metric": failure.metric,
        "reason": failure.reason,
    })).collect();
    json!({"passed": report.passed, "rows": rows, "failures": failures})
}

fn run_check(paths: &[String], json_mode: bool) -> Result<String, String> {
    if paths.is_empty() {
        return Err("usage: tools/design check <ride file> ...".into());
    }
    let mut rows = Vec::new();
    let mut text = String::new();
    let mut expectation_failures = Vec::new();
    for path in paths {
        let bytes = fs::read(path).map_err(|error| format!("read ride {path}: {error}"))?;
        let ride: RideFile = serde_json::from_slice(&bytes).map_err(|error| format!("parse ride {path}: {error}"))?;
        let (test_run, estimates) = test_ride(&ride).map_err(|error| format!("{path}: {error}"))?;
        let agreement = compare_estimates(&ride, &test_run, &estimates)?;
        let failures = check_expectations(&ride, &test_run);
        for failure in &failures {
            expectation_failures.push(format!("{path}: {failure}"));
        }
        writeln!(text, "{}: expectations {}; native agreement {} (informational)", ride.name, if failures.is_empty() { "pass" } else { "fail" }, if agreement.passed { "pass" } else { "fail" }).unwrap();
        for failure in &failures {
            writeln!(text, "  {failure}").unwrap();
        }
        rows.push(json!({
            "path": path,
            "name": ride.name,
            "expectationFailures": failures,
            "agreement": agreement_json(&agreement),
        }));
    }
    if !expectation_failures.is_empty() {
        return Err(format!("{}\nExpectation checks failed:\n{}", text, expectation_failures.join("\n")));
    }
    if json_mode {
        return serde_json::to_string_pretty(&Value::Array(rows))
            .map(|value| value + "\n")
            .map_err(|error| error.to_string());
    }
    Ok(text)
}

fn check_expectations(ride: &RideFile, test_run: &TestRun) -> Vec<String> {
    let Some(expect) = &ride.expect else {
        return Vec::new();
    };
    let mut failures = Vec::new();
    if let Some([minimum, maximum]) = expect.excitement {
        if !(minimum..=maximum).contains(&test_run.excitement) {
            failures.push(format!("excitement expected {minimum:.2}-{maximum:.2}, observed {:.2}", test_run.excitement));
        }
    }
    if let Some([minimum, maximum]) = expect.sustained_max_vertical_g {
        let observed = test_run.statistics.max_positive_vertical_g;
        if !(minimum..=maximum).contains(&observed) {
            failures.push(format!("maximum vertical g expected {minimum:.2}-{maximum:.2}, observed {observed:.2}"));
        }
    }
    if let Some([minimum, maximum]) = expect.sustained_min_vertical_g {
        let observed = test_run.statistics.max_negative_vertical_g;
        if !(minimum..=maximum).contains(&observed) {
            failures.push(format!("minimum vertical g expected {minimum:.2}-{maximum:.2}, observed {observed:.2}"));
        }
    }
    if let Some(maximum) = expect.max_lateral_g {
        if test_run.statistics.max_lateral_g > maximum {
            failures.push(format!("maximum lateral g expected at most {maximum:.2}, observed {:.2}", test_run.statistics.max_lateral_g));
        }
    }
    if let Some(inversions) = expect.inversions {
        if test_run.statistics.inversions != inversions as usize {
            failures.push(format!("inversions expected {inversions}, observed {}", test_run.statistics.inversions));
        }
    }
    if let Some(minimum) = expect.airtime_moments_at_least {
        if test_run.statistics.airtime_moments < minimum as usize {
            failures.push(format!("airtime moments expected at least {minimum}, observed {}", test_run.statistics.airtime_moments));
        }
    }
    failures
}

fn export_session(ride: &RideFile, simulation: &WasmSimulation) -> Result<Value, String> {
    let mut commands = vec![
        json!({"tick": 0, "command": {"type": "construction-select", "piece": format!("model:{}", ride.model)}}),
        json!({"tick": 0, "command": {"type": "construction-start", "x": ride.station.x, "y": ride.station.y, "z": ride.station.z, "heading": match ride.station.heading { RideHeading::East => 0, RideHeading::North => 1, RideHeading::West => 2, RideHeading::South => 3 }}}),
        json!({"tick": 0, "command": {"type": "construction-set-cars-per-train", "cars": ride.cars_per_train}}),
    ];
    let mut piece_index = 0;
    for step in &ride.steps {
        match step {
            RideStep::Place(place) => {
                commands.push(json!({"tick": 0, "command": {"type": "construction-select", "piece": place.place, "exact": true}}));
                commands.push(json!({"tick": 0, "command": {"type": "construction-append"}}));
                if let Some(target_speed) = place.launch_speed {
                    let track: Vec<Value> = serde_json::from_str(&simulation.construction_track()).map_err(|error| error.to_string())?;
                    let id = track.get(piece_index + 1).and_then(|piece| piece["id"].as_u64()).ok_or_else(|| format!("ride step {} has no bridge piece id", piece_index + 1))?;
                    commands.push(json!({"tick": 0, "command": {"type": "construction-set-launch-intensity", "id": id, "targetSpeed": target_speed}}));
                }
                piece_index += 1;
            }
            RideStep::Chain(chain) => commands.push(json!({"tick": 0, "command": {"type": "construction-set-chain", "enabled": chain.chain}})),
            RideStep::Close(close) if close.close => {
                commands.push(json!({"tick": 0, "command": {"type": "construction-close"}}));
                if ride.trains != 1 {
                    commands.push(json!({"tick": 0, "command": {"type": "construction-set-train-count", "count": ride.trains}}));
                }
            }
            RideStep::Close(_) => return Err("ride file close step must be true".into()),
        }
    }
    Ok(json!({
        "format": "theme-park-session",
        "version": 1,
        "seed": "42",
        "park": "new",
        "guestCount": 0,
        "commands": commands,
        "checkpoints": [],
        "finalTick": 0,
    }))
}

fn write_ride_atomic(path: &Path, ride: &RideFile) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(ride).map_err(|error| error.to_string())?;
    write_bytes_atomic(path, &bytes)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("output");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temporary_path)
        .map_err(|error| format!("create {}: {error}", temporary_path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", temporary_path.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", temporary_path.display()))?;
    fs::rename(&temporary_path, path)
        .map_err(|error| format!("replace {}: {error}", path.display()))
}

fn optional_number(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:.1}"))
}
