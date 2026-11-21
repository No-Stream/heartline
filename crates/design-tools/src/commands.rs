use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use theme_park_sim::physics::{CarPieceForcePeakLocations, Circuit, ForcePeakValues, TestRun};
use theme_park_sim::track::blocks::BlockSectionPlacement;
use theme_park_sim::track::construction::shaped_turn_intent;
use theme_park_sim::track::ride_file::{
    RideBlockSectionsStep, RideChainStep, RideCloseStep, RideExpectFeatureStep, RideExpectFeatures,
    RideFile, RideFileFormat, RideHeading, RidePlaceStep, RideShapeStep, RideStation, RideStep,
};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, TurnSizeHint, default_hill_strength,
};
use theme_park_sim::track::{
    CoasterModel, Hand, Heading as TrackHeading, Heartlining, HillFeel, PieceId, coaster_models,
};
use theme_park_sim_wasm::WasmSimulation;
use theme_park_sim_wasm::builder_session::{BuilderEstimates, BuilderSession};
use theme_park_sim_wasm::construction::{
    ConstructionCandidateForcePreviewWire, ConstructionForcePreviewWire,
    ConstructionForceRatingBandWire, ConstructionLastPlacedForceWire,
    ConstructionShapedElementWire,
};
use theme_park_sim_wasm::reports::TestingSample;

use crate::agreement::{
    AgreementInput, AgreementReport, ForcePeaks, PlacementFailureClassification,
    PlacementPeakContext, PlacementPeakContexts, evaluate_agreement,
};
use crate::brake_report::{
    brake_run_speed_rows, mid_course_brake_energy_report, render_brake_run_speed_table,
};
use crate::finish_report::{check_completion, finish_options_json, render_finish_options};
use crate::report::{
    ForceRatings, RidePieceEstimate, ShapedElementReportRow, default_physics_test_run,
    element_physics_report_rows, force_headroom_results, format_force_headroom_results,
    rated_speed_margin_failures, rated_speed_margin_report, render_element_physics_table,
    render_ride_report_with_margins, render_shaped_elements_table, shaped_element_entry_speeds,
};

#[derive(Debug)]
struct CommandReply {
    text: String,
    json: Value,
    changed_ride: bool,
}

#[derive(Debug)]
enum CliResponse {
    Success(String),
    Failure { stdout: String, stderr: String },
}

#[derive(Debug)]
/// Captured command output, including the process exit code.
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
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
    Probe(String),
    Place {
        id: String,
        matched: bool,
        launch_speed: Option<f64>,
        note: Option<String>,
        shape_request: Option<ShapedPlaceRequest>,
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
    FinishApply(usize),
    Close,
    Test,
    Report(bool),
    Agreement,
    Map,
    ExportSave(PathBuf),
    ExportSession(PathBuf),
}

#[derive(Debug)]
enum ShapedPlaceRequest {
    Hill {
        feel: HillFeel,
        strength_g: Option<f64>,
        size: HillSizeHint,
    },
    Turn {
        hand: Hand,
        size: TurnSizeHint,
    },
}

impl ShapedPlaceRequest {
    fn into_intent(self, model: &CoasterModel) -> ElementIntent {
        match self {
            Self::Hill {
                feel,
                strength_g,
                size,
            } => ElementIntent::Hill {
                feel,
                strength_g: strength_g.unwrap_or_else(|| default_hill_strength(feel, model)),
                size,
            },
            Self::Turn { hand, size } => ElementIntent::Turn { hand, size },
        }
    }
}

const COMMAND_NAMES: &[&str] = &[
    "new",
    "state",
    "choices",
    "options",
    "estimate",
    "probe",
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

const METRES_PER_TILE: f64 = 4.0;
const REPLAY_SNAPSHOT_MAGIC: &[u8] = b"TPRS";
const REPLAY_SNAPSHOT_VERSION: u8 = 1;
const AUTO_BLOCKS_WORK_BUDGET: u32 = 1_000;

/// Runs the command line interface and returns command output or its error text.
pub fn run_cli(arguments: &[String], stdin: &str) -> Result<String, String> {
    let output = run_cli_with_status(arguments, stdin);
    if output.exit_code == 0 {
        Ok(output.stdout)
    } else if !output.stdout.is_empty() {
        Err(output.stdout)
    } else {
        Err(output.stderr)
    }
}

/// Runs the command line interface while preserving stdout, stderr, and exit status separately.
pub fn run_cli_with_status(arguments: &[String], stdin: &str) -> CliOutput {
    cli_output(run_cli_response(arguments, stdin))
}

fn cli_output(response: Result<CliResponse, String>) -> CliOutput {
    match response {
        Ok(CliResponse::Success(stdout)) => CliOutput {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        },
        Ok(CliResponse::Failure { stdout, stderr }) => CliOutput {
            stdout,
            stderr,
            exit_code: 1,
        },
        Err(stderr) => CliOutput {
            stdout: String::new(),
            stderr,
            exit_code: 1,
        },
    }
}

fn run_cli_response(arguments: &[String], stdin: &str) -> Result<CliResponse, String> {
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
        "help" => Ok(CliResponse::Success(help_text().to_owned())),
        "ride" => {
            if arguments.len() < 2 {
                return Err("usage: tools/design ride <file> [command ...]".into());
            }
            let mut command_tokens = arguments[2..].to_vec();
            if command_tokens.is_empty() {
                command_tokens.extend(stdin.lines().map(str::to_owned));
            }
            let commands = parse_commands(&command_tokens)?;
            run_ride(Path::new(&arguments[1]), commands, json_mode).map(CliResponse::Success)
        }
        "check" => run_check(&arguments[1..], json_mode),
        "park" => run_park(&arguments[1..], json_mode).map(CliResponse::Success),
        _ => Err(help_text().to_owned()),
    }
}

fn help_text() -> &'static str {
    "usage: tools/design <command> ...
Top-level commands:
  help                                      list commands and choice groups
  ride <file> [command ...]                 create or edit a ride file
  check <ride file> ...                     check ride estimates against expectations, then agree (text mode)
  agree <ride file> ...                     replay rides through the browser builder path in Node/WASM
  park build <park file> --save <path>      build and save a park
  park report <park file> [options]          simulate and report on a park

Ride commands:
  new --model <key> --station x,y,z,heading [--cars <count>]
                                            start a ride
  state                                     show the open end and construction status
  choices [group]                           list offered pieces and their rated windows
  options shapes                            list feasible shaped elements at the open end
  estimate <id> ...                         estimate one or more exact pieces
  probe <id>                                preview a piece without placing it
  place <id> [options]                      place a piece
  place hill feel=<floater|strong-floater|ejector> [g=<target>] [height=<metres>]
  place turn <left|right> feel=banked [radius=<metres>]
  chain on|off                              toggle a lift chain
  launch-speed <piece> <m/s>                set a launch speed
  cars <count>                              set cars per train
  trains <count>                            set the train count
  auto-blocks                               find automatic block sections
  undo [count]                              undo placements
  truncate <steps>                          keep the first ride steps
  finish                                    list ranked ways back to the station
  finish apply <n>                          place finish option n and close the circuit
  close                                     close the circuit at the station
  test                                      run a ride test
  report [--samples]                        print a ride test report
  agreement                                 compare estimates with measured forces
  map                                       draw the ride footprint
  export --save <path>|--session <path>     export a save or replay session

Choice groups: bank, element, hill, inversion, shapes, slope, special, straight, turn.
Classic hill sizes and speed-turn variants are listed by choices with their rated windows.
"
}

fn run_park(arguments: &[String], json_mode: bool) -> Result<String, String> {
    match arguments.first().map(String::as_str) {
        Some("build") => run_park_build(arguments),
        Some("report") => crate::park_report::run_park_report(arguments, json_mode),
        Some(command) => Err(format!(
            "unknown park command {command:?}; usage: tools/design park build <park file> --save <path> or tools/design park report <park file> [--guests 2000] [--minutes 20] [--seed 42]"
        )),
        None => Err("usage: tools/design park build <park file> --save <path> or tools/design park report <park file> [--guests 2000] [--minutes 20] [--seed 42]".into()),
    }
}

fn run_park_build(arguments: &[String]) -> Result<String, String> {
    let Some(command) = arguments.first() else {
        return Err("usage: tools/design park build <park file> --save <path>".into());
    };
    if command != "build" {
        return Err(format!(
            "unknown park command {command:?}; usage: tools/design park build <park file> --save <path>"
        ));
    }
    if arguments.len() < 2 {
        return Err("usage: tools/design park build <park file> --save <path>".into());
    }

    let park_path = Path::new(&arguments[1]);
    let mut save_path = None;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--save" if save_path.is_none() => {
                index += 1;
                save_path = Some(PathBuf::from(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--save requires a path".to_owned())?,
                ));
            }
            flag => return Err(format!("unknown or repeated park build option {flag:?}")),
        }
        index += 1;
    }
    let save_path = save_path.ok_or_else(|| {
        "park build requires --save <path>: tools/design park build <park file> --save <path>"
            .to_owned()
    })?;

    let park_source = fs::read_to_string(park_path)
        .map_err(|error| format!("read park description {}: {error}", park_path.display()))?;
    let runtime_description = inline_park_ride_files(park_path, &park_source)?;
    let simulation = WasmSimulation::from_park_description(42, 0, &runtime_description)
        .map_err(|error| error.to_string())?;
    let save_bytes = simulation.save_state_native()?;
    fs::write(&save_path, save_bytes)
        .map_err(|error| format!("write park save {}: {error}", save_path.display()))?;

    Ok(format!("Saved park to {}\n", save_path.display()))
}

pub(crate) fn inline_park_ride_files(
    park_path: &Path,
    park_source: &str,
) -> Result<String, String> {
    let mut park: Value = serde_json::from_str(park_source)
        .map_err(|error| format!("parse park description {}: {error}", park_path.display()))?;
    let coasters = park
        .get_mut("coasters")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "park description field `coasters` must be an array".to_owned())?;
    if coasters.len() > 1 {
        return Err("this build supports one player coaster per park".into());
    }

    for (coaster_index, coaster) in coasters.iter_mut().enumerate() {
        let ride_reference = coaster.get("ride").and_then(Value::as_str).ok_or_else(|| {
            format!("park coaster {coaster_index} field `ride` must be a ride file path")
        })?;
        let ride_path = resolve_park_ride_path(coaster_index, ride_reference)?;
        let ride_source = fs::read_to_string(&ride_path).map_err(|error| {
            format!(
                "park coaster {coaster_index} ride {ride_reference:?}: read {}: {error}",
                ride_path.display()
            )
        })?;
        let ride = RideFile::from_json_str(&ride_source).map_err(|error| {
            format!(
                "park coaster {coaster_index} ride {ride_reference:?}: parse {}: {error}",
                ride_path.display()
            )
        })?;
        coaster["ride"] = serde_json::to_value(ride).map_err(|error| {
            format!(
                "park coaster {coaster_index} ride {ride_reference:?}: serialize ride file: {error}"
            )
        })?;
    }

    serde_json::to_string(&park).map_err(|error| format!("serialize park description: {error}"))
}

pub(crate) fn resolve_park_ride_path(
    coaster_index: usize,
    ride_reference: &str,
) -> Result<PathBuf, String> {
    let reference_path = Path::new(ride_reference);
    let is_portable_ride_reference = ride_reference.starts_with("content/rides/")
        && ride_reference.ends_with(".ride.json")
        && reference_path.components().count() == 3
        && reference_path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if !is_portable_ride_reference {
        return Err(format!(
            "park coaster {coaster_index} ride path {ride_reference:?} must use content/rides/<name>.ride.json"
        ));
    }

    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(reference_path))
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
        "options" if parameters == ["shapes"] => {
            Ok(RideCommand::Choices(Some("shapes".to_owned())))
        }
        "estimate" if !parameters.is_empty() => Ok(RideCommand::Estimate(parameters.to_vec())),
        "probe" if parameters.len() == 1 => Ok(RideCommand::Probe(parameters[0].clone())),
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
        "finish" if parameters.len() == 2 && parameters[0] == "apply" => Ok(
            RideCommand::FinishApply(parse_number(&parameters[1], "finish option")?),
        ),
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
    if id == "hill" || id == "turn" {
        return parse_shaped_place(id, &parameters[1..]);
    }
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
        shape_request: None,
    })
}

fn parse_shaped_place(kind: &str, parameters: &[String]) -> Result<RideCommand, String> {
    let (id, request) = match kind {
        "hill" => {
            let (mut feel, mut strength_g, mut height) = (None, None, None);
            for parameter in parameters {
                let (name, value) = parameter
                    .split_once('=')
                    .ok_or_else(|| format!("hill option must use name=value: {parameter:?}"))?;
                match name {
                    "feel" if feel.is_none() => {
                        feel = Some(match value {
                            "floater" => ("floater", HillFeel::Floater),
                            "strong-floater" => ("strongFloater", HillFeel::StrongFloater),
                            "ejector" => ("ejector", HillFeel::Ejector),
                            _ => return Err(format!("unknown hill feel {value:?}")),
                        });
                    }
                    "g" if strength_g.is_none() => {
                        let value = parse_number::<f64>(value, "hill strength")?;
                        if !value.is_finite() || value >= 0.0 {
                            return Err("hill strength must be a finite negative g value".into());
                        }
                        strength_g = Some(value);
                    }
                    "height" if height.is_none() => {
                        let value = parse_number::<f64>(value, "hill height")?;
                        if !value.is_finite() || value <= 0.0 {
                            return Err("hill height must be a finite positive value".into());
                        }
                        height = Some(value);
                    }
                    "feel" | "g" | "height" => {
                        return Err(format!("duplicate hill option {name:?}"));
                    }
                    _ => return Err(format!("unknown hill option {name:?}")),
                }
            }
            let (feel_wire, feel) =
                feel.ok_or("place hill requires feel=<floater|strong-floater|ejector>")?;
            let strength =
                strength_g.map_or_else(|| "default".to_owned(), |value| value.to_string());
            let size = height.map_or_else(|| "auto".to_owned(), |value| format!("fixed-{}", value));
            (
                format!("shape:hill:{feel_wire}:{strength}:{size}"),
                ShapedPlaceRequest::Hill {
                    feel,
                    strength_g,
                    size: height.map_or(HillSizeHint::Auto, HillSizeHint::Height),
                },
            )
        }
        "turn" => {
            let Some(hand) = parameters.first().map(String::as_str) else {
                return Err("place turn requires left or right".into());
            };
            if !matches!(hand, "left" | "right") {
                return Err(format!("unknown turn hand {hand:?}"));
            }
            let (mut feel, mut radius) = (None, None);
            for parameter in &parameters[1..] {
                let (name, value) = parameter
                    .split_once('=')
                    .ok_or_else(|| format!("turn option must use name=value: {parameter:?}"))?;
                match name {
                    "feel" if feel.is_none() => {
                        if value != "banked" {
                            return Err(format!("unknown turn feel {value:?}"));
                        }
                        feel = Some(value);
                    }
                    "radius" if radius.is_none() => {
                        let value = parse_number::<f64>(value, "turn radius")?;
                        if !value.is_finite() || value <= 0.0 {
                            return Err("turn radius must be a finite positive value".into());
                        }
                        radius = Some(value);
                    }
                    "feel" | "radius" => {
                        return Err(format!("duplicate turn option {name:?}"));
                    }
                    _ => return Err(format!("unknown turn option {name:?}")),
                }
            }
            if feel != Some("banked") {
                return Err("place turn requires feel=banked".into());
            }
            let size = radius.map_or_else(|| "auto".to_owned(), |value| format!("fixed-{}", value));
            let hand = if hand == "left" {
                Hand::Left
            } else {
                Hand::Right
            };
            let hand_wire = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            (
                format!("shape:turn:{hand_wire}:fullyBanked:{size}"),
                ShapedPlaceRequest::Turn {
                    hand,
                    size: radius.map_or(TurnSizeHint::Auto, TurnSizeHint::Radius),
                },
            )
        }
        _ => unreachable!("only shaped hill and turn kinds are parsed"),
    };
    Ok(RideCommand::Place {
        id,
        matched: false,
        launch_speed: None,
        note: None,
        shape_request: Some(request),
    })
}

