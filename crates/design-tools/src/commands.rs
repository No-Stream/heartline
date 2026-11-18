use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use theme_park_sim::physics::{ForcePeakValues, TestRun};
use theme_park_sim::track::blocks::BlockSectionPlacement;
use theme_park_sim::track::ride_file::{
    RideBlockSectionsStep, RideChainStep, RideCloseStep, RideFile, RideFileFormat, RideHeading,
    RidePlaceStep, RideStation, RideStep,
};
use theme_park_sim::track::{CoasterModel, coaster_models};
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::{BuilderEstimates, BuilderSession};
use theme_park_sim_wasm::construction::{
    ConstructionCandidateForcePreviewWire, ConstructionForceRatingBandWire,
    ConstructionLastPlacedForceWire,
};

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

const REPLAY_SNAPSHOT_MAGIC: &[u8] = b"TPRS";
const REPLAY_SNAPSHOT_VERSION: u8 = 1;
const AUTO_BLOCKS_WORK_BUDGET: u32 = 1_000;

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
        "park" => Err(
            "park build/report require the park description implementation (work items 8–9)".into(),
        ),
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
        let mut parameters: Vec<String> = inline_words[1..]
            .iter()
            .map(|word| (*word).to_owned())
            .collect();
        let mut end = index + 1;
        while end < arguments.len() {
            let next_words: Vec<_> = arguments[end].split_whitespace().collect();
            if next_words
                .first()
                .is_some_and(|next| COMMAND_NAMES.contains(next))
            {
                break;
            }
            parameters.push(arguments[end].clone());
            end += 1;
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
                station: station
                    .ok_or_else(|| "new requires --station x,y,z,heading".to_owned())?,
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
        "cars" if parameters.len() == 1 => {
            Ok(RideCommand::Cars(parse_number(&parameters[0], "cars")?))
        }
        "trains" if parameters.len() == 1 => {
            Ok(RideCommand::Trains(parse_number(&parameters[0], "trains")?))
        }
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
                    parameters
                        .get(index)
                        .ok_or("--launch-speed requires a value")?,
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
    let mut ride_source_bytes = None;
    let mut ride = if file_exists {
        let bytes =
            fs::read(&path).map_err(|error| format!("read ride {}: {error}", path.display()))?;
        let ride = serde_json::from_slice::<RideFile>(&bytes)
            .map_err(|error| format!("parse ride {}: {error}", path.display()))?;
        ride_source_bytes = Some(bytes);
        ride
    } else {
        match commands.next() {
            Some(RideCommand::New {
                model,
                station,
                cars,
            }) => {
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
            Some(_) => {
                return Err(format!(
                    "ride file {} does not exist; start with new",
                    path.display()
                ));
            }
            None => return Err(format!("ride file {} does not exist", path.display())),
        }
    };

    let mut cache_needs_write = !file_exists;
    let (mut session, mut replay_estimates, mut state_cache) =
        if let Some(ride_bytes) = ride_source_bytes.as_deref() {
            let binary_fingerprint = running_binary_fingerprint()?;
            let snapshot_path = replay_snapshot_path(&path);
            if let Some(snapshot) =
                load_replay_snapshot(&snapshot_path, ride_bytes, binary_fingerprint)?
            {
                (snapshot.session, None, Some(snapshot.state))
            } else {
                cache_needs_write = true;
                let (session, estimates) = create_session_with_estimates(&ride)?;
                (session, Some(estimates), None)
            }
        } else {
            let (session, estimates) = create_session_with_estimates(&ride)?;
            (session, Some(estimates), None)
        };

    let mut replies = Vec::new();
    let mut changed_ride = !file_exists;
    for command in commands {
        let is_state_command = matches!(command, RideCommand::State);
        let result = if is_state_command {
            match replay_estimates.take() {
                Some(estimates) => render_state_with_estimates(&session, estimates),
                None => match state_cache.as_ref() {
                    Some(state) => Ok(render_state_value(state.clone())),
                    None => render_state(&mut session),
                },
            }
        } else {
            execute_command(command, &mut ride, &mut session)
        };
        match result {
            Ok(reply) => {
                if reply.changed_ride {
                    changed_ride = true;
                    cache_needs_write = true;
                    replay_estimates = None;
                    state_cache = None;
                } else if is_state_command {
                    state_cache = Some(reply.json.clone());
                }
                replies.push(reply);
            }
            Err(error) => {
                let current_state = render_state(&mut session)
                    .map_err(|state_error| {
                        format!("{error}\nCould not render state at failure: {state_error}")
                    })?
                    .text;
                return Err(format!("{error}\nState at failure:\n{current_state}"));
            }
        }
    }
    if replies.is_empty() {
        let reply = match (replay_estimates.take(), state_cache.take()) {
            (Some(estimates), None) => render_state_with_estimates(&session, estimates)?,
            (None, Some(state)) => render_state_value(state),
            (None, None) => render_state(&mut session)?,
            (Some(_), Some(_)) => unreachable!("replay estimates and cached state are exclusive"),
        };
        state_cache = Some(reply.json.clone());
        replies.push(reply);
    }
    if state_cache.is_none() {
        state_cache = Some(match replay_estimates.take() {
            Some(estimates) => state_value_with_estimates(&session, estimates)?,
            None => state_value(&mut session)?,
        });
    }
    if changed_ride {
        write_ride_atomic(&path, &ride)?;
    }
    if cache_needs_write {
        let ride_bytes = match ride_source_bytes {
            Some(bytes) if !changed_ride => bytes,
            _ => fs::read(&path)
                .map_err(|error| format!("read ride {} after writing: {error}", path.display()))?,
        };
        let binary_fingerprint = running_binary_fingerprint()?;
        let session_bytes = session.save_native_snapshot()?;
        let snapshot_bytes = encode_replay_snapshot(
            &ride_bytes,
            binary_fingerprint,
            &session_bytes,
            state_cache
                .as_ref()
                .expect("ride replay snapshot has its final state"),
        )?;
        write_bytes_atomic(&replay_snapshot_path(&path), &snapshot_bytes)?;
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
    create_session_with_estimates(ride).map(|(session, _)| session)
}

fn create_session_with_estimates(
    ride: &RideFile,
) -> Result<(BuilderSession, BuilderEstimates), String> {
    let simulation = WasmSimulation::new(42, 0, "new".into());
    let mut session = BuilderSession::new(simulation);
    let estimates = session
        .replay_ride(ride)
        .map_err(|error| error.to_string())?;
    Ok((session, estimates))
}

struct LoadedReplaySnapshot {
    session: BuilderSession,
    state: Value,
}

fn running_binary_fingerprint() -> Result<u64, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("locate design tool executable: {error}"))?;
    let bytes = fs::read(&executable).map_err(|error| {
        format!(
            "read design tool executable {}: {error}",
            executable.display()
        )
    })?;
    Ok(snapshot_checksum(&bytes))
}

