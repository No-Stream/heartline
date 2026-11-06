use std::fmt::Write as _;
#[cfg(not(test))]
use std::fs;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::physics::{
    Circuit, ConstructionTestRunRecorder, DEMO_STATION_ENTRY, ForceAxis, ForceBand, PhysicsConfig,
    TIME_STEP, Train, force_band, preview_open_track,
};
use theme_park_sim::track::construction::{Construction, select_speed_matched_variant_for_train};
use theme_park_sim::track::{
    CoasterModel, Hand, Heading, HillFeel, HillTwist, Piece, PieceFamily, PieceId, TrackState,
    catalog, effective_rated_speeds, hybrid_model, multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

#[derive(Default)]
struct PieceReadings {
    entries: Vec<Option<f64>>,
    vertical_min: f64,
    vertical_max: f64,
    lateral: f64,
    longitudinal: f64,
    roll_rate: f64,
    seen: bool,
}

pub fn report_for_circuit(
    text: &str,
    name: &str,
    construction_test: bool,
    close: bool,
) -> Result<String, String> {
    let model = match name {
        "Demo" | "Timber Tumbler" => hybrid_model(),
        "Velocity Viper" => multi_launch_model(),
        "Briar Blitz" => wooden_twister_model(),
        _ => return Err(format!("Unknown circuit model: {name}")),
    };
    // The demo is laid out in the default park; the player circuits use the fixtures' flat world.
    let (world, start) = if name == "Demo" {
        (demo_world(), DEMO_STATION_ENTRY)
    } else {
        (
            World::new(
                512,
                512,
                Surface {
                    corners: [8; 4],
                    water: None,
                },
            ),
            TrackState {
                x: 482,
                y: 498,
                z: 6,
                heading: Heading::North,
                ..TrackState::origin()
            },
        )
    };
    let mut construction = Construction::new(start, model.clone());
    construction
        .append(PieceId::Station, &world, &[])
        .map_err(|e| format!("Station: {e:?}"))?;
    let header = format!("[{name}]");
    let mut sections = text.split(&header);
    sections.next();
    let section = sections
        .next()
        .ok_or_else(|| format!("Missing circuit {name}"))?;
    if sections.next().is_some() {
        return Err(format!("Duplicate circuit {name}"));
    }
    let config = if construction_test {
        PhysicsConfig {
            lift_speed: 5.0,
            ..PhysicsConfig::default()
        }
    } else {
        PhysicsConfig::default()
    };
    for line in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
    {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line {
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
        let id = if line.starts_with('@') {
            let preview = preview_open_track(construction.pieces(), &model.train, config)
                .ok_or("No station")?;
            let speed = construction
                .estimated_end_speed(Some(&preview))
                .speed
                .ok_or("Cannot estimate entry speed")?;
            resolve_directive(line, speed, &model)?
        } else {
            catalog()
                .iter()
                .find(|piece| format!("{:?}", piece.id) == line)
                .ok_or_else(|| format!("Unknown piece {line}"))?
                .id
        };
        construction
            .append(id, &world, &[])
            .map_err(|e| format!("{line}: {e:?}"))?;
    }
    let end = construction.end();
    let station_exit = construction.pieces()[0].exit;
    let mut output = String::new();
    let end_pos = end.position();
    let station_pos = station_exit.position();
    writeln!(
        output,
        "{name} ({})",
        if construction_test {
            "construction-test physics"
        } else {
            "default physics"
        }
    )
    .unwrap();
    writeln!(
        output,
        "Station exit offset: x {:+.1} m, y {:+.1} m, z {:+.1} m; heading {:?} (station exit {:?})",
        end_pos.x - station_pos.x,
        end_pos.y - station_pos.y,
        end_pos.z - station_pos.z,
        end.heading,
        station_exit.heading
    )
    .unwrap();
    let original_len = construction.pieces().len();
    if close {
        if end == construction.start() {
            writeln!(output, "Completion: already closed").unwrap();
        } else if let Some(completion) = construction.find_completion(&world, &[]) {
            writeln!(output, "Completion: {:?}", completion.pieces).unwrap();
            construction
                .apply_completion(&completion, &world, &[])
                .map_err(|e| format!("Completion: {e:?}"))?;
        } else {
            writeln!(output, "Completion: no connection found").unwrap();
        }
    }
    let pieces = construction.pieces();
    let closed = pieces.last().unwrap().exit == pieces[0].entry;
    let circuit = if closed {
        Circuit::new(pieces.to_vec())
    } else {
        Circuit::open(pieces.to_vec())
    }
    .map_err(str::to_owned)?;
    let start_distance = if construction_test {
        0.0
    } else {
        pieces[0].length() / 2.0
    };
    let finish_distance = circuit.length() + if closed { start_distance } else { 0.0 };
    let mut train = Train::new(
        &circuit,
        &model.train,
        config,
        start_distance,
        if construction_test { 2.0 } else { 0.0 },
    );
    let mut recorder = ConstructionTestRunRecorder::for_circuit_with_train_and_limits(
        &circuit,
        &model.train,
        model.comfortable_force_limits,
    );
    let mut rows: Vec<PieceReadings> = (0..pieces.len())
        .map(|_| PieceReadings {
            entries: vec![None; model.train.cars as usize],
            vertical_min: f64::INFINITY,
            vertical_max: f64::NEG_INFINITY,
            ..PieceReadings::default()
        })
        .collect();
    let mut previous_roll: Vec<Option<f64>> = vec![None; model.train.cars as usize];
    let mut stationary = 0;
    let mut completed = false;
    let mut recovered = false;
    for _ in 0..200_000 {
        train.step(&circuit);
        let frames = train.take_readings();
        for frame in &frames {
            for (car, reading) in frame.iter().enumerate() {
                let car_distance = train.distance()
                    - car as f64 * model.train.length_metres / f64::from(model.train.cars);
                if car_distance < 0.0 {
                    continue;
                }
                let row = &mut rows[reading.piece_index];
                row.entries[car].get_or_insert(reading.speed);
                row.vertical_min = row.vertical_min.min(reading.g.vertical);
                row.vertical_max = row.vertical_max.max(reading.g.vertical);
                row.lateral = row.lateral.max(reading.g.lateral.abs());
                row.longitudinal = row.longitudinal.max(reading.g.longitudinal.abs());
                if let Some(previous) = previous_roll[car] {
                    let difference: f64 = (reading.roll - previous + std::f64::consts::PI)
                        .rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    row.roll_rate = row.roll_rate.max(difference.to_degrees().abs() / TIME_STEP);
                }
                previous_roll[car] = Some(reading.roll);
                row.seen = true;
            }
        }
        recorder.record(&frames);
        if train.speed().abs() < 0.01 {
            stationary += 1;
        } else {
            stationary = 0;
        }
        if !train.notes().is_empty() || stationary >= 400 {
            recovered = true;
            break;
        }
        if train.distance() >= finish_distance {
            completed = true;
            break;
        }
    }
    let report = recorder.finish(completed, recovered);
    writeln!(
        output,
        "# piece kind entry_m/s window_m/s margin_m/s v+_g v-_g lat_g long_g band roll_deg/s notes"
    )
    .unwrap();
    for (index, (piece, row)) in pieces.iter().zip(&rows).enumerate() {
        let marker = if close && index >= original_len {
            " completion"
        } else {
            ""
        };
        if !row.seen {
            writeln!(
                output,
                "{index} {:?} {:?} — — — — — — — — — not reached{marker}",
                piece.id, piece.kind
            )
            .unwrap();
            continue;
        }
        let front = row.entries[0];
        let rated = effective_rated_speeds(piece, &model.train);
        let worst = row.entries.iter().flatten().copied().min_by(|a, b| {
            let margin = |speed: f64| {
                rated.map_or(0.0, |window| {
                    (speed - window.minimum_entry).min(window.maximum_entry - speed)
                })
            };
            margin(*a).total_cmp(&margin(*b)).then_with(|| {
                (front.unwrap_or(*a) - *b)
                    .abs()
                    .total_cmp(&(front.unwrap_or(*a) - *a).abs())
            })
        });
        let entry = match (front, worst) {
            (Some(front), Some(worst)) if (front - worst).abs() > 0.5 => {
                format!("{front:.2}[{worst:.2}]")
            }
            (Some(front), _) => format!("{front:.2}"),
            _ => "—".to_owned(),
        };
        let (window, margin, warning) = match (rated, worst) {
            (Some(window), Some(speed)) => {
                let margin = (speed - window.minimum_entry).min(window.maximum_entry - speed);
                (
                    format!("{:.2}..{:.2}", window.minimum_entry, window.maximum_entry),
                    format!("{margin:+.2}"),
                    if margin < 0.0 { " OUTSIDE" } else { "" },
                )
            }
            (Some(window), None) => (
                format!("{:.2}..{:.2}", window.minimum_entry, window.maximum_entry),
                "—".to_owned(),
                "",
            ),
            _ => ("—".to_owned(), "—".to_owned(), ""),
        };
        let limits = model.comfortable_force_limits;
        let (axis, value) = if row.vertical_min < 0.0 {
            (ForceAxis::Airtime, row.vertical_min)
        } else {
            (ForceAxis::Vertical, row.vertical_max)
        };
        let mut band = force_band(axis, value, limits);
        for (axis, value) in [
            (ForceAxis::Lateral, row.lateral),
            (ForceAxis::Longitudinal, row.longitudinal),
        ] {
            match force_band(axis, value, limits) {
                ForceBand::Dangerous => band = ForceBand::Dangerous,
                ForceBand::TooMuch if band != ForceBand::Dangerous => band = ForceBand::TooMuch,
                _ => {}
            }
        }
        writeln!(output, "{index} {:?} {:?} {entry} {window} {margin} {:.2} {:.2} {:.2} {:.2} {band:?} {:.1}{warning}{marker}", piece.id, piece.kind, row.vertical_max, row.vertical_min, row.lateral, row.longitudinal, row.roll_rate).unwrap();
    }
    writeln!(
        output,
        "{}: {completed}; recovered: {}",
        if closed {
            "Completed lap"
        } else {
            "Reached end of open track"
        },
        recovered
    )
    .unwrap();
    for note in &report.notes {
        writeln!(output, "Note: {note}").unwrap();
    }
    writeln!(output, "Score breakdown:").unwrap();
    for (label, value, contributions) in [
        (
            "Excitement",
            report.excitement,
            &report.breakdown.excitement,
        ),
        ("Intensity", report.intensity, &report.breakdown.intensity),
        ("Nausea", report.nausea, &report.breakdown.nausea),
    ] {
        writeln!(output, "{label}: {value:.2}").unwrap();
        for contribution in contributions {
            writeln!(output, "  {}", contribution.describe()).unwrap();
        }
    }
    Ok(output)
}

fn resolve_directive(line: &str, speed: f64, model: &CoasterModel) -> Result<PieceId, String> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    let candidates: Vec<Piece> = catalog()
        .iter()
        .filter(|piece| model.allowed_pieces.contains(&piece.id))
        .filter(|piece| match (tokens.as_slice(), piece.family) {
            (
                ["@turn", direction, size],
                Some(PieceFamily::SpeedTurn {
                    radius_metres,
                    hand,
                    ..
                }),
            ) => {
                let expected_hand = match *direction {
                    "left" => Some(Hand::Left),
                    "right" => Some(Hand::Right),
                    _ => None,
                };
                let expected_radius = match *size {
                    "small" => Some(16),
                    "large" => Some(32),
                    _ => None,
                };
                Some(hand) == expected_hand && Some(radius_metres) == expected_radius
            }
            (["@hill", feel], Some(PieceFamily::Hill(family))) => {
                let expected = match *feel {
                    "ejector" => Some(HillFeel::Ejector),
                    "floater" => Some(HillFeel::Floater),
                    "speed" => Some(HillFeel::Speed),
                    "zero-g" => Some(HillFeel::ZeroGRoll),
                    _ => None,
                };
                Some(family.feel) == expected
                    && (family.twist.is_none()
                        || (*feel == "zero-g" && family.twist == Some(HillTwist::LeftRoll)))
            }
            (["@hill", "stall", direction], Some(PieceFamily::Hill(family))) => {
                let twist = match *direction {
                    "left" => Some(HillTwist::LeftRoll),
                    "right" => Some(HillTwist::RightRoll),
                    _ => None,
                };
                family.feel == HillFeel::Stall && twist.is_some() && family.twist == twist
            }
            _ => false,
        })
        .cloned()
        .collect();
    let selected = select_speed_matched_variant_for_train(
        &candidates,
        speed,
        &model.train,
        model.comfortable_force_limits,
    )
    .ok_or_else(|| format!("No available size for {line} at {speed:.2} m/s"))?;
    if !selected.fits {
        return Err(format!("No fitting size for {line} at {speed:.2} m/s"));
    }
    Ok(selected.id)
}

#[cfg(not(test))]
fn main() {
    let mut name = "Timber Tumbler".to_owned();
    let mut file = "web/e2e/player/circuits.txt".to_owned();
    let mut construction_test = false;
    let mut close = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--circuit" => name = args.next().expect("--circuit needs a name"),
            "--file" => file = args.next().expect("--file needs a path"),
            "--construction-test" => construction_test = true,
            "--close" => close = true,
            _ => panic!("Unknown argument {arg}"),
        }
    }
    let text = fs::read_to_string(&file).unwrap_or_else(|error| panic!("{file}: {error}"));
    match report_for_circuit(&text, &name, construction_test, close) {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