fn shape_selection_id(intent: &ElementIntent) -> Result<String, String> {
    let selection = match intent {
        ElementIntent::Hill {
            feel,
            strength_g,
            size,
        } => {
            let feel = match feel {
                HillFeel::Floater => "floater",
                HillFeel::StrongFloater => "strongFloater",
                HillFeel::Ejector => "ejector",
                feel => return Err(format!("unsupported shaped hill feel {feel:?}")),
            };
            let strength = strength_g.to_string();
            let size = match size {
                HillSizeHint::Auto => "auto".to_owned(),
                HillSizeHint::Height(height_m) => format!("fixed-{height_m}"),
            };
            format!("shape:hill:{feel}:{strength}:{size}")
        }
        ElementIntent::Turn { hand, size } => {
            let hand = match hand {
                Hand::Left => "left",
                Hand::Right => "right",
            };
            let size = match size {
                TurnSizeHint::Auto => "auto".to_owned(),
                TurnSizeHint::Radius(radius_m) => format!("fixed-{radius_m}"),
            };
            format!("shape:turn:{hand}:fullyBanked:{size}")
        }
    };
    Ok(selection)
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
        let ride = RideFile::from_json_slice(&bytes)
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
                Some(estimates) => render_state_with_estimates(&mut session, estimates),
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
            (Some(estimates), None) => render_state_with_estimates(&mut session, estimates)?,
            (None, Some(state)) => render_state_value(state),
            (None, None) => render_state(&mut session)?,
            (Some(_), Some(_)) => unreachable!("replay estimates and cached state are exclusive"),
        };
        state_cache = Some(reply.json.clone());
        replies.push(reply);
    }
    if state_cache.is_none() {
        state_cache = Some(match replay_estimates.take() {
            Some(estimates) => state_value_with_estimates(&mut session, estimates)?,
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

pub(crate) fn validate_model(model: &str) -> Result<CoasterModel, String> {
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
        RideCommand::Probe(id) => render_probe(session, &id),
        RideCommand::Place {
            id,
            matched,
            launch_speed,
            note,
            shape_request,
        } => {
            let shape_intent = shape_request
                .map(|request| {
                    let model = session
                        .simulation()
                        .construction
                        .as_ref()
                        .ok_or_else(|| "shaped placement requires active construction".to_owned())?
                        .model();
                    Ok::<ElementIntent, String>(request.into_intent(model))
                })
                .transpose()?;
            let id = shape_intent
                .as_ref()
                .map(shape_selection_id)
                .transpose()?
                .unwrap_or(id);
            let selection = session
                .select(&id, !matched)
                .map_err(|error| error.to_string())?;
            if shape_intent.is_some() && selection.selected_id != id {
                return Err(format!(
                    "shaped selection {id:?} resolved to unexpected selection {:?}",
                    selection.selected_id
                ));
            }
            if matched {
                let speed_match = selection
                    .speed_match
                    .as_ref()
                    .ok_or_else(|| format!("{id} has no speed-matched variants"))?;
                if speed_match.fits != Some(true) {
                    let entry_speed = speed_match.entry_speed.map_or_else(
                        || "the current end speed".to_owned(),
                        |speed| format!("{speed:.2} m/s"),
                    );
                    return Err(format!(
                        "no speed-matched variant for {id} fits at {entry_speed}"
                    ));
                }
            }
            let estimates = session.estimates();
            if let Some(refusal) = estimates
                .selected_shape
                .as_ref()
                .and_then(|shape| shape.refusal.as_ref())
            {
                return Err(format!(
                    "shaped selection refused: {}",
                    serde_json::to_string(refusal).expect("shape refusal serializes")
                ));
            }
            if let Some(choice) = estimates
                .choices
                .iter()
                .find(|choice| choice.id == selection.selected_id)
                && !choice.refusal.is_empty()
            {
                return Err(format!(
                    "{} is not offered at this end: {}",
                    selection.selected_id, choice.refusal
                ));
            }
            let geometry = candidate_geometry_value(session);
            let (mut placement_json, estimate_text) = if shape_intent.is_some() {
                let shape = estimates
                    .selected_shape
                    .as_ref()
                    .ok_or_else(|| format!("shaped selection {id} has no shape estimate"))?;
                let shape_value = serde_json::to_value(shape)
                    .map_err(|error| format!("serialize shaped selection {id}: {error}"))?;
                let entry_speed = shape
                    .spec
                    .as_ref()
                    .expect("a non-refused shaped selection has a resolved spec")
                    .design_speed_mps;
                (
                    shaped_candidate_value(&selection.selected_id, shape, geometry),
                    format!(
                        "{entry_speed:.1} m/s entry, {}",
                        format_shape_details(&shape_value).trim()
                    ),
                )
            } else {
                let candidate = estimates.candidate_preview.as_ref().ok_or_else(|| {
                    candidate_preview_unavailable(
                        &selection.selected_id,
                        &estimates.choices,
                        session.simulation(),
                    )
                })?;
                (
                    candidate_value(&selection.selected_id, candidate, None, geometry),
                    format!(
                        "{} m/s entry, {}",
                        optional_number(candidate.entry_speed),
                        format_peaks(candidate.peaks.as_ref())
                    ),
                )
            };
            placement_json["shape"] = estimates
                .selected_shape
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| format!("serialize shaped selection {id}: {error}"))?
                .unwrap_or(Value::Null);
            if let Some(choice) = estimates
                .choices
                .iter()
                .find(|choice| choice.id == selection.selected_id)
            {
                placement_json["refusal"] = json!(choice.refusal);
                placement_json["wouldTunnel"] = json!(choice.would_tunnel);
            }
            let _placed_estimates = session.place().map_err(|error| error.to_string())?;
            let resolved_shape_spec = if shape_intent.is_some() {
                Some(
                    session
                        .simulation()
                        .construction
                        .as_ref()
                        .and_then(|construction| construction.pieces().last())
                        .and_then(|piece| piece.element_spec())
                        .cloned()
                        .ok_or_else(|| {
                            format!("placed shaped selection {id} has no resolved element spec")
                        })?,
                )
            } else {
                None
            };
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
            let saved_id =
                if shape_intent.is_some() || selection.selected_id.starts_with("element:") {
                    selection.selected_id
                } else {
                    track.last().unwrap()["label"]
                        .as_str()
                        .expect("track label is a string")
                        .to_owned()
                };
            if let (Some(intent), Some(spec)) = (shape_intent, resolved_shape_spec) {
                ride.steps
                    .push(RideStep::Shape(RideShapeStep { intent, spec }));
            } else {
                ride.steps.push(RideStep::Place(RidePlaceStep {
                    place: saved_id.clone(),
                    launch_speed,
                    note: note.clone(),
                }));
            }
            let state = state_value(session)?;
            let mut text = format!(
                "Placed {saved_id}{}\n",
                if matched && id != saved_id {
                    format!(" (speed matched from {id})")
                } else {
                    String::new()
                }
            );
            writeln!(text, "Estimate: {estimate_text}").unwrap();
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
                    .rposition(|step| matches!(step, RideStep::Place(_) | RideStep::Shape(_)))
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
            let completions = session
                .finish_options()
                .map_err(|error| error.to_string())?;
            let model = validate_model(&ride.model)?;
            let placed_pieces = placed_construction_pieces(session)?;
            let checks = completions
                .iter()
                .map(|completion| check_completion(&placed_pieces, completion, &model))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CommandReply {
                text: render_finish_options(&completions, &checks),
                json: finish_options_json(&completions, &checks),
                changed_ride: false,
            })
        }
        RideCommand::FinishApply(rank) => {
            let completions = session
                .finish_options()
                .map_err(|error| error.to_string())?;
            let completion = completions.get(rank.wrapping_sub(1)).ok_or_else(|| {
                format!(
                    "finish option {rank} does not exist; the search found {}",
                    completions.len()
                )
            })?;
            let model = validate_model(&ride.model)?;
            let check =
                check_completion(&placed_construction_pieces(session)?, completion, &model)?;
            if !check.passed() {
                return Err(format!(
                    "finish option {rank} fails the ride check: {}",
                    check.failures.join("; ")
                ));
            }
            let track_before = construction_track(session)?.len();
            session
                .finish_apply(rank)
                .map_err(|error| error.to_string())?;
            let track_after = construction_track(session)?;
            let added = track_after.len().saturating_sub(track_before);
            if added != completion.geometry.len() {
                return Err(format!(
                    "finish option {rank} added {added} piece(s) but has {}",
                    completion.geometry.len()
                ));
            }
            for (piece, placed) in completion
                .geometry
                .iter()
                .zip(track_after.iter().skip(track_before))
            {
                ride.steps.push(match piece.element_spec() {
                    Some(spec) => RideStep::Shape(RideShapeStep {
                        intent: shaped_turn_intent(spec),
                        spec: spec.clone(),
                    }),
                    None => RideStep::Place(RidePlaceStep {
                        place: placed["label"]
                            .as_str()
                            .ok_or_else(|| "finished piece has no label".to_owned())?
                            .to_owned(),
                        launch_speed: None,
                        note: None,
                    }),
                });
            }
            ride.steps
                .push(RideStep::Close(RideCloseStep { close: true }));
            let state = state_value(session)?;
            Ok(reply_state(
                format!("Placed finish option {rank} ({added} piece(s)) and closed the circuit.\n"),
                state,
                true,
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
            let (test_run, estimates, rated_margins, circuit) = test_ride_with_margins(ride)?;
            let mut report = render_ride_report_with_margins(
                ride,
                &test_run,
                &estimates,
                &rated_margins,
                false,
            )?;
            let shaped_elements = shaped_element_report_rows(ride, &circuit)?;
            if !shaped_elements.is_empty() {
                report.push_str(&render_shaped_elements_table(&shaped_elements));
            }
            Ok(CommandReply {
                text: report.clone(),
                json: json!({"ride": ride, "testRun": test_run, "estimates": estimates, "shapedElements": shaped_elements, "text": report}),
                changed_ride: false,
            })
        }
        RideCommand::Report(include_samples) => {
            let (test_run, estimates, rated_margins, circuit) = test_ride_with_margins(ride)?;
            let mut report = render_ride_report_with_margins(
                ride,
                &test_run,
                &estimates,
                &rated_margins,
                include_samples,
            )?;
            let model = validate_model(&ride.model)?;
            let default_test_run = default_physics_test_run(&circuit, &model);
            let shaped_elements = shaped_element_report_rows(ride, &circuit)?;
            if !shaped_elements.is_empty() {
                report.push_str(&render_shaped_elements_table(&shaped_elements));
            }
            let element_physics = element_physics_report_rows(
                &circuit,
                &estimates,
                &shaped_elements,
                &default_test_run,
                &test_run,
            )?;
            report.push_str(&render_element_physics_table(&element_physics));
            let brake_runs = brake_run_speed_rows(&circuit, &default_test_run, &test_run);
            report.push_str(&render_brake_run_speed_table(&brake_runs));
            report.push_str(&mid_course_brake_energy_report(
                &circuit,
                &test_run,
                ride.cars_per_train,
            ));
            Ok(CommandReply {
                text: report.clone(),
                json: json!({"ride": ride, "testRun": test_run, "estimates": estimates, "shapedElements": shaped_elements, "elementPhysics": element_physics, "brakeRuns": brake_runs, "text": report}),
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
    session: &mut BuilderSession,
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
    session: &mut BuilderSession,
    estimates: BuilderEstimates,
) -> Result<Value, String> {
    session.search_finish();
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
    .sqrt();
    let offset_to_station_tiles = offset_to_station / METRES_PER_TILE;
    let heading_mismatch_degrees = heading_mismatch_degrees(end.heading, start.heading);
    let ground_height = simulation
        .world
        // Track states count half tiles and two-step heights; the world counts tiles and steps.
        .surface((end.x / 2) as u16, (end.y / 2) as u16)
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
            "positionMetres": {"x": end_position.x, "y": end_position.y, "z": end_position.z},
            "tile": {"x": end_position.x / METRES_PER_TILE, "y": end_position.y / METRES_PER_TILE},
            "tilesToStationEntry": {
                "x": (start_position.x - end_position.x) / METRES_PER_TILE,
                "y": (start_position.y - end_position.y) / METRES_PER_TILE,
            },
            "metresAboveStationEntry": end_position.z - start_position.z,
            "offsetToStationEntryTiles": offset_to_station_tiles,
            "heading": format!("{:?}", end.heading),
            "headingMismatchDegrees": heading_mismatch_degrees,
            "slope": format!("{:?}", end.slope),
            "bank": format!("{:?}", end.bank),
            "heightAboveGroundSteps": end.z * 2 - i32::from(ground_height),
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
    let tile = &end["tile"];
    let to_station = &end["tilesToStationEntry"];
    let finish_reason = state["finishReason"]
        .as_str()
        .expect("finish reason is a string");
    format!(
        "Pieces: {}\nOpen end: tile ({:.1}, {:.1}), height {:.1} m, heading {}, slope {}, bank {}\nHeight above ground: {} steps; offset to station entry: {:.1} m (x {:+.1}, y {:+.1} tiles, height {:+.1} m)\nClosure approach: remaining offset: {:.1} tiles ({:.1} m); heading mismatch: {:.0} degrees\nChain: {}; predicted end speed: {} m/s ({})\nCan close: {}; can finish: {}{}; can test: {}\n",
        state["pieceCount"]
            .as_u64()
            .expect("piece count is an unsigned integer"),
        tile["x"].as_f64().expect("open end tile x is a number"),
        tile["y"].as_f64().expect("open end tile y is a number"),
        position["z"].as_f64().expect("open end height is a number"),
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
        to_station["x"]
            .as_f64()
            .expect("station tile offset x is a number"),
        to_station["y"]
            .as_f64()
            .expect("station tile offset y is a number"),
        end["metresAboveStationEntry"]
            .as_f64()
            .expect("station height offset is a number"),
        end["offsetToStationEntryTiles"]
            .as_f64()
            .expect("station offset in tiles is a number"),
        end["offsetToStationEntryMetres"]
            .as_f64()
            .expect("station offset is a number"),
        end["headingMismatchDegrees"]
            .as_f64()
            .expect("heading mismatch is a number"),
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

fn heading_mismatch_degrees(end: TrackHeading, station: TrackHeading) -> f64 {
    let heading_index = |heading| -> i32 {
        match heading {
            TrackHeading::East => 0,
            TrackHeading::NorthEast => 1,
            TrackHeading::North => 2,
            TrackHeading::NorthWest => 3,
            TrackHeading::West => 4,
            TrackHeading::SouthWest => 5,
            TrackHeading::South => 6,
            TrackHeading::SouthEast => 7,
        }
    };
    let difference = (heading_index(end) - heading_index(station)).abs();
    f64::from(difference.min(8 - difference)) * 45.0
}

fn placed_construction_pieces(
    session: &BuilderSession,
) -> Result<Vec<theme_park_sim::track::Piece>, String> {
    Ok(session
        .simulation()
        .construction
        .as_ref()
        .ok_or_else(|| "ride has no construction".to_owned())?
        .pieces()
        .to_vec())
}

fn construction_track(session: &BuilderSession) -> Result<Vec<Value>, String> {
    serde_json::from_str(&session.simulation().construction_track())
        .map_err(|error| format!("parse construction track: {error}"))
}

fn render_choices(
    session: &mut BuilderSession,
    group: Option<&str>,
) -> Result<CommandReply, String> {
    if group == Some("shapes") {
        return render_shaped_choices(session);
    }
    let current_selection = session.simulation().construction_selected();
    let base = session.estimates();
    let end_speed = base.end_speed;
    let choices = base.choices;
    let elements = base.elements;
    let hill_options: Vec<Value> = serde_json::from_str(&base.hill_options_json)
        .map_err(|error| format!("parse hill options: {error}"))?;
    let speed_turns: Vec<Value> = serde_json::from_str(&base.speed_turns_json)
        .map_err(|error| format!("parse speed turns: {error}"))?;
    let mut rows = Vec::new();
    for choice in choices {
        if hill_options.iter().any(|hill| hill["id"] == choice.id)
            || group.is_some_and(|group| group != choice.group)
        {
            continue;
        }
        let preview = candidate_for(session, choice.id)?;
        let refusal = if choice.refusal.is_empty() {
            preview
                .get("previewUnavailableReason")
                .cloned()
                .unwrap_or_else(|| preview["refusal"].clone())
        } else {
            json!(choice.refusal)
        };
        rows.push(json!({
            "id": choice.id,
            "name": choice.label,
            "group": choice.group,
            "refusal": refusal,
            "wouldTunnel": choice.would_tunnel,
            "ratedWindow": candidate_window(session, &preview, choice.id),
            "entrySpeedMargin": preview["margin"],
            "speedMatchedRecommendation": speed_match_recommends(session, choice.id, end_speed),
            "hillPrediction": preview["hillPrediction"],
            "endTile": preview["endTile"],
            "endHeight": preview["endHeight"],
            "length": preview["length"],
        }));
    }
    for hill in hill_options {
        if group.is_some_and(|group| group != "hill")
            || rows.iter().any(|row| row["id"] == hill["id"])
        {
            continue;
        }
        let id = hill["id"].as_str().expect("hill id is a string");
        let preview = candidate_for(session, id)?;
        let mut rated_window = candidate_window(session, &preview, id);
        if rated_window.is_null() {
            rated_window = hill["ratedSpeed"]
                .as_object()
                .map(|window| json!([window["minimumEntry"], window["maximumEntry"]]))
                .unwrap_or(Value::Null);
        }
        rows.push(json!({
            "id": id,
            "name": format!(
                "{} hill {}x{} m ({})",
                hill["feel"].as_str().expect("hill feel is a string"),
                hill["height"].as_u64().expect("hill height is an integer"),
                hill["span"].as_u64().expect("hill span is an integer"),
                hill["twist"].as_str().expect("hill twist is a string"),
            ),
            "group": "hill",
            "refusal": preview.get("previewUnavailableReason").cloned().unwrap_or_else(|| preview["refusal"].clone()),
            "wouldTunnel": preview["wouldTunnel"],
            "ratedWindow": rated_window,
            "entrySpeedMargin": preview["margin"],
            "speedMatchedRecommendation": speed_match_recommends(session, id, end_speed),
            "hillPrediction": preview["hillPrediction"],
            "endTile": preview["endTile"],
            "endHeight": preview["endHeight"],
            "length": preview["length"],
        }));
    }
    for turn in speed_turns {
        if group.is_some_and(|group| group != "turn")
            || rows.iter().any(|row| row["id"] == turn["id"])
        {
            continue;
        }
        let id = turn["id"].as_str().expect("speed turn id is a string");
        let preview = candidate_for(session, id)?;
        rows.push(json!({
            "id": id,
            "name": format!(
                "{} speed turn {:.0} m/s",
                turn["hand"].as_str().expect("speed turn hand is a string"),
                turn["designSpeed"].as_f64().expect("speed turn design speed is a number"),
            ),
            "group": "turn",
            "refusal": preview.get("previewUnavailableReason").cloned().unwrap_or_else(|| preview["refusal"].clone()),
            "wouldTunnel": preview["wouldTunnel"],
            "ratedWindow": candidate_window(session, &preview, id),
            "entrySpeedMargin": preview["margin"],
            "speedMatchedRecommendation": speed_match_recommends(session, id, end_speed),
            "hillPrediction": Value::Null,
            "endTile": preview["endTile"],
            "endHeight": preview["endHeight"],
            "length": preview["length"],
        }));
    }
    for element in elements {
        if group.is_some_and(|group| group != "element") {
            continue;
        }
        let preview = candidate_for(session, &element.id)?;
        let refusal = preview
            .get("previewUnavailableReason")
            .cloned()
            .unwrap_or_else(|| {
                if element.would_tunnel {
                    json!("would tunnel")
                } else {
                    json!("")
                }
            });
        rows.push(json!({
            "id": element.id,
            "name": element.name,
            "group": "element",
            "refusal": refusal,
            "wouldTunnel": element.would_tunnel,
            "ratedWindow": null,
            "entrySpeedMargin": null,
            "speedMatchedRecommendation": false,
            "endTile": preview["endTile"],
            "endHeight": preview["endHeight"],
            "length": preview["length"],
        }));
    }
    if group.is_none() {
        let shaped_rows = render_shaped_choices(session)?.json;
        let Value::Array(shaped_rows) = shaped_rows else {
            return Err("shaped choice renderer did not return an array".into());
        };
        rows.extend(shaped_rows);
    }
    restore_selection(session, &current_selection)?;
    let mut text = String::from(
        "id                         name                         group       refusal                 tunnel window     margin matched end tile        height m length m\n",
    );
    for row in &rows {
        writeln!(
            text,
            "{:<26} {:<28} {:<11} {:<23} {:<6} {:<12} {:>6} {:<7} {:<15} {:>8} {:>8}",
            row["id"].as_str().expect("choice id is a string"),
            row["name"].as_str().expect("choice name is a string"),
            row["group"].as_str().expect("choice group is a string"),
            format_choice_refusal(&row["refusal"]),
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
            format_end_tile(&row["endTile"]),
            row["endHeight"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), |value| format!("{value:.1}")),
            row["length"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), |value| format!("{value:.1}")),
        )
        .unwrap();
    }
    Ok(CommandReply {
        text,
        json: Value::Array(rows),
        changed_ride: false,
    })
}

fn render_shaped_choices(session: &mut BuilderSession) -> Result<CommandReply, String> {
    const SHAPED_SELECTIONS: [(&str, &str); 5] = [
        ("floater hill", "shape:hill:floater:default:auto"),
        (
            "strong floater hill",
            "shape:hill:strongFloater:default:auto",
        ),
        ("ejector hill", "shape:hill:ejector:default:auto"),
        ("left fully banked turn", "shape:turn:left:fullyBanked:auto"),
        (
            "right fully banked turn",
            "shape:turn:right:fullyBanked:auto",
        ),
    ];
    let current_selection = session.simulation().construction_selected();
    let (model_key, vertical_limits, lateral_limit, rules) = {
        let construction = session
            .simulation()
            .construction
            .as_ref()
            .ok_or_else(|| "there is no active construction".to_owned())?;
        let model = construction.model();
        (
            model.key,
            (
                model.comfortable_force_limits.minimum_vertical_g,
                model.comfortable_force_limits.maximum_vertical_g,
            ),
            model.comfortable_force_limits.maximum_lateral_g,
            model.shaped_element_rules(),
        )
    };
    let mut rows = Vec::with_capacity(SHAPED_SELECTIONS.len());
    for (name, id) in SHAPED_SELECTIONS {
        let preview = candidate_for(session, id)?;
        let shape = preview
            .get("shape")
            .filter(|shape| !shape.is_null())
            .ok_or_else(|| format!("builder did not return shaped selection {id}"))?;
        let refusal = shape
            .get("refusal")
            .filter(|refusal| !refusal.is_null())
            .cloned()
            .or_else(|| preview.get("previewUnavailableReason").cloned())
            .unwrap_or_else(|| json!(""));
        rows.push(json!({
            "id": id,
            "name": name,
            "group": "shapes",
            "modelLimits": {
                "verticalG": [vertical_limits.0, vertical_limits.1],
                "lateralG": lateral_limit,
                "floaterG": rules.default_floater_g,
                "strongFloaterG": rules.default_strong_floater_g,
                "steelEjectorG": rules.steel_ejector_g,
                "woodenEjectorG": rules.wooden_ejector_g,
                "turnRadiusM": [rules.minimum_turn_radius_m, rules.maximum_turn_radius_m],
                "maximumBankDegrees": rules.maximum_bank_degrees,
            },
            "refusal": refusal,
            "wouldTunnel": preview["wouldTunnel"],
            "ratedWindow": null,
            "entrySpeedMargin": null,
            "speedMatchedRecommendation": false,
            "entrySpeed": preview["entrySpeed"],
            "peaks": preview["peaks"],
            "status": preview["status"],
            "shape": shape,
            "endTile": preview["endTile"],
            "endHeight": preview["endHeight"],
            "length": preview["length"],
        }));
    }
    restore_selection(session, &current_selection)?;
    let mut text = format!(
        "Model {model_key} limits: vertical {:+.2} to {:+.2} g, lateral up to {:.2} g; default hill targets floater {:+.2} g, strong floater {:+.2} g, ejector steel {:+.2} g or wood {:+.2} g; turn radius {:.1} to {:.1} m, bank up to {:.1} degrees.\n",
        vertical_limits.0,
        vertical_limits.1,
        lateral_limit,
        rules.default_floater_g,
        rules.default_strong_floater_g,
        rules.steel_ejector_g,
        rules.wooden_ejector_g,
        rules.minimum_turn_radius_m,
        rules.maximum_turn_radius_m,
        rules.maximum_bank_degrees,
    );
    text.push_str(
        "id                                   name                     design m/s delivered g size                 refusal\n",
    );
    for row in &rows {
        let shape = &row["shape"];
        let spec = &shape["spec"];
        let design_speed = optional_number(spec["designSpeedMps"].as_f64());
        let delivered = optional_g(
            spec["deliveredG"]
                .as_f64()
                .or_else(|| spec["lateralG"].as_f64()),
        );
        let size = match shape["kind"].as_str().unwrap_or("") {
            "hill" => format!(
                "{}x{} m",
                optional_number(spec["heightM"].as_f64()),
                optional_number(spec["spanM"].as_f64()),
            ),
            "turn" => format!("radius {} m", optional_number(spec["radiusM"].as_f64())),
            _ => "—".to_owned(),
        };
        writeln!(
            text,
            "{:<36} {:<24} {:>10} {:>11} {:<20} {}",
            row["id"].as_str().expect("shape id is a string"),
            row["name"].as_str().expect("shape name is a string"),
            design_speed,
            delivered,
            size,
            if row["refusal"].is_string() {
                row["refusal"].as_str().unwrap().to_owned()
            } else {
                serde_json::to_string(&row["refusal"]).expect("typed refusal serializes")
            }
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
    let mut text = String::from(
        "piece                      entry m/s window      margin  candidate preview vertical g (rating)     lateral g (rating)   long g (rating) status\n",
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
                writeln!(
                    text,
                    "  hill inspector prediction (closed-form): entry {} m/s, vertical {} to {} g",
                    optional_number(hill["entrySpeed"].as_f64()),
                    optional_number(hill["minimumVerticalG"].as_f64()),
                    optional_number(hill["maximumVerticalG"].as_f64()),
                )
                .unwrap();
            }
        }
        if let Some(shape) = row.get("shape").filter(|shape| !shape["spec"].is_null()) {
            write!(text, "  {}", format_shape_details(shape)).unwrap();
        }
        if let Some(reason) = row["previewUnavailableReason"].as_str() {
            writeln!(text, "  {reason}").unwrap();
        }
    }
    Ok(CommandReply {
        text,
        json: Value::Array(rows),
        changed_ride: false,
    })
}

fn render_probe(session: &mut BuilderSession, id: &str) -> Result<CommandReply, String> {
    let current_selection = session.simulation().construction_selected();
    let candidate = candidate_for(session, id)?;
    restore_selection(session, &current_selection)?;
    let refusal = candidate["refusal"]
        .as_str()
        .expect("candidate refusal is a string");
    let is_shaped = !candidate["shape"].is_null();
    if (candidate["status"] == "unavailable" || !refusal.is_empty()) && !is_shaped {
        if candidate["status"] != "unavailable" {
            return Err(format!("{} is not offered at this end: {refusal}", id));
        }
        return Err(candidate["previewUnavailableReason"]
            .as_str()
            .expect("unavailable candidate has a reason")
            .to_owned());
    }

    let peaks = &candidate["peaks"];
    let mut text = format!(
        "Probe {}\nEntry speed: {} m/s\nForces: {}\nEnd tile: {}; height {} m; length {} m\n",
        candidate["id"].as_str().expect("candidate id is a string"),
        optional_number(candidate["entrySpeed"].as_f64()),
        format_peaks_value(peaks),
        format_end_tile(&candidate["endTile"]),
        optional_number(candidate["endHeight"].as_f64()),
        optional_number(candidate["length"].as_f64()),
    );
    if let Some(shape) = candidate.get("shape").filter(|shape| !shape.is_null()) {
        text.push_str(&format_shape_details(shape));
    }
    Ok(CommandReply {
        text,
        json: candidate,
        changed_ride: false,
    })
}

fn candidate_for(session: &mut BuilderSession, id: &str) -> Result<Value, String> {
    let selection = session
        .select(id, true)
        .map_err(|error| error.to_string())?;
    let estimates = session.estimates();
    let shape_value = estimates
        .selected_shape
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| format!("serialize shaped selection {id}: {error}"))?
        .unwrap_or(Value::Null);
    if let Some(shape) = estimates
        .selected_shape
        .as_ref()
        .filter(|shape| shape.spec.is_some())
    {
        return Ok(shaped_candidate_value(
            &selection.selected_id,
            shape,
            candidate_geometry_value(session),
        ));
    }
    let Some(candidate) = estimates.candidate_preview.as_ref() else {
        let mut value = unavailable_candidate_value(
            &selection.selected_id,
            &estimates.choices,
            session.simulation(),
        );
        value["shape"] = shape_value;
        return Ok(value);
    };
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
            "method": "closed-form hill inspector prediction",
        }));
    let geometry = candidate_geometry_value(session);
    let mut value = candidate_value(&selection.selected_id, candidate, hill, geometry);
    let choice = estimates
        .choices
        .iter()
        .find(|choice| choice.id == selection.selected_id);
    value["refusal"] = json!(choice.map_or("", |choice| choice.refusal.as_str()));
    value["wouldTunnel"] = json!(choice.is_some_and(|choice| choice.would_tunnel));
    value["shape"] = shape_value;
    if selection.selected_id != id {
        value["requestedId"] = json!(id);
    }
    Ok(value)
}

fn format_shape_details(value: &Value) -> String {
    let Some(spec) = value.get("spec").filter(|spec| !spec.is_null()) else {
        let refusal = value
            .get("refusal")
            .filter(|refusal| !refusal.is_null())
            .expect("a shaped selection without a spec has a typed refusal");
        return format!(
            "Shape refusal: {}\n",
            serde_json::to_string(refusal).expect("shape refusal JSON serializes")
        );
    };
    match value["kind"].as_str().expect("shape kind is a string") {
        "hill" => format!(
            "Shaped hill: {} at {:.1} m/s for {} cars; target {} g, delivered {} g; height {} m, span {} m\n",
            value["feel"].as_str().expect("shaped hill has a feel"),
            spec["designSpeedMps"]
                .as_f64()
                .expect("design speed is a number"),
            spec["cars"].as_u64().expect("car count is an integer"),
            optional_g(spec["targetG"].as_f64()),
            optional_g(spec["deliveredG"].as_f64()),
            optional_number(spec["heightM"].as_f64()),
            optional_number(spec["spanM"].as_f64()),
        ),
        "turn" => format!(
            "Shaped {} fully banked turn: {:.1} m/s for {} cars; delivered vertical {} g and lateral {} g; radius {} m, bank {} degrees\n",
            value["hand"].as_str().expect("shaped turn has a hand"),
            spec["designSpeedMps"]
                .as_f64()
                .expect("design speed is a number"),
            spec["cars"].as_u64().expect("car count is an integer"),
            optional_g(spec["verticalG"].as_f64()),
            optional_g(spec["lateralG"].as_f64()),
            optional_number(spec["radiusM"].as_f64()),
            optional_number(spec["bankDegrees"].as_f64()),
        ),
        kind => panic!("unknown shaped element kind {kind:?}"),
    }
}

fn format_choice_refusal(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(reason) => reason.clone(),
        _ => serde_json::to_string(value).expect("typed choice refusal serializes"),
    }
}

fn unavailable_candidate_value(
    id: &str,
    choices: &[theme_park_sim_wasm::construction::ConstructionPieceWire],
    simulation: &WasmSimulation,
) -> Value {
    let choice = choices.iter().find(|choice| choice.id == id);
    let reason = candidate_preview_unavailable(id, choices, simulation);
    let geometry = simulation.construction_candidate_geometry();
    let hill = choice.and_then(|choice| {
        choice.hill_estimate.as_ref().map(|estimate| {
            json!({
                "entrySpeed": estimate.entry_speed,
                "ratedWindow": estimate.rated_speed.as_ref().map(|window| [window.minimum_entry, window.maximum_entry]),
                "minimumVerticalG": estimate.minimum_vertical_g,
                "maximumVerticalG": estimate.maximum_vertical_g,
                "maximumLateralG": estimate.maximum_absolute_lateral_g,
                "source": estimate.source,
                "method": "closed-form hill inspector prediction",
            })
        })
    });
    json!({
        "id": id,
        "entrySpeed": null,
        "ratedWindow": choice.and_then(|choice| choice.hill_estimate.as_ref())
            .and_then(|estimate| estimate.rated_speed.as_ref())
            .map(|window| json!([window.minimum_entry, window.maximum_entry]))
            .or_else(|| rated_window_for_id(simulation, id).map(|window| json!(window))),
        "margin": null,
        "peaks": null,
        "ratings": null,
        "status": "unavailable",
        "samples": [],
        "hillPrediction": hill,
        "refusal": choice.map_or("", |choice| choice.refusal.as_str()),
        "wouldTunnel": choice.is_some_and(|choice| choice.would_tunnel),
        "previewUnavailableReason": reason,
        "endTile": geometry.as_ref().map(|geometry| json!({"x": geometry.end_tile_x, "y": geometry.end_tile_y})),
        "endHeight": geometry.as_ref().map(|geometry| geometry.end_height),
        "length": geometry.map(|geometry| geometry.length),
    })
}

fn candidate_preview_unavailable(
    id: &str,
    choices: &[theme_park_sim_wasm::construction::ConstructionPieceWire],
    simulation: &WasmSimulation,
) -> String {
    let preview_reason = simulation.construction_preview_reason();
    if let Some(choice) = choices.iter().find(|choice| choice.id == id) {
        if !choice.refusal.is_empty() {
            return format!("{id} is not offered at this end: {}", choice.refusal);
        }
        if !preview_reason.is_empty() {
            return format!("candidate preview is unavailable for {id}: {preview_reason}");
        }
        let Some(construction) = simulation.construction.as_ref() else {
            return format!(
                "candidate preview is unavailable for {id}: there is no active construction"
            );
        };
        if construction.pieces().is_empty() {
            return format!(
                "candidate preview is unavailable for {id}: the construction has no track"
            );
        }
        return format!(
            "candidate preview is unavailable for {id}: the builder did not produce force samples"
        );
    }
    if preview_reason.is_empty() {
        format!("{id} is not offered at this end")
    } else {
        format!("{id} is not offered at this end: {preview_reason}")
    }
}

fn candidate_geometry_value(session: &BuilderSession) -> Option<Value> {
    let geometry = session.simulation().construction_candidate_geometry()?;
    Some(json!({
        "endTile": {"x": geometry.end_tile_x, "y": geometry.end_tile_y},
        "endHeight": geometry.end_height,
        "length": geometry.length,
    }))
}

fn rated_window_for_id(simulation: &WasmSimulation, id: &str) -> Option<[f64; 2]> {
    let construction = simulation.construction.as_ref()?;
    let model = construction.model();
    let piece = theme_park_sim::track::catalog()
        .iter()
        .find(|piece| format!("{:?}", piece.id) == id)?;
    if !model.allows_piece(piece) {
        return None;
    }
    let window = theme_park_sim::track::rated_speeds(model, piece)?;
    Some([window.minimum_entry, window.maximum_entry])
}

fn candidate_window(session: &BuilderSession, preview: &Value, id: &str) -> Value {
    preview["ratedWindow"]
        .as_array()
        .map(|_| preview["ratedWindow"].clone())
        .or_else(|| rated_window_for_id(session.simulation(), id).map(|window| json!(window)))
        .unwrap_or(Value::Null)
}

fn speed_match_recommends(session: &BuilderSession, id: &str, end_speed: Option<f64>) -> bool {
    session
        .simulation()
        .construction_speed_match(id, end_speed)
        .and_then(|result| result.recommended_id.map(|recommended| recommended == id))
        .unwrap_or(false)
}

fn optional_rating<'a>(ratings: &'a Value, key: &str) -> &'a str {
    ratings[key].as_str().unwrap_or("—")
}