fn replay_snapshot_path(ride_path: &Path) -> PathBuf {
    let mut file_name: OsString = ride_path.as_os_str().to_owned();
    file_name.push(".snapshot");
    PathBuf::from(file_name)
}

fn encode_replay_snapshot(
    ride_bytes: &[u8],
    binary_fingerprint: u64,
    session_bytes: &[u8],
    state: &Value,
) -> Result<Vec<u8>, String> {
    let state_bytes = serde_json::to_vec(state).map_err(|error| error.to_string())?;
    let mut payload = binary_fingerprint.to_le_bytes().to_vec();
    append_snapshot_field(&mut payload, ride_bytes);
    append_snapshot_field(&mut payload, session_bytes);
    append_snapshot_field(&mut payload, &state_bytes);

    let mut bytes = REPLAY_SNAPSHOT_MAGIC.to_vec();
    bytes.push(REPLAY_SNAPSHOT_VERSION);
    bytes.extend(snapshot_checksum(&payload).to_le_bytes());
    bytes.extend(payload);
    Ok(bytes)
}

fn load_replay_snapshot(
    path: &Path,
    ride_bytes: &[u8],
    binary_fingerprint: u64,
) -> Result<Option<LoadedReplaySnapshot>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read replay snapshot {}: {error}", path.display())),
    };
    if !bytes.starts_with(REPLAY_SNAPSHOT_MAGIC) {
        return Err(format!(
            "invalid replay snapshot header in {}",
            path.display()
        ));
    }
    let version = *bytes
        .get(REPLAY_SNAPSHOT_MAGIC.len())
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    if version != REPLAY_SNAPSHOT_VERSION {
        return Ok(None);
    }
    let data = bytes
        .get(REPLAY_SNAPSHOT_MAGIC.len() + 1..)
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    let (stored_checksum, payload) = data
        .split_at_checked(8)
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    let stored_checksum = u64::from_le_bytes(
        stored_checksum
            .try_into()
            .expect("replay snapshot checksum length"),
    );
    if snapshot_checksum(payload) != stored_checksum {
        return Err(format!("replay snapshot {} is corrupt", path.display()));
    }
    let (stored_binary_fingerprint, fields) = payload
        .split_at_checked(8)
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    let stored_binary_fingerprint = u64::from_le_bytes(
        stored_binary_fingerprint
            .try_into()
            .expect("replay snapshot binary fingerprint length"),
    );
    let mut fields = fields;
    let stored_ride_bytes = take_snapshot_field(&mut fields, path)?;
    let session_bytes = take_snapshot_field(&mut fields, path)?;
    let state_bytes = take_snapshot_field(&mut fields, path)?;
    if !fields.is_empty() {
        return Err(format!(
            "replay snapshot {} has trailing data",
            path.display()
        ));
    }
    if stored_binary_fingerprint != binary_fingerprint || stored_ride_bytes != ride_bytes {
        return Ok(None);
    }
    let state: Value = serde_json::from_slice(state_bytes)
        .map_err(|error| format!("parse replay snapshot state {}: {error}", path.display()))?;
    if !state.is_object() {
        return Err(format!(
            "replay snapshot state in {} is not an object",
            path.display()
        ));
    }
    let session = BuilderSession::load_native_snapshot(session_bytes)
        .map_err(|error| format!("load replay snapshot {}: {error}", path.display()))?;
    Ok(Some(LoadedReplaySnapshot { session, state }))
}

fn append_snapshot_field(payload: &mut Vec<u8>, field: &[u8]) {
    let length = u64::try_from(field.len()).expect("snapshot field length fits in u64");
    payload.extend(length.to_le_bytes());
    payload.extend(field);
}

