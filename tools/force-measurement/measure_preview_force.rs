use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;
use theme_park_sim::physics::{Circuit, GForces, PhysicsConfig, Train, preview_at_step};
use theme_park_sim::physics::force_evaluation::ForceEvaluator;
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{CoasterModel, Heading, Piece, PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model, wooden_twister_model};
use theme_park_sim::world::{Surface, World};
const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");
fn build_player_circuit(name: &str, model: CoasterModel) -> Vec<Piece> {
    let world = World::new(
        512,
        512,
        Surface {
            corners: [8; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 482,
        y: 498,
        z: 6,
        heading: Heading::North,
        ..TrackState::origin()
    };
    let mut construction = Construction::new(start, model);
    construction.append(PieceId::Station, &world, &[]).unwrap();

    let header = format!("[{name}]");
    let mut sections = CIRCUITS.split(&header);
    sections.next().unwrap();
    let section = sections
        .next()
        .unwrap_or_else(|| panic!("{name} circuit missing"));
    assert!(sections.next().is_none(), "duplicate {name} circuit");
    for piece_name in section
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        match piece_name {
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
        let piece = catalog()
            .iter()
            .find(|piece| format!("{:?}", piece.id) == piece_name)
            .unwrap_or_else(|| panic!("unknown piece {piece_name}"));
        construction
            .append(piece.id, &world, &[])
            .unwrap_or_else(|error| panic!("{piece_name}: {error:?}"));
    }
    construction.close().unwrap();
    construction.pieces().to_vec()
}

#[derive(Clone, Copy)]
struct Sample { force: [f64; 3], flags: [bool; 3], has_reading: bool }
fn values(force: GForces) -> [f64; 3] { [force.vertical, force.lateral, force.longitudinal] }
fn preview(pieces: &[Piece], model: &CoasterModel, step: f64) -> BTreeMap<usize, Sample> {
    let circuit = Circuit::new(pieces.to_vec()).unwrap();
    let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), pieces[0].length() / 2.0, 0.0);
    let mut evaluator = ForceEvaluator::new(model.comfortable_force_limits, step);
    let mut samples = BTreeMap::new();
    let mut previous: Option<(f64, [f64; 3])> = None;
    let mut flags: BTreeMap<usize, [bool; 3]> = BTreeMap::new();
    let mut stationary = 0;
    for _ in 0..(1000.0 / step) as usize {
        train.step_preview(&circuit, step);
        if train.distance() >= circuit.length() { break; }
        for frame in train.take_readings() {
            let reading = frame[0];
            let evaluated = evaluator.update(reading.g);
            let force = values(evaluated.filtered);
            let metre = reading.distance.floor() as usize;
            let entry = flags.entry(metre).or_insert([false; 3]);
            for axis in 0..3 { entry[axis] |= !evaluated.axis_within_limits[axis]; }
            if let Some((old_distance, old_force)) = previous {
                if reading.distance > old_distance {
                    for mark in (old_distance.ceil() as usize)..=(reading.distance.floor() as usize) {
                        let fraction = ((mark as f64 - old_distance) / (reading.distance - old_distance)).clamp(0.0, 1.0);
                        let mut interpolated = [0.0; 3];
                        for axis in 0..3 { interpolated[axis] = old_force[axis] + fraction * (force[axis] - old_force[axis]); }
                        samples.insert(mark, Sample { force: interpolated, flags: [false; 3], has_reading: false });
                    }
                }
            }
            previous = Some((reading.distance, force));
        }
        if train.speed().abs() < 0.01 { stationary += 1; } else { stationary = 0; }
        if !train.notes().is_empty() || stationary >= (2.0 / step) as usize { break; }
    }
    let empty = samples.keys().filter(|metre| !flags.contains_key(metre)).count();
    println!("step={step} sampled_metres_without_reading={empty}");
    for (metre, sample) in &mut samples { sample.has_reading = flags.contains_key(metre); sample.flags = flags.get(metre).copied().unwrap_or([false; 3]); }
    samples
}
fn compare(pieces: &[Piece], model: &CoasterModel) {
    let fast = preview(pieces, model, 0.005);
    let slow = preview(pieces, model, 0.05);
    let mut maxima = [0.0_f64; 3];
    let mut differing = 0;
    let mut over = 0;
    let mut under = 0;
    let mut mixed = 0;
    let mut common = 0;
    let mut matched_flags = 0;
    for (metre, fine) in &fast {
        let Some(coarse) = slow.get(metre) else { continue };
        common += 1;
        for axis in 0..3 { maxima[axis] = maxima[axis].max((fine.force[axis] - coarse.force[axis]).abs()); }
        if !(fine.has_reading && coarse.has_reading) { continue; }
        matched_flags += 1;
        if fine.flags != coarse.flags {
            differing += 1;
            let plus = (0..3).any(|axis| coarse.flags[axis] && !fine.flags[axis]);
            let minus = (0..3).any(|axis| fine.flags[axis] && !coarse.flags[axis]);
            match (plus, minus) { (true, false) => over += 1, (false, true) => under += 1, (true, true) => mixed += 1, _ => unreachable!() }
        }
    }
    println!("comparison common={common} matched_flag_metres={matched_flags} fine={} coarse={} max_g={maxima:?} differing_metres={differing} preview_only={over} test_only={under} mixed={mixed}", fast.len(), slow.len());
}
fn test_run(pieces: &[Piece], model: &CoasterModel) {
    let circuit = Circuit::new(pieces.to_vec()).unwrap();
    let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), pieces[0].length() / 2.0, 0.0);
    let mut evaluators: Vec<_> = (0..model.train.cars).map(|_| ForceEvaluator::new(model.comfortable_force_limits, 0.005)).collect();
    let limits = model.comfortable_force_limits;
    let mut raw = BTreeSet::new();
    let mut evaluated = BTreeSet::new();
    let mut raw_active = BTreeSet::new();
    let mut evaluated_active = BTreeSet::new();
    let mut stationary = 0;
    let mut complete = false;
    let mut furthest = 0.0_f64;
    for _ in 0..200_000 {
        train.step(&circuit);
        furthest = furthest.max(train.distance());
        for frame in train.take_readings() {
            for (reading, evaluator) in frame.iter().zip(&mut evaluators) {
                let g = reading.g;
                let raw_exceeded = g.vertical < limits.minimum_vertical_g || g.vertical > limits.maximum_vertical_g || g.lateral.abs() > limits.maximum_lateral_g || g.longitudinal.abs() > limits.maximum_longitudinal_g;
                let evaluated_exceeded = !evaluator.update(g).within_limits;
                let active = reading.speed > 1.0 && matches!(pieces[reading.piece_index].kind, PieceKind::Track | PieceKind::Launch | PieceKind::Brakes);
                if raw_exceeded { raw.insert(reading.piece_index); if active { raw_active.insert(reading.piece_index); } }
                if evaluated_exceeded { evaluated.insert(reading.piece_index); if active { evaluated_active.insert(reading.piece_index); } }
            }
        }
        if train.speed().abs() < 0.01 { stationary += 1; } else { stationary = 0; }
        if stationary >= 400 || !train.notes().is_empty() { break; }
        if train.distance() >= circuit.length() + pieces[0].length() / 2.0 { complete = true; break; }
    }
    let describe = |indices: Vec<usize>| -> Vec<String> { indices.into_iter().map(|index| format!("{}:{:?}", index, pieces[index].id)).collect() };
    println!("active_only raw={} evaluated={} new={:?} removed={:?}", raw_active.len(), evaluated_active.len(), describe(evaluated_active.difference(&raw_active).copied().collect()), describe(raw_active.difference(&evaluated_active).copied().collect()));
    println!("test_run completed={complete} final_distance={:.2} furthest={furthest:.2} raw={} evaluated={} new={:?} removed={:?}",train.distance(),raw.len(),evaluated.len(),describe(evaluated.difference(&raw).copied().collect()),describe(raw.difference(&evaluated).copied().collect()));
}
fn timing(pieces: &[Piece], model: &CoasterModel, evaluated: bool) {
    let mut times = Vec::new();
    let limits = evaluated.then_some(model.comfortable_force_limits);
    for iteration in 0..105 {
        let start = Instant::now();
        let result = preview_at_step(pieces, &model.train, PhysicsConfig::default(), 20_000, 0.05, limits).unwrap();
        black_box(result);
        if iteration >= 5 { times.push(start.elapsed().as_secs_f64() * 1000.0); }
    }
    times.sort_by(f64::total_cmp);
    println!("timing evaluated={evaluated} median_ms={:.3} p95_ms={:.3} max_ms={:.3}", (times[49]+times[50])/2.0, times[94],times[99]);
}
fn main() {
    let flags_only = std::env::args().any(|argument| argument == "--flags-only");
    for (name, model) in [("Timber Tumbler", hybrid_model()), ("Velocity Viper", multi_launch_model()), ("Briar Blitz", wooden_twister_model())] {
        let pieces = build_player_circuit(name, model.clone());
        println!("{name}");
        if !flags_only { compare(&pieces, &model); }
        test_run(&pieces, &model);
        if !flags_only {
            timing(&pieces, &model, false);
            timing(&pieces, &model, true);
        }
    }
}