fn candidate_value(
    id: &str,
    candidate: &ConstructionCandidateForcePreviewWire,
    hill: Option<Value>,
    geometry: Option<Value>,
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
        "forceEstimateMethod": "open-track candidate simulation",
        "refusal": "",
        "wouldTunnel": false,
        "endTile": geometry.as_ref().map(|geometry| geometry["endTile"].clone()),
        "endHeight": geometry.as_ref().and_then(|geometry| geometry["endHeight"].as_f64()),
        "length": geometry.as_ref().and_then(|geometry| geometry["length"].as_f64()),
    });
    if let Some(hill) = hill {
        value["hillPrediction"] = hill;
    }
    value
}

fn shaped_candidate_value(
    id: &str,
    shape: &ConstructionShapedElementWire,
    geometry: Option<Value>,
) -> Value {
    let spec = shape
        .spec
        .as_ref()
        .expect("a solved shaped candidate has a resolved spec");
    json!({
        "id": id,
        "entrySpeed": spec.design_speed_mps,
        "ratedWindow": null,
        "margin": null,
        "peaks": null,
        "ratings": null,
        "status": "reachedEnd",
        "samples": [],
        "forceEstimateMethod": "train-aware shaped-element calibration",
        "refusal": "",
        "wouldTunnel": false,
        "endTile": geometry.as_ref().map(|geometry| geometry["endTile"].clone()),
        "endHeight": geometry.as_ref().and_then(|geometry| geometry["endHeight"].as_f64()),
        "length": geometry.as_ref().and_then(|geometry| geometry["length"].as_f64()),
        "shape": shape,
    })
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

fn format_end_tile(value: &Value) -> String {
    let (Some(x), Some(y)) = (value["x"].as_f64(), value["y"].as_f64()) else {
        return "—".to_owned();
    };
    format!("({x:.1}, {y:.1})")
}

fn format_peaks_value(peaks: &Value) -> String {
    if peaks.is_null() {
        return "preview unavailable".to_owned();
    }
    format!(
        "v+ {} g, v- {} g, lateral {} g, longitudinal {} g",
        optional_number(peaks["maximumVerticalG"].as_f64()),
        optional_number(peaks["minimumVerticalG"].as_f64()),
        optional_number(peaks["maximumLateralG"].as_f64()),
        optional_number(peaks["maximumLongitudinalG"].as_f64()),
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
        let sample_count = (piece.length() / 4.0).ceil().max(1.0) as usize;
        for sample_index in 0..=sample_count {
            // The product can round a hair past the piece length, which `sample` refuses.
            let distance =
                (piece.length() * sample_index as f64 / sample_count as f64).min(piece.length());
            let position = piece.sample(distance).position;
            samples.push((piece_index, position.x, position.y, position.z));
        }
    }
    if samples.is_empty() {
        return Err("construction has no track samples".into());
    }
    let tiles_per_cell = 2.0;
    let scale = tiles_per_cell * METRES_PER_TILE;
    let cell = |metres: f64| (metres / scale).floor() as i32;
    let min_x = samples.iter().map(|sample| cell(sample.1)).min().unwrap();
    let max_x = samples.iter().map(|sample| cell(sample.1)).max().unwrap();
    let min_y = samples.iter().map(|sample| cell(sample.2)).min().unwrap();
    let max_y = samples.iter().map(|sample| cell(sample.2)).max().unwrap();
    let mut cells = BTreeMap::new();
    // The station is drawn last so the track around it cannot hide it.
    for (piece_index, x, y, _) in samples
        .iter()
        .filter(|sample| sample.0 != 0)
        .chain(samples.iter().filter(|sample| sample.0 == 0))
    {
        cells.insert(
            (cell(*x), cell(*y)),
            if *piece_index == 0 { 'S' } else { '#' },
        );
    }
    let mut text = format!(
        "Footprint (each cell is {tiles_per_cell} tiles, {scale} m; S is the station; north is up, x grows to the right; x {:.0} to {:.0}, y {:.0} to {:.0} tiles):\n",
        f64::from(min_x) * tiles_per_cell,
        f64::from(max_x + 1) * tiles_per_cell,
        f64::from(min_y) * tiles_per_cell,
        f64::from(max_y + 1) * tiles_per_cell,
    );
    for y in (min_y..=max_y).rev() {
        for x in min_x..=max_x {
            text.push(*cells.get(&(x, y)).unwrap_or(&'.'));
        }
        text.push('\n');
    }
    text.push_str(
        "Height profile (piece index, id, end tile, end height and highest point in metres):\n",
    );
    let mut profile = Vec::new();
    for (piece_index, piece) in construction.pieces().iter().enumerate() {
        let exit = piece.exit.position();
        let highest = samples
            .iter()
            .filter(|sample| sample.0 == piece_index)
            .map(|sample| sample.3)
            .fold(f64::NEG_INFINITY, f64::max);
        let (tile_x, tile_y) = (exit.x / METRES_PER_TILE, exit.y / METRES_PER_TILE);
        writeln!(
            text,
            "{piece_index:>3} {:<28} ({tile_x:>6.1}, {tile_y:>6.1}) {:>6.1} {highest:>6.1}",
            format!("{:?}", piece.id),
            exit.z
        )
        .unwrap();
        profile.push(json!({
            "pieceIndex": piece_index,
            "id": format!("{:?}", piece.id),
            "endTile": {"x": tile_x, "y": tile_y},
            "heightMetres": exit.z,
            "highestMetres": highest,
        }));
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
    let (mut session, estimates, _) = replay_with_estimates(ride, false)?;
    finish_test_run(&mut session).map(|test_run| (test_run, estimates))
}

fn test_ride_with_margins(
    ride: &RideFile,
) -> Result<
    (
        TestRun,
        Vec<RidePieceEstimate>,
        String,
        theme_park_sim::physics::Circuit,
    ),
    String,
> {
    let (mut session, estimates, _) = replay_with_estimates(ride, false)?;
    let (circuit, _) = session
        .simulation()
        .construction_circuit_and_train()
        .ok_or_else(|| "rated entry margins require a closed circuit".to_owned())?;
    let model = coaster_models()
        .into_iter()
        .find(|model| model.key == ride.model)
        .ok_or_else(|| format!("unknown coaster model {:?}", ride.model))?;
    let rated_margins = rated_speed_margin_report(&circuit, &model)?;
    let test_run = finish_test_run(&mut session)?;
    Ok((test_run, estimates, rated_margins, circuit))
}

fn shaped_element_report_rows(
    ride: &RideFile,
    circuit: &theme_park_sim::physics::Circuit,
) -> Result<Vec<ShapedElementReportRow>, String> {
    let shape_steps: Vec<&RideShapeStep> = ride
        .steps
        .iter()
        .filter_map(|step| match step {
            RideStep::Shape(shape) => Some(shape),
            _ => None,
        })
        .collect();
    if shape_steps.is_empty() {
        return Ok(Vec::new());
    }
    let shaped_piece_indices: Vec<usize> = circuit
        .pieces()
        .iter()
        .enumerate()
        .filter_map(|(piece_index, piece)| {
            matches!(
                piece.id,
                PieceId::ShapedHill | PieceId::ShapedTurnLeft | PieceId::ShapedTurnRight
            )
            .then_some(piece_index)
        })
        .collect();
    if shaped_piece_indices.len() != shape_steps.len() {
        return Err(format!(
            "ride has {} shape steps but the constructed circuit has {} shaped pieces",
            shape_steps.len(),
            shaped_piece_indices.len()
        ));
    }
    let model = validate_model(&ride.model)?;
    let entries = shaped_element_entry_speeds(circuit, &model, &shaped_piece_indices)?;
    shape_steps
        .into_iter()
        .zip(shaped_piece_indices)
        .map(|(shape_step, piece_index)| {
            let (design_speed_mps, element, intent, delivered, headroom, size) =
                match (&shape_step.intent, &shape_step.spec) {
                    (
                        ElementIntent::Hill { feel, .. },
                        ElementSpec::Hill {
                            design_speed_mps,
                            target_g,
                            delivered_g,
                            height_m,
                            span_m,
                            ..
                        },
                    ) => {
                        let feel_name = match feel {
                            HillFeel::Floater => "floater",
                            HillFeel::StrongFloater => "strong floater",
                            HillFeel::Ejector => "ejector",
                            feel => {
                                return Err(format!(
                                    "unsupported shaped hill feel {feel:?} at piece {piece_index}"
                                ));
                            }
                        };
                        (
                            *design_speed_mps,
                            format!("{feel_name} hill"),
                            format!("sustained airtime target {target_g:.2} g"),
                            format!("{delivered_g:.2} g"),
                            format!(
                                "{:.2} g vertical",
                                delivered_g - model.comfortable_force_limits.minimum_vertical_g
                            ),
                            format!("height {height_m:.1} m, span {span_m:.1} m"),
                        )
                    }
                    (
                        ElementIntent::Turn { hand, .. },
                        ElementSpec::Turn {
                            design_speed_mps,
                            hand: spec_hand,
                            vertical_g,
                            lateral_g,
                            radius_m,
                            bank_degrees,
                            ..
                        },
                    ) => {
                        if hand != spec_hand {
                            return Err(format!(
                                "ride turn intent hand does not match its resolved spec at piece {piece_index}"
                            ));
                        }
                        let hand_name = match hand {
                            Hand::Left => "left",
                            Hand::Right => "right",
                        };
                        (
                            *design_speed_mps,
                            format!("{hand_name} fully banked turn"),
                            format!(
                                "fully banked, vertical target {vertical_g:.2} g, lateral target 0.00 g"
                            ),
                            format!("vertical {vertical_g:.2} g, lateral {lateral_g:.2} g"),
                            format!(
                                "{:.2} g vertical, {:.2} g lateral",
                                model.comfortable_force_limits.maximum_vertical_g - vertical_g,
                                model.comfortable_force_limits.maximum_lateral_g - lateral_g.abs()
                            ),
                            format!("radius {radius_m:.1} m, bank {bank_degrees:.1} degrees"),
                        )
                    }
                    _ => {
                        return Err(format!(
                            "ride shape step for circuit piece {piece_index} has mismatched intent and spec kinds"
                        ));
                    }
                };
            let (default_entry_speed_mps, construction_entry_speed_mps) = entries[&piece_index];
            Ok(ShapedElementReportRow {
                piece_index,
                element,
                design_speed_mps,
                default_entry_speed_mps,
                construction_entry_speed_mps,
                intent,
                delivered,
                headroom,
                size,
            })
        })
        .collect()
}

fn test_ride_with_browser_estimates(
    ride: &RideFile,
) -> Result<
    (
        TestRun,
        Vec<RidePieceEstimate>,
        Vec<Value>,
        theme_park_sim::physics::Circuit,
    ),
    String,
> {
    let (mut session, estimates, browser_estimates) = replay_with_estimates(ride, true)?;
    let test_run = finish_test_run(&mut session)?;
    let (circuit, _) = session
        .simulation()
        .construction_circuit_and_train()
        .ok_or_else(|| "ride test completed without a construction circuit".to_owned())?;
    Ok((test_run, estimates, browser_estimates, circuit))
}

fn finish_test_run(session: &mut BuilderSession) -> Result<TestRun, String> {
    session.test().map_err(|error| error.to_string())?;
    session
        .simulation()
        .construction_report
        .as_ref()
        .cloned()
        .ok_or_else(|| "ride test completed without a native report".to_owned())
}

fn format_replay_step_error(step_index: usize, message: impl std::fmt::Display) -> String {
    format!("ride step {step_index}: {message}")
}

fn format_replay_piece_error(
    step_index: usize,
    piece_index: usize,
    message: impl std::fmt::Display,
) -> String {
    format!("ride step {step_index} (piece {piece_index}): {message}")
}

pub(crate) fn replay_with_estimates(
    ride: &RideFile,
    include_browser_estimates: bool,
) -> Result<(BuilderSession, Vec<RidePieceEstimate>, Vec<Value>), String> {
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
    let mut browser_estimates = Vec::new();
    for (step_index, step) in ride.steps.iter().enumerate() {
        match step {
            RideStep::Place(place) => {
                let construction_before = session
                    .simulation()
                    .construction
                    .as_ref()
                    .ok_or_else(|| "there is no active construction".to_owned())?;
                let first_piece_index = construction_before.pieces().len();
                let start_distance = construction_before
                    .pieces()
                    .iter()
                    .map(|piece| piece.length())
                    .sum::<f64>();
                session.select(&place.place, true).map_err(|error| {
                    format_replay_piece_error(step_index + 1, first_piece_index, error)
                })?;
                let placement = session.estimates();
                let candidate = placement.candidate_preview.as_ref().ok_or_else(|| {
                    format_replay_piece_error(
                        step_index + 1,
                        first_piece_index,
                        "candidate preview unavailable",
                    )
                })?;
                let selection_snapshot = include_browser_estimates
                    .then(|| builder_estimates_snapshot(&session, &placement))
                    .transpose()?;
                let mut placement_estimates = session.place().map_err(|error| {
                    format_replay_piece_error(step_index + 1, first_piece_index, error)
                })?;
                if let Some(speed) = place.launch_speed {
                    let track = construction_track(&session)?;
                    let bridge_id = track
                        .last()
                        .and_then(|piece| piece["id"].as_u64())
                        .map(|id| id as usize)
                        .ok_or_else(|| {
                            format_replay_piece_error(
                                step_index + 1,
                                first_piece_index,
                                "launch has no bridge piece id",
                            )
                        })?;
                    session
                        .set_launch_speed(bridge_id, speed)
                        .map_err(|error| {
                            format_replay_piece_error(step_index + 1, first_piece_index, error)
                        })?;
                    placement_estimates = session.estimates();
                }
                let construction_after = session
                    .simulation()
                    .construction
                    .as_ref()
                    .ok_or_else(|| "there is no active construction".to_owned())?;
                let track_after = construction_track(&session)?;
                let pieces = construction_after.pieces();
                let added_piece_count = pieces
                    .len()
                    .checked_sub(first_piece_index)
                    .expect("a successful placement does not remove existing pieces");
                let mut piece_start = start_distance;
                for relative_piece_index in 0..added_piece_count {
                    let piece_index = first_piece_index + relative_piece_index;
                    let piece = &pieces[piece_index];
                    let entry_speed = if relative_piece_index == 0 {
                        candidate.entry_speed
                    } else {
                        speed_at_distance(&candidate.samples, piece_start)
                    };
                    let piece_id = track_after
                        .get(piece_index)
                        .and_then(|row| row["label"].as_str())
                        .ok_or_else(|| {
                            format_replay_piece_error(
                                step_index + 1,
                                piece_index,
                                format!("placed piece {piece_index} has no track label"),
                            )
                        })?;
                    let note = if relative_piece_index == 0 {
                        place.note.clone()
                    } else {
                        None
                    };
                    let estimate = if added_piece_count == 1 {
                        ride_piece_estimate(piece_index, &place.place, candidate, note)
                    } else {
                        ride_piece_estimate_from_piece_peaks(
                            piece_index,
                            piece_id,
                            entry_speed,
                            candidate,
                            note,
                        )
                    };
                    estimates.push(estimate);
                    piece_start += piece.length();
                }
                if let Some(selection_snapshot) = selection_snapshot {
                    browser_estimates.push(json!({
                        "stepIndex": step_index + 1,
                        "pieceIndex": first_piece_index,
                        "pieceCount": added_piece_count,
                        "pieceId": place.place,
                        "selection": selection_snapshot,
                        "placement": builder_estimates_snapshot(&session, &placement_estimates)?,
                    }));
                }
            }
            RideStep::Shape(shape) => {
                let construction_before = session
                    .simulation()
                    .construction
                    .as_ref()
                    .ok_or_else(|| "there is no active construction".to_owned())?;
                let piece_index = construction_before.pieces().len();
                let start_distance = construction_before
                    .pieces()
                    .iter()
                    .map(|piece| piece.length())
                    .sum::<f64>();
                let selection_id = shape_selection_id(&shape.intent).map_err(|error| {
                    format_replay_piece_error(step_index + 1, piece_index, error)
                })?;
                session.select(&selection_id, true).map_err(|error| {
                    format_replay_piece_error(step_index + 1, piece_index, error)
                })?;
                let selection_estimates = session.estimates();
                let selection_snapshot = include_browser_estimates
                    .then(|| builder_estimates_snapshot(&session, &selection_estimates))
                    .transpose()?;
                let placement = session
                    .append_resolved(&shape.intent, &shape.spec)
                    .map_err(|error| {
                        format_replay_piece_error(step_index + 1, piece_index, error)
                    })?;
                let construction_after = session
                    .simulation()
                    .construction
                    .as_ref()
                    .ok_or_else(|| "there is no active construction".to_owned())?;
                if construction_after.pieces().len() != piece_index + 1 {
                    return Err(format_replay_piece_error(
                        step_index + 1,
                        piece_index,
                        "a shaped ride step must add exactly one piece",
                    ));
                }
                let track_after = construction_track(&session)?;
                let piece_id = track_after
                    .get(piece_index)
                    .and_then(|row| row["label"].as_str())
                    .ok_or_else(|| {
                        format_replay_piece_error(
                            step_index + 1,
                            piece_index,
                            "placed shaped piece has no track label",
                        )
                    })?;
                let force_preview = placement.force_preview.as_ref().ok_or_else(|| {
                    format_replay_piece_error(
                        step_index + 1,
                        piece_index,
                        "resolved shaped piece has no construction force preview",
                    )
                })?;
                let entry_speed = speed_at_distance(&force_preview.samples, start_distance);
                estimates.push(ride_piece_estimate_from_force_preview(
                    piece_index,
                    piece_id,
                    entry_speed,
                    force_preview,
                    Some("resolved shaped geometry".to_owned()),
                ));
                if let Some(selection_snapshot) = selection_snapshot {
                    browser_estimates.push(json!({
                        "stepIndex": step_index + 1,
                        "pieceIndex": piece_index,
                        "pieceCount": 1,
                        "pieceId": selection_id,
                        "selection": selection_snapshot,
                        "placement": builder_estimates_snapshot(&session, &placement)?,
                    }));
                }
            }
            RideStep::Chain(chain) => {
                session
                    .set_chain(chain.chain)
                    .map_err(|error| format_replay_step_error(step_index + 1, error))?;
                session.estimates();
            }
            RideStep::Close(close) => {
                if !close.close {
                    return Err(format_replay_step_error(
                        step_index + 1,
                        "close must be true",
                    ));
                }
                session
                    .close()
                    .map_err(|error| format_replay_step_error(step_index + 1, error))?;
            }
            RideStep::BlockSections(blocks) => {
                session
                    .install_block_sections(
                        &blocks.block_sections,
                        blocks.train_capacity,
                        blocks.capacity_search_complete,
                        blocks.placement_search_complete,
                    )
                    .map_err(|error| format_replay_step_error(step_index + 1, error))?;
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
    Ok((session, estimates, browser_estimates))
}

fn builder_estimates_snapshot(
    session: &BuilderSession,
    estimates: &BuilderEstimates,
) -> Result<Value, String> {
    let selected = session.simulation().construction_selected();
    if selected.is_empty() {
        return Err("builder snapshot has no selected piece".into());
    }
    let force_preview = estimates
        .force_preview
        .as_ref()
        .ok_or("builder snapshot has no construction force preview")?;
    let hill_estimate = estimates
        .choices
        .iter()
        .find(|choice| choice.id == selected)
        .and_then(|choice| choice.hill_estimate.as_ref());
    Ok(json!({
        "selected": selected,
        "selectedShape": estimates.selected_shape,
        "constructionForcePreview": force_preview,
        "candidateForcePreview": estimates.candidate_preview,
        "speedMatch": estimates.speed_match,
        "hillEstimate": hill_estimate,
    }))
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
    ride_piece_estimate_with_peaks(
        piece_index,
        piece_id,
        candidate.entry_speed,
        candidate,
        estimated,
        note,
    )
}

fn ride_piece_estimate_from_piece_peaks(
    piece_index: usize,
    piece_id: &str,
    entry_speed: Option<f64>,
    candidate: &ConstructionCandidateForcePreviewWire,
    note: Option<String>,
) -> RidePieceEstimate {
    let estimated = candidate
        .piece_force_peaks
        .iter()
        .find(|peaks| peaks.piece_index == piece_index)
        .and_then(|peaks| peaks.worst_car.as_ref())
        .map(|peaks| ForcePeakValues {
            minimum_vertical_g: peaks.minimum_vertical_g,
            maximum_vertical_g: peaks.maximum_vertical_g,
            maximum_lateral_g: peaks.maximum_lateral_g,
            maximum_longitudinal_g: peaks.maximum_longitudinal_g,
        });
    ride_piece_estimate_with_peaks(
        piece_index,
        piece_id,
        entry_speed,
        candidate,
        estimated,
        note,
    )
}

fn ride_piece_estimate_from_force_preview(
    piece_index: usize,
    piece_id: &str,
    entry_speed: Option<f64>,
    force_preview: &ConstructionForcePreviewWire,
    note: Option<String>,
) -> RidePieceEstimate {
    let estimated = force_preview
        .piece_force_peaks
        .iter()
        .find(|peaks| peaks.piece_index == piece_index)
        .and_then(|peaks| peaks.worst_car.as_ref())
        .map(|peaks| ForcePeakValues {
            minimum_vertical_g: peaks.minimum_vertical_g,
            maximum_vertical_g: peaks.maximum_vertical_g,
            maximum_lateral_g: peaks.maximum_lateral_g,
            maximum_longitudinal_g: peaks.maximum_longitudinal_g,
        });
    RidePieceEstimate {
        piece_index,
        piece_id: piece_id.to_owned(),
        entry_speed,
        rated_window: None,
        estimated,
        ratings: None,
        preview_status: force_preview.status.to_owned(),
        notes: note.into_iter().collect(),
    }
}

fn ride_piece_estimate_with_peaks(
    piece_index: usize,
    piece_id: &str,
    entry_speed: Option<f64>,
    candidate: &ConstructionCandidateForcePreviewWire,
    estimated: Option<ForcePeakValues>,
    note: Option<String>,
) -> RidePieceEstimate {
    let ratings = estimated.map(|peaks| ForceRatings {
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
            peaks.maximum_lateral_g,
        ),
        longitudinal: classify_rating(
            &candidate.limits.rating_bands.longitudinal,
            peaks.maximum_longitudinal_g,
        ),
    });
    RidePieceEstimate {
        piece_index,
        piece_id: piece_id.to_owned(),
        entry_speed,
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

pub(crate) fn compare_estimates(
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
    let FinishedTrackEstimates {
        by_piece: finished_track,
        piece_starts,
        circuit_length,
    } = finished_track_estimates(ride, estimates)?;
    let car_peak_locations = test_run.car_piece_force_peak_locations();
    let train_spacing = model.train.length_metres / f64::from(ride.cars_per_train);
    let peak_context_resolver = PlacementPeakContextResolver::new(
        &car_peak_locations,
        train_spacing,
        usize::from(ride.cars_per_train),
        &piece_starts,
        circuit_length,
    );
    let mut inputs = Vec::new();
    for (piece_index, finished_piece) in finished_track {
        let placement = estimate_by_piece.get(&piece_index).copied();
        let measured_peaks = measured.get(&piece_index);
        let measured_worst = measured_peaks
            .and_then(|peaks| peaks.worst_car)
            .map(force_peaks);
        let entry_measured = test_run
            .samples
            .iter()
            .find(|sample| sample.piece_index == piece_index)
            .map(|sample| sample.speed);
        inputs.push(AgreementInput {
            piece_index,
            piece_name: finished_piece.piece_name,
            model_name: model.name.to_string(),
            placement_entry_speed_estimate: placement.and_then(|estimate| estimate.entry_speed),
            entry_speed_estimate: finished_piece.entry_speed,
            entry_speed_measured_front: entry_measured,
            placement_estimate: placement
                .and_then(|estimate| estimate.estimated)
                .map(force_peaks),
            finished_track_estimate: finished_piece.front_car,
            finished_track_worst_car_estimate: finished_piece.worst_car,
            measured_front: measured_peaks
                .and_then(|peaks| peaks.front_car)
                .map(force_peaks),
            measured_worst,
            placement_peak_contexts: peak_context_resolver.for_piece(piece_index, measured_worst),
            limits: model.comfortable_force_limits,
        });
    }
    Ok(evaluate_agreement(&inputs))
}

struct FinishedTrackEstimate {
    piece_name: String,
    entry_speed: Option<f64>,
    front_car: Option<ForcePeaks>,
    worst_car: Option<ForcePeaks>,
}

struct FinishedTrackEstimates {
    by_piece: BTreeMap<usize, FinishedTrackEstimate>,
    piece_starts: Vec<f64>,
    circuit_length: f64,
}

fn finished_track_estimates(
    ride: &RideFile,
    placement: &[RidePieceEstimate],
) -> Result<FinishedTrackEstimates, String> {
    let mut session = create_session(ride)?;
    let preview = session
        .estimates()
        .force_preview
        .ok_or_else(|| "finished-track preview is unavailable".to_owned())?;
    let force_peaks_by_piece: BTreeMap<_, _> = preview
        .piece_force_peaks
        .into_iter()
        .map(|peaks| {
            (
                peaks.piece_index,
                (
                    peaks.front_car.map(|peaks| ForcePeaks {
                        minimum_vertical_g: peaks.minimum_vertical_g,
                        maximum_vertical_g: peaks.maximum_vertical_g,
                        maximum_lateral_g: peaks.maximum_lateral_g,
                        maximum_longitudinal_g: peaks.maximum_longitudinal_g,
                    }),
                    peaks.worst_car.map(|peaks| ForcePeaks {
                        minimum_vertical_g: peaks.minimum_vertical_g,
                        maximum_vertical_g: peaks.maximum_vertical_g,
                        maximum_lateral_g: peaks.maximum_lateral_g,
                        maximum_longitudinal_g: peaks.maximum_longitudinal_g,
                    }),
                ),
            )
        })
        .collect();
    let construction = session
        .simulation()
        .construction
        .as_ref()
        .ok_or_else(|| "there is no active construction".to_owned())?;
    let pieces = construction.pieces();
    let mut piece_starts = Vec::with_capacity(pieces.len());
    let mut distance = 0.0;
    for piece in pieces {
        piece_starts.push((distance, piece.name.to_owned()));
        distance += piece.length();
    }
    let mut by_piece = BTreeMap::new();
    for (piece_index, (start, name)) in piece_starts.iter().enumerate().skip(1) {
        let entry_speed = speed_at_distance(&preview.samples, *start);
        let measured_name = placement
            .iter()
            .find(|estimate| estimate.piece_index == piece_index)
            .map_or(name.clone(), |estimate| estimate.piece_id.clone());
        let (front_car, worst_car) = force_peaks_by_piece
            .get(&piece_index)
            .copied()
            .unwrap_or((None, None));
        by_piece.insert(
            piece_index,
            FinishedTrackEstimate {
                piece_name: measured_name,
                entry_speed,
                front_car,
                worst_car,
            },
        );
    }
    Ok(FinishedTrackEstimates {
        by_piece,
        piece_starts: piece_starts.into_iter().map(|(start, _)| start).collect(),
        circuit_length: distance,
    })
}

#[derive(Clone, Copy)]
enum ForcePeakMetric {
    MinimumVerticalG,
    MaximumVerticalG,
    MaximumLateralG,
    MaximumLongitudinalG,
}

struct PlacementPeakContextResolver<'a> {
    car_peak_locations: &'a [CarPieceForcePeakLocations],
    train_spacing: f64,
    car_count: usize,
    piece_starts: &'a [f64],
    circuit_length: f64,
}

impl<'a> PlacementPeakContextResolver<'a> {
    fn new(
        car_peak_locations: &'a [CarPieceForcePeakLocations],
        train_spacing: f64,
        car_count: usize,
        piece_starts: &'a [f64],
        circuit_length: f64,
    ) -> Self {
        Self {
            car_peak_locations,
            train_spacing,
            car_count,
            piece_starts,
            circuit_length,
        }
    }

    fn for_piece(
        &self,
        piece_index: usize,
        measured_worst: Option<ForcePeaks>,
    ) -> PlacementPeakContexts {
        let Some(measured_worst) = measured_worst else {
            return PlacementPeakContexts::default();
        };
        PlacementPeakContexts {
            minimum_vertical_g: self.peak_context(
                piece_index,
                measured_worst.minimum_vertical_g,
                ForcePeakMetric::MinimumVerticalG,
            ),
            maximum_vertical_g: self.peak_context(
                piece_index,
                measured_worst.maximum_vertical_g,
                ForcePeakMetric::MaximumVerticalG,
            ),
            maximum_lateral_g: self.peak_context(
                piece_index,
                measured_worst.maximum_lateral_g,
                ForcePeakMetric::MaximumLateralG,
            ),
            maximum_longitudinal_g: self.peak_context(
                piece_index,
                measured_worst.maximum_longitudinal_g,
                ForcePeakMetric::MaximumLongitudinalG,
            ),
        }
    }

    fn peak_context(
        &self,
        piece_index: usize,
        measured_value: f64,
        metric: ForcePeakMetric,
    ) -> Option<PlacementPeakContext> {
        let mut context = None;
        for car_peaks in self
            .car_peak_locations
            .iter()
            .filter(|peaks| peaks.piece_index == piece_index)
        {
            let (peak_value, locations) = match metric {
                ForcePeakMetric::MinimumVerticalG => (
                    car_peaks.peaks.minimum_vertical_g,
                    car_peaks.minimum_vertical_g.as_slice(),
                ),
                ForcePeakMetric::MaximumVerticalG => (
                    car_peaks.peaks.maximum_vertical_g,
                    car_peaks.maximum_vertical_g.as_slice(),
                ),
                ForcePeakMetric::MaximumLateralG => (
                    car_peaks.peaks.maximum_lateral_g,
                    car_peaks.maximum_lateral_g.as_slice(),
                ),
                ForcePeakMetric::MaximumLongitudinalG => (
                    car_peaks.peaks.maximum_longitudinal_g,
                    car_peaks.maximum_longitudinal_g.as_slice(),
                ),
            };
            if peak_value != measured_value {
                continue;
            }
            for location in locations {
                let mut peak_context = PlacementPeakContext {
                    peak_car_index: location.car_index,
                    later_car_index: None,
                    later_piece_index: None,
                };
                for car_index in 0..self.car_count {
                    let car_distance = (location.distance_m
                        + (location.car_index as f64 - car_index as f64) * self.train_spacing)
                        .rem_euclid(self.circuit_length);
                    let car_piece_index = self
                        .piece_starts
                        .partition_point(|piece_start| *piece_start <= car_distance)
                        .saturating_sub(1);
                    if car_piece_index > piece_index {
                        peak_context.later_car_index = Some(car_index);
                        peak_context.later_piece_index = Some(car_piece_index);
                        return Some(peak_context);
                    }
                }
                context.get_or_insert(peak_context);
            }
        }
        context
    }
}

fn speed_at_distance(samples: &[TestingSample], distance: f64) -> Option<f64> {
    let upper_index = samples.partition_point(|sample| sample.distance < distance);
    let next = samples.get(upper_index)?;
    if next.distance == distance {
        return Some(next.speed);
    }
    let previous = samples.get(upper_index.checked_sub(1)?)?;
    let fraction = (distance - previous.distance) / (next.distance - previous.distance);
    Some(previous.speed + fraction * (next.speed - previous.speed))
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
    let mut text = String::from(
        "piece check2 check3 entry[placement/finished/test m/s] placement[min/max/lat/long g] finished-front[min/max/lat/long g] finished-worst[min/max/lat/long g] measured-front[min/max/lat/long g] measured-worst[min/max/lat/long g]\n",
    );
    for row in &report.rows {
        writeln!(
            text,
            "{} {:<24} {:?} {:?} {}/{}/{} {} {} {} {} {}",
            row.piece_index,
            row.piece_name,
            row.check_2,
            row.check_3,
            format_optional(row.placement_entry_speed_estimate),
            format_optional(row.entry_speed_estimate),
            format_optional(row.entry_speed_measured_front),
            format_force_peaks(row.placement_estimate),
            format_force_peaks(row.finished_track_estimate),
            format_force_peaks(row.finished_track_worst_car_estimate),
            format_force_peaks(row.measured_front),
            format_force_peaks(row.measured_worst)
        )
        .unwrap();
    }
    for failure in &report.failures {
        writeln!(
            text,
            "{} {:?}: {} ({}){}{}",
            failure.piece_index,
            failure.check,
            failure.metric,
            failure.reason,
            failure
                .classification
                .map_or_else(String::new, |classification| {
                    format!(
                        " [{}]",
                        placement_failure_classification_name(classification)
                    )
                }),
            failure
                .cause
                .as_ref()
                .map_or_else(String::new, |cause| format!("; cause: {cause}"))
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

pub(crate) fn agreement_json(report: &AgreementReport) -> Value {
    let rows: Vec<_> = report
        .rows
        .iter()
        .map(|row| {
            json!({
                "pieceIndex": row.piece_index,
                "pieceName": row.piece_name,
                "placementEntrySpeedEstimate": row.placement_entry_speed_estimate,
                "finishedEntrySpeedEstimate": row.entry_speed_estimate,
                "entrySpeedMeasuredFront": row.entry_speed_measured_front,
                "placementEstimate": force_peaks_json(row.placement_estimate),
                "finishedTrackEstimate": force_peaks_json(row.finished_track_estimate),
                "finishedTrackWorstCarEstimate": force_peaks_json(row.finished_track_worst_car_estimate),
                "measuredFront": force_peaks_json(row.measured_front),
                "measuredWorst": force_peaks_json(row.measured_worst),
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
                "classification": failure.classification.map(placement_failure_classification_name),
                "cause": failure.cause,
            })
        })
        .collect();
    let downstream_track_failures = report
        .failures
        .iter()
        .filter(|failure| {
            failure.classification == Some(PlacementFailureClassification::DownstreamTrack)
        })
        .count();
    let estimate_error_failures = report
        .failures
        .iter()
        .filter(|failure| {
            failure.classification == Some(PlacementFailureClassification::EstimateError)
        })
        .count();
    json!({
        "passed": report.passed,
        "placementFailureCounts": {
            "downstreamTrack": downstream_track_failures,
            "estimateError": estimate_error_failures,
        },
        "rows": rows,
        "failures": failures,
    })
}

fn placement_failure_classification_name(
    classification: PlacementFailureClassification,
) -> &'static str {
    match classification {
        PlacementFailureClassification::DownstreamTrack => "downstream track",
        PlacementFailureClassification::EstimateError => "estimate error",
    }
}

fn format_force_peaks(peaks: Option<ForcePeaks>) -> String {
    peaks.map_or_else(
        || "—".to_owned(),
        |peaks| {
            format!(
                "{:.3}/{:.3}/{:.3}/{:.3}",
                peaks.minimum_vertical_g,
                peaks.maximum_vertical_g,
                peaks.maximum_lateral_g,
                peaks.maximum_longitudinal_g
            )
        },
    )
}

fn format_optional(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"))
}

fn force_peaks_json(peaks: Option<ForcePeaks>) -> Value {
    peaks.map_or(Value::Null, |peaks| {
        json!({
            "minimumVerticalG": peaks.minimum_vertical_g,
            "maximumVerticalG": peaks.maximum_vertical_g,
            "maximumLateralG": peaks.maximum_lateral_g,
            "maximumLongitudinalG": peaks.maximum_longitudinal_g,
        })
    })
}

fn run_check(paths: &[String], json_mode: bool) -> Result<CliResponse, String> {
    if paths.is_empty() {
        return Err("usage: tools/design check <ride file> ...".into());
    }
    let mut rows = Vec::new();
    let mut text = String::new();
    let mut failed_ride_summaries = Vec::new();
    for path in paths {
        let bytes = fs::read(path).map_err(|error| format!("read ride {path}: {error}"))?;
        let ride = RideFile::from_json_slice(&bytes)
            .map_err(|error| format!("parse ride {path}: {error}"))?;
        let model = validate_model(&ride.model)?;
        theme_park_sim::track::saved_shape_check::check_saved_shaped_geometry(&ride)
            .map_err(|error| format!("{path}: stale shaped geometry: {error}"))?;
        let (test_run, estimates, browser_estimates, circuit) =
            test_ride_with_browser_estimates(&ride).map_err(|error| format!("{path}: {error}"))?;
        let agreement = compare_estimates(&ride, &test_run, &estimates)?;
        let rated_speed_margins = crate::report::rated_speed_margins(&circuit, &model)
            .map_err(|error| format!("{path}: {error}"))?;
        let rated_speed_failures = rated_speed_margin_failures(&rated_speed_margins);
        let default_test_run = default_physics_test_run(&circuit, &model);
        let force_headroom = [
            force_headroom_results(
                &default_test_run,
                model.comfortable_force_limits,
                "default physics",
            ),
            force_headroom_results(
                &test_run,
                model.comfortable_force_limits,
                "construction-test physics",
            ),
        ]
        .concat();
        let force_headroom_failures: Vec<_> = force_headroom
            .iter()
            .filter(|result| !result.passed)
            .map(|result| result.failure())
            .collect();
        let expectation_results = evaluate_expectations(&ride, &test_run, &model, &circuit)?;
        let default_expectation_results =
            evaluate_expectations(&ride, &default_test_run, &model, &circuit)?;
        let failures: Vec<_> = expectation_results
            .iter()
            .filter(|result| !result.passed)
            .map(ExpectationResult::format)
            .collect();
        let default_expectation_failures: Vec<_> = default_expectation_results
            .iter()
            .filter(|result| !result.passed)
            .map(|result| format!("default physics: {}", result.format()))
            .collect();
        let design_margins_required = ride
            .expect
            .as_ref()
            .and_then(|expectation| expectation.design_margins)
            .unwrap_or(false);
        let failed_gates = check_failed_gates(
            &failures,
            &default_expectation_failures,
            &agreement,
            &rated_speed_failures,
            &force_headroom_failures,
            design_margins_required,
        );
        if !failed_gates.is_empty() {
            failed_ride_summaries.push(format!("{}: {}", ride.name, failed_gates.join(", ")));
        }
        writeln!(
            text,
            "{}: expectations {}; native agreement {}",
            ride.name,
            if failures.is_empty() && default_expectation_failures.is_empty() {
                "pass"
            } else {
                "fail"
            },
            if agreement.passed { "pass" } else { "fail" }
        )
        .unwrap();
        text.push_str(&crate::report::format_rated_speed_margins(
            &rated_speed_margins,
        ));
        text.push_str(&format_force_headroom_results(&force_headroom));
        for result in &expectation_results {
            writeln!(text, "  {}", result.format()).unwrap();
        }
        for result in &default_expectation_results {
            writeln!(text, "  default physics {}", result.format()).unwrap();
        }
        for failure in &rated_speed_failures {
            writeln!(text, "  rated speed margin failure: {failure}").unwrap();
        }
        for failure in &force_headroom_failures {
            writeln!(text, "  force headroom failure: {failure}").unwrap();
        }
        for failure in &agreement.failures {
            writeln!(
                text,
                "  piece {} {:?} {}: {}",
                failure.piece_index, failure.check, failure.metric, failure.reason
            )
            .unwrap();
        }
        rows.push(json!({
            "path": path,
            "name": ride.name,
            "agreementGate": "enabled; finished-track agreement failures exit nonzero",
            "ratedSpeedMargins": rated_speed_margins.iter().map(|margin| json!({
                "pieceIndex": margin.piece_index,
                "pieceId": margin.piece_id,
                "carIndex": margin.car_index,
                "entrySpeed": margin.entry_speed,
                "ratedWindow": [margin.window.0, margin.window.1],
                "ruleMargin": margin.rule_margin,
                "status": if margin.unavailable_reason.is_some() {
                    "UNAVAILABLE"
                } else {
                    match margin.rule_margin {
                        Some(rule_margin) if rule_margin >= 0.0 => "PASS",
                        Some(_) => "FAIL",
                        None => "UNAVAILABLE",
                    }
                },
                "unavailableReason": margin.unavailable_reason,
                "physicsConfiguration": if margin.construction_test { "construction-test" } else { "default" },
            })).collect::<Vec<_>>(),
            "expectations": expectation_results.iter().map(ExpectationResult::json).collect::<Vec<_>>(),
            "expectationFailures": failures,
            "defaultPhysicsExpectations": default_expectation_results.iter().map(ExpectationResult::json).collect::<Vec<_>>(),
            "defaultPhysicsExpectationFailures": default_expectation_failures,
            "ratedSpeedMarginFailures": rated_speed_failures,
            "forceHeadroom": force_headroom,
            "forceHeadroomFailures": force_headroom_failures,
            "agreement": agreement_json(&agreement),
            "browserEstimates": browser_estimates,
            "reportSamples": test_run.samples,
        }));
    }
    check_response(json_mode, rows, text, &failed_ride_summaries)
}

/// Names the check gates a ride failed; rated-margin and force-headroom failures only gate
/// rides whose expectations set `designMargins`.
fn check_failed_gates(
    expectation_failures: &[String],
    default_expectation_failures: &[String],
    agreement: &AgreementReport,
    rated_speed_failures: &[String],
    force_headroom_failures: &[String],
    design_margins_required: bool,
) -> Vec<&'static str> {
    [
        (!expectation_failures.is_empty(), "expectations"),
        (
            !default_expectation_failures.is_empty(),
            "default physics expectations",
        ),
        (!agreement.passed, "native agreement"),
        (
            design_margins_required && !rated_speed_failures.is_empty(),
            "rated speed margins",
        ),
        (
            design_margins_required && !force_headroom_failures.is_empty(),
            "force headroom",
        ),
    ]
    .into_iter()
    .filter_map(|(failed, gate)| failed.then_some(gate))
    .collect()
}

/// A failing check prints its full report on stdout and exits nonzero. Text mode also writes a
/// short summary of the failing rides to stderr; JSON mode keeps stderr empty.
fn check_response(
    json_mode: bool,
    rows: Vec<Value>,
    text: String,
    failed_ride_summaries: &[String],
) -> Result<CliResponse, String> {
    let ride_count = rows.len();
    let stdout = if json_mode {
        serde_json::to_string_pretty(&Value::Array(rows)).map_err(|error| error.to_string())? + "\n"
    } else {
        text
    };
    if failed_ride_summaries.is_empty() {
        return Ok(CliResponse::Success(stdout));
    }
    let stderr = if json_mode {
        String::new()
    } else {
        format!(
            "check failed for {} of {ride_count} ride(s); the report is on stdout:\n{}",
            failed_ride_summaries.len(),
            failed_ride_summaries
                .iter()
                .map(|summary| format!("  {summary}\n"))
                .collect::<String>()
        )
    };
    Ok(CliResponse::Failure { stdout, stderr })
}

pub(crate) fn check_expectations(
    ride: &RideFile,
    test_run: &TestRun,
    circuit: &Circuit,
) -> Result<Vec<String>, String> {
    let model = validate_model(&ride.model)?;
    Ok(evaluate_expectations(ride, test_run, &model, circuit)?
        .into_iter()
        .filter(|result| !result.passed)
        .map(|result| result.format())
        .collect())
}

struct ExpectationResult {
    metric: &'static str,
    expected: String,
    observed: Value,
    observed_text: String,
    passed: bool,
}

impl ExpectationResult {
    fn format(&self) -> String {
        format!(
            "expectation {} expected {}, observed {} ({})",
            self.metric,
            self.expected,
            self.observed_text,
            if self.passed { "pass" } else { "fail" }
        )
    }

    fn json(&self) -> Value {
        json!({
            "metric": self.metric,
            "expected": self.expected,
            "observed": self.observed,
            "passed": self.passed,
        })
    }
}

fn evaluate_expectations(
    ride: &RideFile,
    test_run: &TestRun,
    model: &CoasterModel,
    circuit: &Circuit,
) -> Result<Vec<ExpectationResult>, String> {
    let Some(expect) = &ride.expect else {
        return Ok(Vec::new());
    };
    let mut results = Vec::new();
    let sustained = test_run.sustained_force_peaks();
    if let Some([minimum, maximum]) = expect.excitement {
        results.push(range_expectation(
            "excitement",
            minimum,
            maximum,
            Some(test_run.excitement),
        ));
    }
    if let Some([minimum, maximum]) = expect.sustained_max_vertical_g {
        results.push(range_expectation(
            "sustained maximum vertical g",
            minimum,
            maximum,
            sustained.maximum_vertical_g,
        ));
    }
    if let Some([minimum, maximum]) = expect.sustained_min_vertical_g {
        results.push(range_expectation(
            "sustained minimum vertical g",
            minimum,
            maximum,
            sustained.minimum_vertical_g,
        ));
    }
    if let Some(maximum) = expect.max_lateral_g {
        let observed = test_run.statistics.max_lateral_g;
        results.push(ExpectationResult {
            metric: "maximum lateral g",
            expected: format!("at most {maximum:.2}"),
            observed: json!(observed),
            observed_text: format!("{observed:.2}"),
            passed: observed <= maximum,
        });
    }
    if let Some(inversions) = expect.inversions {
        let observed = test_run.statistics.inversions;
        results.push(count_expectation(
            "inversions",
            inversions,
            observed,
            observed == inversions as usize,
        ));
    }
    if let Some(minimum) = expect.airtime_moments_at_least {
        let observed = test_run.statistics.airtime_moments;
        results.push(ExpectationResult {
            metric: "airtime moments",
            expected: format!("at least {minimum}"),
            observed: json!(observed),
            observed_text: observed.to_string(),
            passed: observed >= minimum as usize,
        });
    }
    if let Some(expected) = expect.within_comfortable_force_limits {
        let stats = test_run.statistics;
        let limits = model.comfortable_force_limits;
        let within_limits = stats.max_positive_vertical_g <= limits.maximum_vertical_g
            && stats.max_negative_vertical_g >= limits.minimum_vertical_g
            && stats.max_lateral_g <= limits.maximum_lateral_g
            && stats.max_forward_g.abs() <= limits.maximum_longitudinal_g
            && stats.max_braking_g.abs() <= limits.maximum_longitudinal_g;
        results.push(ExpectationResult {
            metric: "comfortable force limits",
            expected: if expected {
                "within limits"
            } else {
                "outside limits"
            }
            .to_owned(),
            observed: json!(within_limits),
            observed_text: if within_limits {
                "within limits"
            } else {
                "outside limits"
            }
            .to_owned(),
            passed: expected == within_limits,
        });
    }
    if let Some(minimum) = expect.inversions_at_least {
        let observed = test_run.statistics.inversions;
        results.push(ExpectationResult {
            metric: "inversions",
            expected: format!("at least {minimum}"),
            observed: json!(observed),
            observed_text: observed.to_string(),
            passed: observed >= minimum as usize,
        });
    }
    if let Some(launches) = expect.launches {
        let observed = test_run.statistics.launches;
        results.push(count_expectation(
            "launches",
            launches,
            observed,
            observed == launches as usize,
        ));
    }
    if let Some(minimum) = expect.biggest_drop_at_least {
        results.push(minimum_expectation(
            "biggest drop",
            minimum,
            test_run.statistics.biggest_drop,
        ));
    }
    if let Some(minimum) = expect.max_speed_at_least {
        results.push(minimum_expectation(
            "maximum speed",
            minimum,
            test_run.statistics.max_speed,
        ));
    }
    if let Some(expected) = expect.inverted_seconds {
        let observed = test_run.statistics.inverted_seconds;
        results.push(ExpectationResult {
            metric: "inverted time",
            expected: format!("{expected:.2}"),
            observed: json!(observed),
            observed_text: format!("{observed:.2}"),
            passed: observed == expected,
        });
    }
    if let Some(features) = &expect.features {
        results.extend(evaluate_feature_expectations(
            &ride.steps,
            features,
            circuit,
        )?);
    }
    Ok(results)
}

fn evaluate_feature_expectations(
    ride_steps: &[RideStep],
    features: &RideExpectFeatures,
    circuit: &Circuit,
) -> Result<Vec<ExpectationResult>, String> {
    if features
        .piece_prefixes_at_least
        .iter()
        .any(|(prefix, minimum)| prefix.is_empty() || *minimum == 0)
        || features.piece_prefix_groups_at_least.iter().any(|group| {
            group.prefixes.is_empty()
                || group.prefixes.iter().any(String::is_empty)
                || group.minimum == 0
        })
        || features.heartlined_inversions_at_least == Some(0)
        || features.piece_prefixes_after.iter().any(|expectation| {
            expectation.prefix.is_empty() || !valid_feature_step(&expectation.after)
        })
        || features
            .ordered_steps
            .iter()
            .chain(&features.adjacent_steps)
            .any(|sequence| sequence.is_empty())
        || features
            .ordered_steps
            .iter()
            .chain(&features.adjacent_steps)
            .flatten()
            .any(|step| !valid_feature_step(step))
    {
        return Err("ride expect.features contains an empty prefix or step sequence".into());
    }

    let mut results = Vec::new();
    for (prefix, minimum) in &features.piece_prefixes_at_least {
        let observed = ride_steps
            .iter()
            .filter(|step| {
                ride_step_piece_id(step).is_some_and(|piece_id| piece_id.starts_with(prefix))
            })
            .count();
        results.push(ExpectationResult {
            metric: "ride feature count",
            expected: format!("at least {minimum} pieces with prefix {prefix:?}"),
            observed: json!(observed),
            observed_text: observed.to_string(),
            passed: observed >= *minimum as usize,
        });
    }
    for group in &features.piece_prefix_groups_at_least {
        let observed = ride_steps
            .iter()
            .filter(|step| {
                ride_step_piece_id(step).is_some_and(|piece_id| {
                    group
                        .prefixes
                        .iter()
                        .any(|prefix| piece_id.starts_with(prefix))
                })
            })
            .count();
        results.push(ExpectationResult {
            metric: "grouped ride feature count",
            expected: format!(
                "at least {} pieces with any prefix {:?}",
                group.minimum, group.prefixes
            ),
            observed: json!(observed),
            observed_text: observed.to_string(),
            passed: observed >= group.minimum as usize,
        });
    }
    if let Some(minimum) = features.heartlined_inversions_at_least {
        let observed = circuit
            .pieces()
            .iter()
            .filter(|piece| {
                piece.inversion.is_some() && piece.heartlining == Some(Heartlining::Heartlined)
            })
            .count();
        results.push(ExpectationResult {
            metric: "heartlined inversion pieces",
            expected: format!("at least {minimum}"),
            observed: json!(observed),
            observed_text: observed.to_string(),
            passed: observed >= minimum as usize,
        });
    }
    for expectation in &features.piece_prefixes_after {
        let after_index = ride_steps
            .iter()
            .position(|step| ride_feature_step_matches(step, &expectation.after));
        let matching_indices: Vec<_> = ride_steps
            .iter()
            .enumerate()
            .filter(|(_, step)| {
                ride_step_piece_id(step)
                    .is_some_and(|piece_id| piece_id.starts_with(&expectation.prefix))
            })
            .map(|(step_index, _)| step_index)
            .collect();
        let passed = after_index.is_some_and(|after_index| {
            !matching_indices.is_empty()
                && matching_indices
                    .iter()
                    .all(|step_index| *step_index > after_index)
        });
        results.push(ExpectationResult {
            metric: "ride feature prefix ordering",
            expected: format!(
                "all pieces with prefix {:?} after {}",
                expectation.prefix,
                describe_feature_sequence(std::slice::from_ref(&expectation.after))
            ),
            observed: json!(passed),
            observed_text: if passed { "found" } else { "not found" }.to_owned(),
            passed,
        });
    }
    for sequence in &features.ordered_steps {
        let passed = contains_ordered_feature_sequence(ride_steps, sequence);
        results.push(ExpectationResult {
            metric: "ordered ride feature steps",
            expected: describe_feature_sequence(sequence),
            observed: json!(passed),
            observed_text: if passed { "found" } else { "not found" }.to_owned(),
            passed,
        });
    }
    for sequence in &features.adjacent_steps {
        let passed = contains_adjacent_feature_sequence(ride_steps, sequence);
        results.push(ExpectationResult {
            metric: "adjacent ride feature steps",
            expected: describe_feature_sequence(sequence),
            observed: json!(passed),
            observed_text: if passed { "found" } else { "not found" }.to_owned(),
            passed,
        });
    }
    Ok(results)
}

fn valid_feature_step(step: &RideExpectFeatureStep) -> bool {
    match step {
        RideExpectFeatureStep::Place(place) => !place.place.is_empty(),
        RideExpectFeatureStep::PlacePrefix(place) => !place.place_prefix.is_empty(),
        RideExpectFeatureStep::Chain(_) => true,
    }
}

fn ride_step_piece_id(ride_step: &RideStep) -> Option<&str> {
    match ride_step {
        RideStep::Place(place) => Some(&place.place),
        RideStep::Shape(shape) => Some(match &shape.spec {
            ElementSpec::Hill { .. } => "ShapedHill",
            ElementSpec::Turn {
                hand: Hand::Left, ..
            } => "ShapedTurnLeft",
            ElementSpec::Turn {
                hand: Hand::Right, ..
            } => "ShapedTurnRight",
        }),
        _ => None,
    }
}

fn contains_ordered_feature_sequence(
    ride_steps: &[RideStep],
    feature_sequence: &[RideExpectFeatureStep],
) -> bool {
    let mut ride_step_index = 0;
    for expected_step in feature_sequence {
        while ride_step_index < ride_steps.len()
            && !ride_feature_step_matches(&ride_steps[ride_step_index], expected_step)
        {
            ride_step_index += 1;
        }
        if ride_step_index == ride_steps.len() {
            return false;
        }
        ride_step_index += 1;
    }
    true
}

fn contains_adjacent_feature_sequence(
    ride_steps: &[RideStep],
    feature_sequence: &[RideExpectFeatureStep],
) -> bool {
    ride_steps.windows(feature_sequence.len()).any(|window| {
        window
            .iter()
            .zip(feature_sequence)
            .all(|(ride_step, expected_step)| ride_feature_step_matches(ride_step, expected_step))
    })
}

fn ride_feature_step_matches(ride_step: &RideStep, expected_step: &RideExpectFeatureStep) -> bool {
    match (ride_step, expected_step) {
        (RideStep::Place(_) | RideStep::Shape(_), RideExpectFeatureStep::Place(expected)) => {
            ride_step_piece_id(ride_step).is_some_and(|piece_id| piece_id == expected.place)
        }
        (RideStep::Place(_) | RideStep::Shape(_), RideExpectFeatureStep::PlacePrefix(expected)) => {
            ride_step_piece_id(ride_step)
                .is_some_and(|piece_id| piece_id.starts_with(&expected.place_prefix))
        }
        (RideStep::Chain(chain), RideExpectFeatureStep::Chain(expected)) => {
            chain.chain == expected.chain
        }
        _ => false,
    }
}

fn describe_feature_sequence(feature_sequence: &[RideExpectFeatureStep]) -> String {
    feature_sequence
        .iter()
        .map(|step| match step {
            RideExpectFeatureStep::Place(place) => format!("place {:?}", place.place),
            RideExpectFeatureStep::PlacePrefix(place) => {
                format!("place prefix {:?}", place.place_prefix)
            }
            RideExpectFeatureStep::Chain(chain) => format!("chain {}", chain.chain),
        })
        .collect::<Vec<_>>()
        .join(" then ")
}

fn range_expectation(
    metric: &'static str,
    minimum: f64,
    maximum: f64,
    observed: Option<f64>,
) -> ExpectationResult {
    let (observed, observed_text, passed) = observed.map_or_else(
        || (Value::Null, "not measured".to_owned(), false),
        |value| {
            (
                json!(value),
                format!("{value:.2}"),
                (minimum..=maximum).contains(&value),
            )
        },
    );
    ExpectationResult {
        metric,
        expected: format!("{minimum:.2} to {maximum:.2}"),
        observed,
        observed_text,
        passed,
    }
}

fn minimum_expectation(metric: &'static str, minimum: f64, observed: f64) -> ExpectationResult {
    ExpectationResult {
        metric,
        expected: format!("at least {minimum:.2}"),
        observed: json!(observed),
        observed_text: format!("{observed:.2}"),
        passed: observed >= minimum,
    }
}

fn count_expectation(
    metric: &'static str,
    expected: u32,
    observed: usize,
    passed: bool,
) -> ExpectationResult {
    ExpectationResult {
        metric,
        expected: expected.to_string(),
        observed: json!(observed),
        observed_text: observed.to_string(),
        passed,
    }
}

fn export_session(ride: &RideFile, session: &BuilderSession) -> Result<Value, String> {
    let expected_placement_count = ride
        .steps
        .iter()
        .filter(|step| matches!(step, RideStep::Place(_) | RideStep::Shape(_)))
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
            RideStep::Shape(shape) => {
                let bridge_ids = &session.placed_piece_bridge_ids()[placement_index];
                placement_index += 1;
                if bridge_ids.is_empty() {
                    return Err(format!("ride step {step_index} has no bridge piece id"));
                }
                let intent_json = serde_json::to_string(&shape.intent).map_err(|error| {
                    format!("serialize shaped intent for ride step {step_index}: {error}")
                })?;
                let spec_json = serde_json::to_string(&shape.spec).map_err(|error| {
                    format!(
                        "serialize resolved shaped geometry for ride step {step_index}: {error}"
                    )
                })?;
                commands.push(json!({"tick": 0, "command": {
                    "type": "construction-append-resolved",
                    "intentJson": intent_json,
                    "specJson": spec_json,
                }}));
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

/// Shaped-element forces print to two decimals, as the browser's element inspector shows them.
fn optional_g(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_owned(), |value| format!("{value:.2}"))
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

#[cfg(test)]
mod agreement_replay_tests {
    use super::{
        agreement_json, check_failed_gates, check_response, cli_output, replay_with_estimates,
        speed_at_distance,
    };
    use crate::agreement::{AgreementCheck, AgreementFailure, AgreementReport};
    use theme_park_sim::track::ride_file::RideFile;
    use theme_park_sim_wasm::reports::TestingSample;

    #[test]
    fn placement_agreement_failure_is_reported_without_failing_json_check() {
        let agreement = AgreementReport {
            rows: Vec::new(),
            failures: vec![AgreementFailure {
                piece_index: 1,
                piece_name: "Launch".into(),
                check: AgreementCheck::PlacementEstimateVsWorstCar,
                metric: "longitudinal g",
                reason: "estimate differs from the worst-car measurement".into(),
                classification: Some(
                    crate::agreement::PlacementFailureClassification::EstimateError,
                ),
                cause: Some("no later piece was occupied at the peak".into()),
            }],
            passed: true,
        };

        assert!(check_failed_gates(&[], &[], &agreement, &[], &[], false).is_empty());
        let response = check_response(
            true,
            vec![serde_json::json!({"agreement": agreement_json(&agreement)})],
            String::new(),
            &[],
        );
        let output = cli_output(response);

        assert_eq!(output.exit_code, 0);
        assert!(output.stderr.is_empty());
        let reports: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
        assert_eq!(reports[0]["agreement"]["passed"], true);
        assert_eq!(
            reports[0]["agreement"]["failures"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            reports[0]["agreement"]["placementFailureCounts"]["estimateError"],
            1
        );
        assert_eq!(
            reports[0]["agreement"]["failures"][0]["classification"],
            "estimate error"
        );
        assert!(reports[0]["agreement"]["failures"][0]["cause"].is_string());
    }

    #[test]
    fn design_margin_failures_are_gated_only_when_required() {
        let agreement = AgreementReport {
            rows: Vec::new(),
            failures: Vec::new(),
            passed: true,
        };
        let rated_failures = ["piece 3 rated margin -0.20 m/s".to_owned()];
        let force_failures = ["default physics maximum vertical g headroom -0.01 g".to_owned()];

        assert_eq!(
            check_failed_gates(&[], &[], &agreement, &rated_failures, &[], true),
            ["rated speed margins"]
        );
        assert!(check_failed_gates(&[], &[], &agreement, &rated_failures, &[], false).is_empty());
        assert_eq!(
            check_failed_gates(&[], &[], &agreement, &[], &force_failures, true),
            ["force headroom"]
        );
        assert!(check_failed_gates(&[], &[], &agreement, &[], &force_failures, false).is_empty());
        assert_eq!(
            check_failed_gates(
                &[],
                &["default physics: expectation excitement failed".to_owned()],
                &agreement,
                &[],
                &[],
                false,
            ),
            ["default physics expectations"]
        );

        let json_output = cli_output(check_response(
            true,
            vec![serde_json::json!({"ratedSpeedMarginFailures": rated_failures})],
            String::new(),
            &["Test ride: rated speed margins".to_owned()],
        ));
        assert_eq!(json_output.exit_code, 1);
        assert!(json_output.stderr.is_empty());

        let text_output = cli_output(check_response(
            false,
            vec![serde_json::json!({}), serde_json::json!({})],
            "full report\n".to_owned(),
            &["Test ride: force headroom".to_owned()],
        ));
        assert_eq!(text_output.exit_code, 1);
        assert_eq!(text_output.stdout, "full report\n");
        assert_eq!(
            text_output.stderr,
            "check failed for 1 of 2 ride(s); the report is on stdout:\n  Test ride: force headroom\n"
        );
    }

    #[test]
    fn entry_speed_is_interpolated_at_the_piece_boundary_without_extrapolation() {
        let samples = [
            TestingSample {
                distance: 1.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                speed: 2.0,
                vertical_g: 1.0,
                lateral_g: 0.0,
                longitudinal_g: 0.0,
                airtime: false,
                force_limit_exceeded: [false; 3],
            },
            TestingSample {
                distance: 3.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                speed: 6.0,
                vertical_g: 1.0,
                lateral_g: 0.0,
                longitudinal_g: 0.0,
                airtime: false,
                force_limit_exceeded: [false; 3],
            },
        ];

        assert_eq!(speed_at_distance(&samples, 2.0), Some(4.0));
        assert_eq!(speed_at_distance(&samples, 0.0), None);
        assert_eq!(speed_at_distance(&samples, 4.0), None);
    }

    #[test]
    fn placement_estimates_cover_each_physical_piece_in_an_expanded_element() {
        let ride: RideFile = serde_json::from_str(
            r#"{
                "format": "heartline-ride",
                "version": 1,
                "name": "Element agreement fixture",
                "model": "velocity_viper",
                "station": { "x": 100, "y": 100, "z": 10, "heading": "east" },
                "carsPerTrain": 6,
                "trains": 1,
                "steps": [
                    { "place": "LaunchLong" },
                    { "place": "element:small-left-banked-turn" }
                ]
            }"#,
        )
        .unwrap();

        let (_, estimates, _) = replay_with_estimates(&ride, false).unwrap();

        assert_eq!(estimates.len(), 4);
        assert_eq!(
            estimates
                .iter()
                .map(|estimate| estimate.piece_index)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert!(
            estimates
                .iter()
                .all(|estimate| estimate.estimated.is_some())
        );
    }
}

#[cfg(test)]
mod placement_peak_context_tests {
    use super::PlacementPeakContextResolver;
    use crate::agreement::ForcePeaks;
    use theme_park_sim::physics::{CarPieceForcePeakLocations, ForcePeakLocation, ForcePeakValues};

    fn car_peak_locations(
        piece_index: usize,
        car_index: usize,
        distance_m: f64,
    ) -> CarPieceForcePeakLocations {
        let location = ForcePeakLocation {
            piece_index,
            distance_m,
            car_index,
        };
        CarPieceForcePeakLocations {
            piece_index,
            car_index,
            peaks: ForcePeakValues {
                minimum_vertical_g: -1.0,
                maximum_vertical_g: 3.0,
                maximum_lateral_g: 0.5,
                maximum_longitudinal_g: 0.8,
            },
            minimum_vertical_g: vec![location],
            maximum_vertical_g: vec![location],
            maximum_lateral_g: vec![location],
            maximum_longitudinal_g: vec![location],
        }
    }

    #[test]
    fn peak_context_uses_train_spacing_and_wraps_closed_track_positions() {
        let measured = ForcePeaks {
            minimum_vertical_g: -1.0,
            maximum_vertical_g: 3.0,
            maximum_lateral_g: 0.5,
            maximum_longitudinal_g: 0.8,
        };
        let piece_starts = [0.0, 10.0, 20.0, 30.0];
        let downstream_peak_locations = [car_peak_locations(1, 2, 18.0)];
        let downstream = PlacementPeakContextResolver::new(
            &downstream_peak_locations,
            2.0,
            3,
            &piece_starts,
            40.0,
        )
        .for_piece(1, Some(measured));
        assert_eq!(
            downstream.maximum_longitudinal_g.unwrap().later_piece_index,
            Some(2)
        );

        let wrapped_peak_locations = [car_peak_locations(1, 3, 18.0)];
        let wrapped =
            PlacementPeakContextResolver::new(&wrapped_peak_locations, 8.0, 4, &piece_starts, 40.0)
                .for_piece(1, Some(measured));
        let wrapped_context = wrapped.maximum_longitudinal_g.unwrap();
        assert_eq!(wrapped_context.later_piece_index, Some(3));
        assert_eq!(wrapped_context.later_car_index, Some(1));

        let no_later_peak_locations = [car_peak_locations(2, 0, 25.0)];
        let no_later_piece = PlacementPeakContextResolver::new(
            &no_later_peak_locations,
            4.0,
            2,
            &piece_starts,
            40.0,
        )
        .for_piece(2, Some(measured));
        assert_eq!(
            no_later_piece
                .maximum_longitudinal_g
                .unwrap()
                .later_piece_index,
            None
        );
    }
}

#[cfg(test)]
mod expectation_tests {
    use super::*;
    use theme_park_sim::track::ride_file::RideExpect;

    #[test]
    fn piece_prefix_groups_and_after_expectations_preserve_their_quantifiers() {
        let ride: RideFile =
            serde_json::from_str(include_str!("../../../content/rides/briar-blitz.ride.json"))
                .unwrap();
        let circuit = theme_park_sim::physics::demo_circuit();
        let features: RideExpectFeatures = serde_json::from_value(serde_json::json!({
            "piecePrefixGroupsAtLeast": [{"prefixes": ["TightTwister", "Twister"], "minimum": 2}],
            "piecePrefixesAfter": [{"prefix": "TightTwister", "after": {"chain": false}}]
        }))
        .unwrap();

        let results = evaluate_feature_expectations(&ride.steps, &features, &circuit).unwrap();
        assert!(results.iter().all(|result| result.passed));
        assert!(
            results
                .iter()
                .any(|result| result.metric == "grouped ride feature count")
        );
        assert!(
            results
                .iter()
                .any(|result| result.metric == "ride feature prefix ordering")
        );

        let mut failing_features = features.clone();
        failing_features.piece_prefixes_after[0].after =
            serde_json::from_value(serde_json::json!({"place": "BankedTurnStandalone"})).unwrap();
        let failing_results =
            evaluate_feature_expectations(&ride.steps, &failing_features, &circuit).unwrap();
        assert!(
            failing_results.iter().any(|result| {
                result.metric == "ride feature prefix ordering" && !result.passed
            })
        );
    }

    #[test]
    fn shaped_piece_prefix_expectations_count_canonical_hill_and_turn_ids() {
        let ride: RideFile = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .unwrap();
        let features: RideExpectFeatures = serde_json::from_value(serde_json::json!({
            "piecePrefixesAtLeast": {"ShapedHill": 4, "ShapedTurn": 7},
            "piecePrefixGroupsAtLeast": [{"prefixes": ["ShapedHill", "ShapedTurn"], "minimum": 11}],
            "piecePrefixesAfter": [{"prefix": "ShapedTurnRight", "after": {"placePrefix": "ShapedTurnLeft"}}],
            "orderedSteps": [[{"placePrefix": "ShapedHill"}, {"placePrefix": "ShapedTurn"}]],
            "adjacentSteps": [[{"placePrefix": "ShapedHill"}, {"placePrefix": "ShapedTurn"}]]
        }))
        .unwrap();

        let results = evaluate_feature_expectations(
            &ride.steps,
            &features,
            &theme_park_sim::physics::demo_circuit(),
        )
        .unwrap();

        let counts: Vec<_> = results
            .iter()
            .filter(|result| result.metric == "ride feature count")
            .collect();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].observed, serde_json::json!(4));
        assert_eq!(counts[1].observed, serde_json::json!(14));
        assert!(counts.iter().all(|result| result.passed));
        let group = results
            .iter()
            .find(|result| result.metric == "grouped ride feature count")
            .unwrap();
        assert_eq!(group.observed, serde_json::json!(18));
        assert!(group.passed);
        assert!(results.iter().all(|result| {
            !matches!(
                result.metric,
                "ride feature prefix ordering"
                    | "ordered ride feature steps"
                    | "adjacent ride feature steps"
            ) || result.passed
        }));
    }

    #[test]
    fn sustained_expectations_ignore_short_peak_stats_but_check_held_force() {
        let mut ride: RideFile = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .unwrap();
        let (base_test_run, _, _, circuit) = test_ride_with_browser_estimates(&ride).unwrap();
        let base_sustained = base_test_run.sustained_force_peaks();
        let maximum_vertical_g = base_sustained.maximum_vertical_g.unwrap();
        let minimum_vertical_g = base_sustained.minimum_vertical_g.unwrap();
        let brief_positive_peak = maximum_vertical_g + 0.5;
        let brief_negative_peak = minimum_vertical_g - 0.5;
        let test_run = with_brief_per_car_peaks(
            &base_test_run,
            maximum_vertical_g + 0.01,
            minimum_vertical_g - 0.01,
            brief_positive_peak,
            brief_negative_peak,
        );
        let sustained_after_brief_peaks = test_run.sustained_force_peaks();
        assert_eq!(
            sustained_after_brief_peaks.maximum_vertical_g,
            base_sustained.maximum_vertical_g
        );
        assert_eq!(
            sustained_after_brief_peaks.minimum_vertical_g,
            base_sustained.minimum_vertical_g
        );

        ride.expect = Some(RideExpect {
            sustained_max_vertical_g: Some([maximum_vertical_g - 0.01, maximum_vertical_g + 0.01]),
            sustained_min_vertical_g: Some([minimum_vertical_g - 0.01, minimum_vertical_g + 0.01]),
            ..RideExpect::default()
        });

        assert!(
            check_expectations(&ride, &test_run, &circuit)
                .unwrap()
                .is_empty()
        );

        let expect = ride.expect.as_mut().unwrap();
        expect.sustained_max_vertical_g =
            Some([maximum_vertical_g + 0.1, maximum_vertical_g + 0.2]);
        expect.sustained_min_vertical_g =
            Some([minimum_vertical_g - 0.2, minimum_vertical_g - 0.1]);
        let failures = check_expectations(&ride, &test_run, &circuit).unwrap();
        assert_eq!(failures.len(), 2);
        assert!(failures[0].starts_with("expectation sustained maximum vertical g expected"));
        assert!(failures[1].starts_with("expectation sustained minimum vertical g expected"));
    }

    fn with_brief_per_car_peaks(
        test_run: &TestRun,
        maximum_target: f64,
        minimum_target: f64,
        brief_positive_peak: f64,
        brief_negative_peak: f64,
    ) -> TestRun {
        let mut test_run_value = serde_json::to_value(test_run).unwrap();
        let per_car_samples = test_run_value["rating_inputs"][0]["samples"]
            .as_array_mut()
            .unwrap();
        let positive_sample_index = brief_sample_index(per_car_samples, maximum_target, true);
        let negative_sample_index = brief_sample_index(per_car_samples, minimum_target, false);
        assert_ne!(positive_sample_index, negative_sample_index);
        per_car_samples[positive_sample_index]["vertical_g"] = json!(brief_positive_peak);
        per_car_samples[negative_sample_index]["vertical_g"] = json!(brief_negative_peak);
        test_run_value["statistics"]["max_positive_vertical_g"] = json!(brief_positive_peak);
        test_run_value["statistics"]["max_negative_vertical_g"] = json!(brief_negative_peak);

        serde_json::from_value(test_run_value).unwrap()
    }

    fn brief_sample_index(samples: &[Value], target: f64, positive: bool) -> usize {
        (1..samples.len() - 1)
            .find(|&sample_index| {
                let previous = &samples[sample_index - 1];
                let sample = &samples[sample_index];
                let next = &samples[sample_index + 1];
                let vertical_g = sample["vertical_g"].as_f64().unwrap();
                let previous_vertical_g = previous["vertical_g"].as_f64().unwrap();
                let next_vertical_g = next["vertical_g"].as_f64().unwrap();
                let neighbor_force_clearance = if positive {
                    previous_vertical_g < target && next_vertical_g < target
                } else {
                    previous_vertical_g > target && next_vertical_g > target
                };
                let local_duration_seconds = (next["distance"].as_f64().unwrap()
                    - previous["distance"].as_f64().unwrap())
                    / ((previous["speed"].as_f64().unwrap()
                        + 2.0 * sample["speed"].as_f64().unwrap()
                        + next["speed"].as_f64().unwrap())
                        / 4.0);
                neighbor_force_clearance
                    && local_duration_seconds < 0.3
                    && if positive {
                        vertical_g > 0.0
                    } else {
                        vertical_g < 0.0
                    }
            })
            .expect(
                "Horizon Hawk per-car samples must contain a short positive and negative interval",
            )
    }
}

#[cfg(test)]
mod shaped_command_parser_tests {
    use super::{
        RideCommand, parse_commands, render_choices, render_probe, shape_selection_id,
        validate_model,
    };
    use theme_park_sim::track::ride_file::{RideFile, RideStep};
    use theme_park_sim::track::shaped::ElementIntent;
    use theme_park_sim_wasm::WasmSimulation;
    use theme_park_sim_wasm::builder_session::BuilderSession;

    #[test]
    fn parses_hill_turn_and_shape_options_commands() {
        let commands = parse_commands(&[
            "place hill feel=floater g=-1.0 height=36".to_owned(),
            "place turn left feel=banked radius=64".to_owned(),
            "options shapes".to_owned(),
        ])
        .expect("shaped design-tool commands should parse");

        assert_eq!(commands.len(), 3);
        let RideCommand::Place { id: hill, .. } = &commands[0] else {
            panic!("first command should select a shaped hill");
        };
        assert_eq!(hill, "shape:hill:floater:-1:fixed-36");
        let RideCommand::Place { id: turn, .. } = &commands[1] else {
            panic!("second command should select a shaped turn");
        };
        assert_eq!(turn, "shape:turn:left:fullyBanked:fixed-64");
        assert!(matches!(
            &commands[2],
            RideCommand::Choices(Some(group)) if group == "shapes"
        ));
    }

    #[test]
    fn shaped_commands_reject_unknown_or_duplicate_parameters() {
        for command in [
            "place hill feel=launch g=-1.0",
            "place hill feel=floater feel=ejector",
            "place turn left feel=banked radius=-1",
            "options hills",
        ] {
            assert!(parse_commands(&[command.to_owned()]).is_err(), "{command}");
        }
    }

    #[test]
    fn cli_strong_floater_uses_the_builder_hill_feel_token() {
        let commands = parse_commands(&["place hill feel=strong-floater".to_owned()])
            .expect("strong floater command should parse");
        let RideCommand::Place { id, .. } = &commands[0] else {
            panic!("command should select a shaped hill");
        };
        assert_eq!(id, "shape:hill:strongFloater:default:auto");
    }

    #[test]
    fn design_default_matches_horizon_hawk_ride_and_browser_selection() {
        let commands = parse_commands(&["place hill feel=strong-floater height=24".to_owned()])
            .expect("strong floater command should parse");
        let RideCommand::Place {
            shape_request: Some(request),
            ..
        } = &commands[0]
        else {
            panic!("command should retain its shaped hill request");
        };
        let model = validate_model("hyper").expect("Horizon Hawk model is available");
        let design_intent = match request {
            super::ShapedPlaceRequest::Hill {
                feel,
                strength_g,
                size,
            } => super::ShapedPlaceRequest::Hill {
                feel: *feel,
                strength_g: *strength_g,
                size: *size,
            }
            .into_intent(&model),
            super::ShapedPlaceRequest::Turn { .. } => panic!("expected hill request"),
        };
        let ride: RideFile = serde_json::from_str(include_str!(
            "../../../content/rides/horizon-hawk.ride.json"
        ))
        .expect("Horizon Hawk shaped ride is valid");
        let ride_intent = ride
            .steps
            .iter()
            .find_map(|step| match step {
                RideStep::Shape(shape)
                    if matches!(
                        &shape.intent,
                        ElementIntent::Hill {
                            feel: theme_park_sim::track::HillFeel::StrongFloater,
                            strength_g,
                            ..
                        } if (*strength_g + 0.85).abs() <= 1e-9
                    ) =>
                {
                    Some(shape.intent.clone())
                }
                _ => None,
            })
            .expect("Horizon Hawk contains a strong floater hill");
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        simulation.construction_select("model:hyper");
        simulation.construction_start(100, 100, 20, 0);
        simulation.construction_select("shape:hill:strongFloater:default:fixed-24");

        assert_eq!(design_intent, ride_intent);
        assert_eq!(
            shape_selection_id(&design_intent).unwrap(),
            simulation.construction_selected()
        );
    }

    #[test]
    fn shape_choices_report_feasibility_and_model_ranges_at_the_open_end() {
        let simulation = WasmSimulation::new(42, 0, "new".into());
        let mut session = BuilderSession::new(simulation);
        session.start("hyper", 100, 100, 10, 0).unwrap();
        session.estimates();

        let reply = render_choices(&mut session, Some("shapes")).unwrap();
        let rows = reply.json.as_array().unwrap();

        assert_eq!(rows.len(), 5);
        assert!(rows.iter().all(|row| row["group"] == "shapes"));
        assert!(rows.iter().all(|row| !row["shape"].is_null()));
        assert!(reply.text.contains("turn radius"));
        assert!(reply.text.contains("bank up to"));
    }

    #[test]
    fn probe_reports_a_shaped_candidate_or_its_typed_refusal() {
        let simulation = WasmSimulation::new(42, 0, "new".into());
        let mut session = BuilderSession::new(simulation);
        session.start("hyper", 100, 100, 10, 0).unwrap();
        session.estimates();

        let reply = render_probe(&mut session, "shape:hill:floater:default:auto").unwrap();

        assert!(!reply.json["shape"].is_null());
        assert!(reply.text.contains("Shaped hill") || reply.text.contains("Shape refusal"));
    }
}

#[cfg(test)]
mod builder_snapshot_tests {
    use super::builder_estimates_snapshot;
    use theme_park_sim_wasm::WasmSimulation;
    use theme_park_sim_wasm::builder_session::BuilderSession;

    #[test]
    fn builder_snapshot_serializes_an_absent_candidate_preview_as_null() {
        let simulation = WasmSimulation::new(42, 0, "new".into());
        let mut session = BuilderSession::new(simulation);
        session.start("hyper", 100, 100, 10, 0).unwrap();
        let mut estimates = session.estimates();
        let selected_snapshot = builder_estimates_snapshot(&session, &estimates).unwrap();
        assert!(selected_snapshot["candidateForcePreview"].is_object());

        estimates.candidate_preview = None;

        let placement_snapshot = builder_estimates_snapshot(&session, &estimates).unwrap();

        assert!(placement_snapshot["constructionForcePreview"].is_object());
        assert!(placement_snapshot["candidateForcePreview"].is_null());
    }
}

#[cfg(test)]
mod shaped_replay_estimate_tests {
    use super::{replay_with_estimates, shape_selection_id};
    use theme_park_sim::physics::ForcePeakValues;
    use theme_park_sim::track::Hand;
    use theme_park_sim::track::ride_file::{
        RideChainStep, RideFile, RideFileFormat, RideHeading, RidePlaceStep, RideShapeStep,
        RideStation, RideStep,
    };
    use theme_park_sim::track::shaped::{ElementIntent, TurnSizeHint};
    use theme_park_sim_wasm::WasmSimulation;
    use theme_park_sim_wasm::builder_session::BuilderSession;

    #[test]
    fn shaped_placement_estimate_uses_worst_car_force_peaks() {
        let intent = ElementIntent::Turn {
            hand: Hand::Left,
            size: TurnSizeHint::Auto,
        };
        let mut spec_session = BuilderSession::new(WasmSimulation::new(42, 0, "new".into()));
        spec_session.start("hyper", 241, 249, 10, 1).unwrap();
        spec_session.set_cars_per_train(8).unwrap();
        spec_session.estimates();
        spec_session.set_chain(true).unwrap();
        let lift_pieces = [
            "LevelToGentleUp",
            "GentleToSteepUp",
            "SteepUp",
            "SteepUp",
            "SteepToGentleUp",
            "GentleUpToLevel",
        ];
        for piece in lift_pieces {
            spec_session.select(piece, true).unwrap();
            spec_session.place().unwrap();
        }
        spec_session.set_chain(false).unwrap();
        for piece in ["WideLevelToSixtyDown", "WideSixtyDownToLevel"] {
            spec_session.select(piece, true).unwrap();
            spec_session.place().unwrap();
        }
        spec_session
            .select(&shape_selection_id(&intent).unwrap(), true)
            .unwrap();
        spec_session.estimates();
        spec_session.place().unwrap();
        let spec = spec_session
            .simulation()
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .last()
            .unwrap()
            .element_spec()
            .unwrap()
            .clone();
        let mut steps = vec![RideStep::Chain(RideChainStep { chain: true })];
        steps.extend(lift_pieces.into_iter().map(|place| {
            RideStep::Place(RidePlaceStep {
                place: place.into(),
                launch_speed: None,
                note: None,
            })
        }));
        steps.push(RideStep::Chain(RideChainStep { chain: false }));
        steps.extend(
            ["WideLevelToSixtyDown", "WideSixtyDownToLevel"].map(|place| {
                RideStep::Place(RidePlaceStep {
                    place: place.into(),
                    launch_speed: None,
                    note: None,
                })
            }),
        );
        steps.push(RideStep::Shape(RideShapeStep { intent, spec }));
        let ride = RideFile {
            format: RideFileFormat::HeartlineRide,
            version: 1,
            name: "Shaped estimate source regression".into(),
            model: "hyper".into(),
            station: RideStation {
                x: 241,
                y: 249,
                z: 10,
                heading: RideHeading::North,
            },
            cars_per_train: 8,
            trains: 1,
            steps,
            expect: None,
        };

        let (mut session, estimates, _) = replay_with_estimates(&ride, false).unwrap();
        let piece_index = session
            .simulation()
            .construction
            .as_ref()
            .unwrap()
            .pieces()
            .len()
            - 1;
        let shaped_estimate = estimates
            .iter()
            .find(|estimate| estimate.piece_index == piece_index)
            .unwrap();
        let force_preview = session.estimates().force_preview.unwrap();
        let worst_car = force_preview
            .piece_force_peaks
            .iter()
            .find(|peaks| peaks.piece_index == piece_index)
            .and_then(|peaks| peaks.worst_car.as_ref())
            .unwrap();

        assert_eq!(
            shaped_estimate.estimated,
            Some(ForcePeakValues {
                minimum_vertical_g: worst_car.minimum_vertical_g,
                maximum_vertical_g: worst_car.maximum_vertical_g,
                maximum_lateral_g: worst_car.maximum_lateral_g,
                maximum_longitudinal_g: worst_car.maximum_longitudinal_g,
            })
        );
    }
}