fn take_snapshot_field<'a>(payload: &mut &'a [u8], path: &Path) -> Result<&'a [u8], String> {
    let (length_bytes, remaining) = payload
        .split_at_checked(8)
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    let length = u64::from_le_bytes(length_bytes.try_into().expect("snapshot length bytes"));
    let length = usize::try_from(length).map_err(|error| {
        format!(
            "replay snapshot {} field is too large: {error}",
            path.display()
        )
    })?;
    let (field, remaining) = remaining
        .split_at_checked(length)
        .ok_or_else(|| format!("replay snapshot {} is incomplete", path.display()))?;
    *payload = remaining;
    Ok(field)
}

fn snapshot_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
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
            let selection = session
                .select(&id, !matched)
                .map_err(|error| error.to_string())?;
            let estimates = session.estimates();
            let candidate = estimates.candidate_preview.as_ref().ok_or_else(|| {
                format!(
                    "candidate preview is unavailable for {}",
                    selection.selected_id
                )
            })?;
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
            writeln!(
                text,
                "Estimate: {} m/s entry, {}",
                optional_number(candidate.entry_speed),
                format_peaks(candidate.peaks.as_ref())
            )
            .unwrap();
            text.push_str(&format_state_text(&state));
            Ok(CommandReply {
                text,
                json: json!({"placed": saved_id, "requested": id, "estimate": placement_json, "state": state}),
                changed_ride: true,
            })
        }
        RideCommand::Chain(enabled) => {
            session
                .set_chain(enabled)
                .map_err(|error| error.to_string())?;
            ride.steps
                .push(RideStep::Chain(RideChainStep { chain: enabled }));
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Chain lift: {}\n", if enabled { "on" } else { "off" }),
                state,
                true,
            ))
        }
        RideCommand::LaunchSpeed {
            piece_index,
            target_speed,
        } => {
            if piece_index == 0 {
                return Err("launch-speed piece index is one-based".into());
            }
            let place_step_index = ride
                .steps
                .iter()
                .enumerate()
                .filter_map(|(step_index, step)| {
                    matches!(step, RideStep::Place(_)).then_some(step_index)
                })
                .nth(piece_index - 1)
                .ok_or_else(|| format!("ride has no placed piece {piece_index}"))?;
            let bridge_ids = session
                .placed_piece_bridge_ids()
                .get(piece_index - 1)
                .ok_or_else(|| format!("ride has no bridge pieces for placement {piece_index}"))?;
            if bridge_ids.len() != 1 {
                return Err(format!(
                    "placement {piece_index} expands into {} bridge pieces; launch speed requires a single-piece launch",
                    bridge_ids.len()
                ));
            }
            let bridge_id = bridge_ids[0];
            session
                .set_launch_speed(bridge_id, target_speed)
                .map_err(|error| error.to_string())?;
            let RideStep::Place(mut place) = ride.steps[place_step_index].clone() else {
                unreachable!("the placement index only selects ride place steps");
            };
            place.launch_speed = Some(target_speed);
            ride.steps[place_step_index] = RideStep::Place(place);
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Launch speed for piece {piece_index}: {target_speed:.1} m/s\n"),
                state,
                true,
            ))
        }
        RideCommand::Cars(cars) => {
            session
                .set_cars_per_train(cars)
                .map_err(|error| error.to_string())?;
            ride.cars_per_train = cars;
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Cars per train: {cars}\n"),
                state,
                true,
            ))
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
            let (search_report, search_work_completed) = session
                .auto_blocks_with_budget(AUTO_BLOCKS_WORK_BUDGET)
                .map_err(|error| error.to_string())?;
            let block_sections: Vec<_> = search_report["sections"]
                .as_array()
                .ok_or_else(|| "block report has no section list".to_owned())?
                .iter()
                .filter(|section| section["editable"].as_bool() == Some(true))
                .map(|section| -> Result<BlockSectionPlacement, String> {
                    let boundary_metres = section["boundaryMetres"]
                        .as_f64()
                        .ok_or_else(|| "block report section has no boundary".to_owned())?;
                    let brake_start_metres = section["brakeStartMetres"]
                        .as_f64()
                        .ok_or_else(|| "editable block section has no brake start".to_owned())?;
                    let brake_piece_indices = section["brakePieceIndices"]
                        .as_array()
                        .ok_or_else(|| {
                            "editable block section has no brake piece indices".to_owned()
                        })?
                        .iter()
                        .map(|index| {
                            usize::try_from(index.as_u64().ok_or_else(|| {
                                "editable block section has an invalid brake piece index".to_owned()
                            })?)
                            .map_err(|error| format!("brake piece index is out of range: {error}"))
                        })
                        .collect::<Result<_, _>>()?;
                    Ok(BlockSectionPlacement {
                        boundary_metres,
                        brake_start_metres,
                        brake_piece_indices,
                    })
                })
                .collect::<Result<_, _>>()?;
            let train_capacity = usize::try_from(
                search_report["trainCapacity"]
                    .as_u64()
                    .ok_or_else(|| "block report has no train capacity".to_owned())?,
            )
            .map_err(|error| format!("block report train capacity is out of range: {error}"))?;
            let capacity_search_complete = search_report["capacitySearchComplete"]
                .as_bool()
                .ok_or_else(|| "block report has no capacity search status".to_owned())?;
            let placement_search_complete = search_report["placementSearchComplete"]
                .as_bool()
                .ok_or_else(|| "block report has no placement search status".to_owned())?;
            let block_report = session
                .install_block_sections(
                    &block_sections,
                    train_capacity,
                    capacity_search_complete,
                    placement_search_complete,
                )
                .map_err(|error| error.to_string())?;
            ride.steps
                .push(RideStep::BlockSections(RideBlockSectionsStep {
                    block_sections: block_sections.clone(),
                    train_capacity,
                    capacity_search_complete,
                    placement_search_complete,
                }));
            let result = json!({
                "blockSections": block_sections,
                "trainCapacity": train_capacity,
                "capacitySearchComplete": capacity_search_complete,
                "placementSearchComplete": placement_search_complete,
                "searchWorkBudget": AUTO_BLOCKS_WORK_BUDGET,
                "searchWorkCompleted": search_work_completed,
                "blockReport": block_report,
            });
            Ok(CommandReply {
                text: format!(
                    "Automatic block report (budget: {AUTO_BLOCKS_WORK_BUDGET} work steps; used: {search_work_completed}):\n{}\n",
                    serde_json::to_string_pretty(&result).unwrap()
                ),
                json: result,
                changed_ride: true,
            })
        }
        RideCommand::Undo(count) => {
            let mut removed = 0;
            for _ in 0..count {
                session.undo().map_err(|error| error.to_string())?;
                if let Some(close_index) = ride
                    .steps
                    .iter()
                    .position(|step| matches!(step, RideStep::Close(_)))
                {
                    ride.steps.remove(close_index);
                }
                let place_index = ride
                    .steps
                    .iter()
                    .rposition(|step| matches!(step, RideStep::Place(_)))
                    .ok_or_else(|| {
                        "undo removed a bridge piece with no matching ride step".to_owned()
                    })?;
                ride.steps.remove(place_index);
                removed += 1;
            }
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Undid {removed} placement(s).\n"),
                state,
                removed > 0,
            ))
        }
        RideCommand::Truncate(step_count) => {
            if step_count > ride.steps.len() {
                return Err(format!(
                    "ride has {} steps; cannot keep {step_count}",
                    ride.steps.len()
                ));
            }
            ride.steps.truncate(step_count);
            *session = create_session(ride)?;
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Kept {step_count} ride step(s).\n"),
                state,
                true,
            ))
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
            Ok(reply_state(
                format!("Finish search added {added} piece(s).\n"),
                state,
                added > 0,
            ))
        }
        RideCommand::Close => {
            session.close().map_err(|error| error.to_string())?;
            ride.steps
                .push(RideStep::Close(RideCloseStep { close: true }));
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
            let session_file = export_session(ride, session)?;
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
    let estimates = session.estimates();
    render_state_with_estimates(session, estimates)
}

fn render_state_with_estimates(
    session: &BuilderSession,
    estimates: BuilderEstimates,
) -> Result<CommandReply, String> {
    let state = state_value_with_estimates(session, estimates)?;
    Ok(render_state_value(state))
}

fn render_state_value(state: Value) -> CommandReply {
    CommandReply {
        text: format_state_text(&state),
        json: state,
        changed_ride: false,
    }
}

fn state_value(session: &mut BuilderSession) -> Result<Value, String> {
    let estimates = session.estimates();
    state_value_with_estimates(session, estimates)
}

fn state_value_with_estimates(
    session: &BuilderSession,
    estimates: BuilderEstimates,
) -> Result<Value, String> {
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
        .expect("surface tiles have four height corners");
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
    let finish_reason = state["finishReason"]
        .as_str()
        .expect("finish reason is a string");
    format!(
        "Pieces: {}\nOpen end: ({:.1}, {:.1}, {:.1}) m, heading {}, slope {}, bank {}\nHeight above ground: {} steps; offset to station entry: {:.1} m\nChain: {}; predicted end speed: {} m/s ({})\nCan close: {}; can finish: {}{}; can test: {}\n",
        state["pieceCount"]
            .as_u64()
            .expect("piece count is an unsigned integer"),
        position["x"].as_f64().expect("open end x is a number"),
        position["y"].as_f64().expect("open end y is a number"),
        position["z"].as_f64().expect("open end z is a number"),
        end["heading"]
            .as_str()
            .expect("open end heading is a string"),
        end["slope"].as_str().expect("open end slope is a string"),
        end["bank"].as_str().expect("open end bank is a string"),
        end["heightAboveGroundSteps"]
            .as_i64()
            .expect("height above ground is an integer"),
        end["offsetToStationEntryMetres"]
            .as_f64()
            .expect("station offset is a number"),
        state["chain"].as_bool().expect("chain state is a boolean"),
        optional_number(state["predictedEndSpeed"].as_f64()),
        state["previewStatus"].as_str().unwrap_or("unavailable"),
        state["canClose"]
            .as_bool()
            .expect("close availability is a boolean"),
        state["canFinish"]
            .as_bool()
            .expect("finish availability is a boolean"),
        if finish_reason.is_empty() {
            String::new()
        } else {
            format!(" ({finish_reason})")
        },
        state["canTest"]
            .as_bool()
            .expect("test availability is a boolean"),
    )
}

fn construction_track(session: &BuilderSession) -> Result<Vec<Value>, String> {
    serde_json::from_str(&session.simulation().construction_track())
        .map_err(|error| format!("parse construction track: {error}"))
}

fn render_choices(
    session: &mut BuilderSession,
    group: Option<&str>,
) -> Result<CommandReply, String> {
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
        let preview = candidate_for(session, choice.id, true)?;
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
    let mut text = String::from(
        "id                         name                         group       refusal                 tunnel window     margin matched\n",
    );
    for row in &rows {
        writeln!(
            text,
            "{:<26} {:<28} {:<11} {:<23} {:<6} {:<12} {:>6} {}",
            row["id"].as_str().expect("choice id is a string"),
            row["name"].as_str().expect("choice name is a string"),
            row["group"].as_str().expect("choice group is a string"),
            row["refusal"].as_str().expect("choice refusal is a string"),
            if row["wouldTunnel"]
                .as_bool()
                .expect("choice tunnel flag is a boolean")
            {
                "yes"
            } else {
                "no"
            },
            format_window(&row["ratedWindow"]),
            row["entrySpeedMargin"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), |value| format!("{value:+.1}")),
            if row["speedMatchedRecommendation"]
                .as_bool()
                .expect("choice recommendation flag is a boolean")
            {
                "yes"
            } else {
                "no"
            },
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
        rows.push(candidate_for(session, id, false)?);
    }
    restore_selection(session, &current_selection)?;
    let mut text = String::from(
        "piece                      entry m/s window      margin  vertical g (rating)                       lateral g (rating)   long g (rating) status\n",
    );
    for row in &rows {
        let peaks = &row["peaks"];
        let ratings = &row["ratings"];
        let vertical = format!(
            "{} ({}) / {} ({})",
            optional_number(peaks["maximumVerticalG"].as_f64()),
            optional_rating(ratings, "vertical"),
            optional_number(peaks["minimumVerticalG"].as_f64()),
            optional_rating(ratings, "airtime"),
        );
        let lateral = format!(
            "{} ({})",
            optional_number(peaks["maximumLateralG"].as_f64()),
            optional_rating(ratings, "lateral"),
        );
        let longitudinal = format!(
            "{} ({})",
            optional_number(peaks["maximumLongitudinalG"].as_f64()),
            optional_rating(ratings, "longitudinal"),
        );
        writeln!(
            text,
            "{:<26} {:>8} {:<11} {:>6} {:<42} {:<18} {:<18} {}",
            row["id"].as_str().expect("candidate id is a string"),
            optional_number(row["entrySpeed"].as_f64()),
            format_window(&row["ratedWindow"]),
            row["margin"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), |value| format!("{value:+.1}")),
            vertical,
            lateral,
            longitudinal,
            row["status"]
                .as_str()
                .expect("candidate status is a string"),
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

fn candidate_for(session: &mut BuilderSession, id: &str, exact: bool) -> Result<Value, String> {
    let selection = session
        .select(id, exact)
        .map_err(|error| error.to_string())?;
    let estimates = session.estimates();
    let candidate = estimates.candidate_preview.as_ref().ok_or_else(|| {
        format!(
            "candidate preview is unavailable for {}",
            selection.selected_id
        )
    })?;
    let hill = estimates
        .choices
        .iter()
        .find(|choice| choice.id == selection.selected_id)
        .and_then(|choice| choice.hill_estimate.as_ref())
        .map(|estimate| json!({
            "entrySpeed": estimate.entry_speed,
            "ratedWindow": estimate.rated_speed.as_ref().map(|window| [window.minimum_entry, window.maximum_entry]),
            "minimumVerticalG": estimate.minimum_vertical_g,
            "maximumVerticalG": estimate.maximum_vertical_g,
            "maximumLateralG": estimate.maximum_absolute_lateral_g,
            "source": estimate.source,
        }));
    let mut value = candidate_value(&selection.selected_id, candidate, hill);
    if selection.selected_id != id {
        value["requestedId"] = json!(id);
    }
    Ok(value)
}

fn optional_rating<'a>(ratings: &'a Value, key: &str) -> &'a str {
    ratings[key].as_str().unwrap_or("—")
}

fn candidate_value(
    id: &str,
    candidate: &ConstructionCandidateForcePreviewWire,
    hill: Option<Value>,
) -> Value {
    let peaks = candidate.peaks.as_ref().map(|peaks| {
        json!({
            "maximumSpeed": peaks.maximum_speed,
            "minimumVerticalG": peaks.minimum_vertical_g,
            "maximumVerticalG": peaks.maximum_vertical_g,
            "maximumLateralG": peaks.maximum_absolute_lateral_g,
            "maximumLongitudinalG": peaks.maximum_absolute_longitudinal_g,
        })
    });
    let rated_window = candidate
        .rated_speed
        .as_ref()
        .map(|window| [window.minimum_entry, window.maximum_entry]);
    let margin = candidate
        .entry_speed
        .zip(candidate.rated_speed.as_ref())
        .map(|(speed, window)| (speed - window.minimum_entry).min(window.maximum_entry - speed));
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
        session
            .select(id, true)
            .map_err(|error| error.to_string())?;
        session.estimates();
    }
    Ok(())
}

fn format_window(value: &Value) -> String {
    value.as_array().map_or_else(
        || "—".to_owned(),
        |window| {
            format!(
                "{:.1}-{:.1}",
                window[0]
                    .as_f64()
                    .expect("rated window minimum is a number"),
                window[1]
                    .as_f64()
                    .expect("rated window maximum is a number")
            )
        },
    )
}

fn format_peaks(peaks: Option<&ConstructionLastPlacedForceWire>) -> String {
    peaks.map_or_else(
        || "preview unavailable".to_owned(),
        |peaks| {
            format!(
                "v+ {:.2} g, v- {:.2} g, lateral {:.2} g, longitudinal {:.2} g",
                peaks.maximum_vertical_g,
                peaks.minimum_vertical_g,
                peaks.maximum_absolute_lateral_g,
                peaks.maximum_absolute_longitudinal_g
            )
        },
    )
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
            let position = piece
                .sample(piece.length() * sample_index as f64 / sample_count as f64)
                .position
                / 4.0;
            samples.push((piece_index, position.x, position.y, position.z));
        }
    }
    if samples.is_empty() {
        return Err("construction has no track samples".into());
    }
    let scale = 4.0;
    let min_x = samples
        .iter()
        .map(|sample| sample.1.floor() as i32)
        .min()
        .unwrap();
    let max_x = samples
        .iter()
        .map(|sample| sample.1.floor() as i32)
        .max()
        .unwrap();
    let min_y = samples
        .iter()
        .map(|sample| sample.2.floor() as i32)
        .min()
        .unwrap();
    let max_y = samples
        .iter()
        .map(|sample| sample.2.floor() as i32)
        .max()
        .unwrap();
    let mut cells = BTreeMap::new();
    for (piece_index, x, y, _) in &samples {
        cells.insert(
            ((*x / scale).floor() as i32, (*y / scale).floor() as i32),
            if *piece_index == 0 { 'S' } else { '#' },
        );
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
        .map(str::to_owned)
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

fn replay_with_estimates(
    ride: &RideFile,
) -> Result<(BuilderSession, Vec<RidePieceEstimate>), String> {
    let simulation = WasmSimulation::new(42, 0, "new".into());
    let mut session = BuilderSession::new(simulation);
    let heading = match ride.station.heading {
        RideHeading::East => 0,
        RideHeading::North => 1,
        RideHeading::West => 2,
        RideHeading::South => 3,
    };
    session
        .start(
            &ride.model,
            ride.station.x,
            ride.station.y,
            ride.station.z,
            heading,
        )
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
                let track_before = construction_track(&session)?.len();
                session
                    .select(&place.place, true)
                    .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                let placement = session.estimates();
                let candidate = placement.candidate_preview.as_ref().ok_or_else(|| {
                    format!(
                        "ride step {}: candidate preview unavailable",
                        step_index + 1
                    )
                })?;
                estimates.push(ride_piece_estimate(
                    piece_index,
                    &place.place,
                    candidate,
                    place.note.clone(),
                ));
                session
                    .place()
                    .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                if let Some(speed) = place.launch_speed {
                    let track = construction_track(&session)?;
                    let bridge_id = track
                        .last()
                        .and_then(|piece| piece["id"].as_u64())
                        .map(|id| id as usize)
                        .ok_or_else(|| {
                            format!(
                                "ride step {}: launch has no bridge piece id",
                                step_index + 1
                            )
                        })?;
                    session
                        .set_launch_speed(bridge_id, speed)
                        .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                    session.estimates();
                }
                let track_after = construction_track(&session)?.len();
                piece_index += track_after - track_before;
            }
            RideStep::Chain(chain) => {
                session
                    .set_chain(chain.chain)
                    .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
                session.estimates();
            }
            RideStep::Close(close) => {
                if !close.close {
                    return Err(format!("ride step {}: close must be true", step_index + 1));
                }
                session
                    .close()
                    .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
            }
            RideStep::BlockSections(blocks) => {
                session
                    .install_block_sections(
                        &blocks.block_sections,
                        blocks.train_capacity,
                        blocks.capacity_search_complete,
                        blocks.placement_search_complete,
                    )
                    .map_err(|error| format!("ride step {}: {error}", step_index + 1))?;
            }
        }
    }
    if ride.trains != 1
        && !ride
            .steps
            .iter()
            .any(|step| matches!(step, RideStep::Close(_)))
    {
        return Err(format!(
            "ride step {}: train count can only be set after the circuit is closed",
            ride.steps.len() + 1
        ));
    }
    let circuit_is_closed = ride
        .steps
        .iter()
        .any(|step| matches!(step, RideStep::Close(_)));
    if circuit_is_closed
        && ride.trains != 1
        && session.simulation().construction_train_count() != usize::from(ride.trains)
    {
        session
            .set_train_count(usize::from(ride.trains))
            .map_err(|error| format!("ride step {}: {error}", ride.steps.len() + 1))?;
        session.estimates();
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
        airtime: classify_rating(
            &candidate.limits.rating_bands.airtime,
            -peaks.minimum_vertical_g,
        ),
        vertical: classify_rating(
            &candidate.limits.rating_bands.vertical,
            peaks.maximum_vertical_g,
        ),
        lateral: classify_rating(
            &candidate.limits.rating_bands.lateral,
            peaks.maximum_absolute_lateral_g,
        ),
        longitudinal: classify_rating(
            &candidate.limits.rating_bands.longitudinal,
            peaks.maximum_absolute_longitudinal_g,
        ),
    });
    RidePieceEstimate {
        piece_index,
        piece_id: piece_id.to_owned(),
        entry_speed: candidate.entry_speed,
        rated_window: candidate
            .rated_speed
            .as_ref()
            .map(|window| (window.minimum_entry, window.maximum_entry)),
        estimated,
        ratings,
        preview_status: candidate.status.to_owned(),
        notes: note.into_iter().collect(),
    }
}

fn compare_estimates(
    ride: &RideFile,
    test_run: &TestRun,
    estimates: &[RidePieceEstimate],
) -> Result<AgreementReport, String> {
    let model = validate_model(&ride.model)?;
    let measured: BTreeMap<_, _> = test_run
        .piece_force_peaks()
        .into_iter()
        .map(|peaks| (peaks.piece_index, peaks))
        .collect();
    let estimate_by_piece: BTreeMap<_, _> = estimates
        .iter()
        .map(|estimate| (estimate.piece_index, estimate))
        .collect();
    let finished_track = finished_track_estimates(ride, estimates)?;
    let mut inputs = Vec::new();
    for (piece_index, (piece_name, finished_entry, finished_peak)) in finished_track {
        let placement = estimate_by_piece.get(&piece_index).copied();
        let measured_peaks = measured.get(&piece_index);
        let entry_measured = test_run
            .samples
            .iter()
            .find(|sample| sample.piece_index == piece_index)
            .map(|sample| sample.speed);
        inputs.push(AgreementInput {
            piece_index,
            piece_name,
            model_name: model.name.to_string(),
            entry_speed_estimate: finished_entry,
            entry_speed_measured_front: entry_measured,
            placement_estimate: placement
                .and_then(|estimate| estimate.estimated)
                .map(force_peaks),
            finished_track_estimate: finished_peak,
            measured_front: measured_peaks
                .and_then(|peaks| peaks.front_car)
                .map(force_peaks),
            measured_worst: measured_peaks
                .and_then(|peaks| peaks.worst_car)
                .map(force_peaks),
            limits: model.comfortable_force_limits,
        });
    }
    Ok(evaluate_agreement(&inputs))
}

type FinishedTrackEstimate = (String, Option<f64>, Option<ForcePeaks>);
type FinishedTrackEstimates = BTreeMap<usize, FinishedTrackEstimate>;

fn finished_track_estimates(
    ride: &RideFile,
    placement: &[RidePieceEstimate],
) -> Result<FinishedTrackEstimates, String> {
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
        let samples: Vec<_> = preview
            .samples
            .iter()
            .filter(|sample| sample.distance >= start && sample.distance <= end)
            .collect();
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
                    peaks.minimum_vertical_g =
                        peaks.minimum_vertical_g.min(current.minimum_vertical_g);
                    peaks.maximum_vertical_g =
                        peaks.maximum_vertical_g.max(current.maximum_vertical_g);
                    peaks.maximum_lateral_g =
                        peaks.maximum_lateral_g.max(current.maximum_lateral_g);
                    peaks.maximum_longitudinal_g = peaks
                        .maximum_longitudinal_g
                        .max(current.maximum_longitudinal_g);
                    peaks
                }
            })
        });
        let entry_speed = preview
            .samples
            .iter()
            .find(|sample| sample.distance >= start)
            .map(|sample| sample.speed);
        let measured_name = placement
            .iter()
            .find(|estimate| estimate.piece_index == piece_index)
            .map_or(name.clone(), |estimate| estimate.piece_id.clone());
        result.insert(
            piece_index,
            (measured_name, entry_speed, peaks.map(force_peaks)),
        );
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
        writeln!(
            text,
            "{} {:<24} {:?} {:?}",
            row.piece_index, row.piece_name, row.check_2, row.check_3
        )
        .unwrap();
    }
    for failure in &report.failures {
        writeln!(
            text,
            "{} {:?}: {} ({})",
            failure.piece_index, failure.check, failure.metric, failure.reason
        )
        .unwrap();
    }
    writeln!(
        text,
        "Agreement: {}",
        if report.passed { "pass" } else { "fail" }
    )
    .unwrap();
    text
}

fn agreement_json(report: &AgreementReport) -> Value {
    let rows: Vec<_> = report
        .rows
        .iter()
        .map(|row| {
            json!({
                "pieceIndex": row.piece_index,
                "pieceName": row.piece_name,
                "check2": format!("{:?}", row.check_2),
                "check3": format!("{:?}", row.check_3),
            })
        })
        .collect();
    let failures: Vec<_> = report
        .failures
        .iter()
        .map(|failure| {
            json!({
                "pieceIndex": failure.piece_index,
                "pieceName": failure.piece_name,
                "check": format!("{:?}", failure.check),
                "metric": failure.metric,
                "reason": failure.reason,
            })
        })
        .collect();
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
        let ride: RideFile = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse ride {path}: {error}"))?;
        let (test_run, estimates) = test_ride(&ride).map_err(|error| format!("{path}: {error}"))?;
        let agreement = compare_estimates(&ride, &test_run, &estimates)?;
        let failures = check_expectations(&ride, &test_run);
        for failure in &failures {
            expectation_failures.push(format!("{path}: {failure}"));
        }
        writeln!(
            text,
            "{}: expectations {}; native agreement {} (informational)",
            ride.name,
            if failures.is_empty() { "pass" } else { "fail" },
            if agreement.passed { "pass" } else { "fail" }
        )
        .unwrap();
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
        return Err(format!(
            "{}\nExpectation checks failed:\n{}",
            text,
            expectation_failures.join("\n")
        ));
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
            failures.push(format!(
                "excitement expected {minimum:.2}-{maximum:.2}, observed {:.2}",
                test_run.excitement
            ));
        }
    }
    if let Some([minimum, maximum]) = expect.sustained_max_vertical_g {
        let observed = test_run.statistics.max_positive_vertical_g;
        if !(minimum..=maximum).contains(&observed) {
            failures.push(format!(
                "maximum vertical g expected {minimum:.2}-{maximum:.2}, observed {observed:.2}"
            ));
        }
    }
    if let Some([minimum, maximum]) = expect.sustained_min_vertical_g {
        let observed = test_run.statistics.max_negative_vertical_g;
        if !(minimum..=maximum).contains(&observed) {
            failures.push(format!(
                "minimum vertical g expected {minimum:.2}-{maximum:.2}, observed {observed:.2}"
            ));
        }
    }
    if let Some(maximum) = expect.max_lateral_g {
        if test_run.statistics.max_lateral_g > maximum {
            failures.push(format!(
                "maximum lateral g expected at most {maximum:.2}, observed {:.2}",
                test_run.statistics.max_lateral_g
            ));
        }
    }
    if let Some(inversions) = expect.inversions {
        if test_run.statistics.inversions != inversions as usize {
            failures.push(format!(
                "inversions expected {inversions}, observed {}",
                test_run.statistics.inversions
            ));
        }
    }
    if let Some(minimum) = expect.airtime_moments_at_least {
        if test_run.statistics.airtime_moments < minimum as usize {
            failures.push(format!(
                "airtime moments expected at least {minimum}, observed {}",
                test_run.statistics.airtime_moments
            ));
        }
    }
    failures
}

fn export_session(ride: &RideFile, session: &BuilderSession) -> Result<Value, String> {
    let expected_placement_count = ride
        .steps
        .iter()
        .filter(|step| matches!(step, RideStep::Place(_)))
        .count();
    if session.placed_piece_bridge_ids().len() != expected_placement_count {
        return Err(format!(
            "ride replay recorded {} placement groups for {expected_placement_count} place steps",
            session.placed_piece_bridge_ids().len()
        ));
    }
    let mut commands = vec![
        json!({"tick": 0, "command": {"type": "construction-select", "piece": format!("model:{}", ride.model)}}),
        json!({"tick": 0, "command": {"type": "construction-start", "x": ride.station.x, "y": ride.station.y, "z": ride.station.z, "heading": match ride.station.heading { RideHeading::East => 0, RideHeading::North => 1, RideHeading::West => 2, RideHeading::South => 3 }}}),
        json!({"tick": 0, "command": {"type": "construction-set-cars-per-train", "cars": ride.cars_per_train}}),
    ];
    let mut placement_index = 0;
    for (step_offset, step) in ride.steps.iter().enumerate() {
        let step_index = step_offset + 1;
        match step {
            RideStep::Place(place) => {
                let bridge_ids = &session.placed_piece_bridge_ids()[placement_index];
                placement_index += 1;
                commands.push(json!({"tick": 0, "command": {"type": "construction-select", "piece": place.place, "exact": true}}));
                commands.push(json!({"tick": 0, "command": {"type": "construction-append"}}));
                if let Some(target_speed) = place.launch_speed {
                    let id = bridge_ids
                        .last()
                        .copied()
                        .ok_or_else(|| format!("ride step {step_index} has no bridge piece id"))?;
                    commands.push(json!({"tick": 0, "command": {"type": "construction-set-launch-intensity", "id": id, "targetSpeed": target_speed}}));
                }
            }
            RideStep::Chain(chain) => {
                commands.push(json!({"tick": 0, "command": {"type": "construction-set-chain", "enabled": chain.chain}}));
            }
            RideStep::Close(close) if close.close => {
                commands.push(json!({"tick": 0, "command": {"type": "construction-close"}}));
            }
            RideStep::Close(_) => return Err("ride file close step must be true".into()),
            RideStep::BlockSections(blocks) => {
                commands.push(json!({"tick": 0, "command": {
                    "type": "construction-set-block-sections",
                    "blockSections": blocks.block_sections,
                    "trainCapacity": blocks.train_capacity,
                    "capacitySearchComplete": blocks.capacity_search_complete,
                    "placementSearchComplete": blocks.placement_search_complete,
                }}));
            }
        }
    }
    if ride.trains != 1
        && !ride
            .steps
            .iter()
            .any(|step| matches!(step, RideStep::Close(_)))
    {
        return Err(format!(
            "ride step {}: train count can only be set after the circuit is closed",
            ride.steps.len() + 1
        ));
    }
    if ride.trains != 1 {
        if session.simulation().construction_train_count() != usize::from(ride.trains) {
            return Err(format!(
                "ride replay has train count {}, expected {}",
                session.simulation().construction_train_count(),
                ride.trains
            ));
        }
        commands.push(json!({"tick": 0, "command": {"type": "construction-set-train-count", "count": ride.trains}}));
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
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
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

#[cfg(test)]
mod replay_snapshot_tests {
    use super::*;

    #[test]
    fn replay_snapshot_is_invalidated_when_binary_fingerprint_changes() {
        let path = std::env::temp_dir().join(format!(
            "theme-park-replay-snapshot-{}-{}.snapshot",
            std::process::id(),
            snapshot_checksum(b"binary fingerprint test")
        ));
        let session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        let snapshot = encode_replay_snapshot(
            b"ride file bytes",
            1,
            &session.save_native_snapshot().unwrap(),
            &json!({"state": "valid"}),
        )
        .unwrap();
        fs::write(&path, snapshot).unwrap();

        let loaded = load_replay_snapshot(&path, b"ride file bytes", 2).unwrap();

        assert!(loaded.is_none());
        fs::remove_file(path).unwrap();
    }
}
