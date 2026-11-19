use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;
use theme_park_sim::physics::{Circuit, GForces, PhysicsConfig, Train, preview_at_step};
use theme_park_sim::physics::force_evaluation::ForceEvaluator;
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::ride_file::{RideFile, RideStep};
use theme_park_sim::track::{CoasterModel, Heading, Piece, PieceId, PieceKind, TrackState, catalog, hybrid_model, multi_launch_model, wooden_twister_model};
use theme_park_sim::world::{Surface, World};
const TIMBER_TUMBLER: &str = include_str!("../../../content/rides/timber-tumbler.ride.json");
const VELOCITY_VIPER: &str = include_str!("../../../content/rides/velocity-viper.ride.json");
const BRIAR_BLITZ: &str = include_str!("../../../content/rides/briar-blitz.ride.json");
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

    let ride = ride_file(name);
    for step in &ride.steps {
        let piece_name = match step {
            RideStep::Chain(step) => {
                construction.set_chain(step.chain);
                continue;
            }
            RideStep::Place(step) => step.place.as_str(),
            RideStep::Close(step) => {
                assert!(step.close, "{name} has a false close step");
                continue;
            }
            _ => panic!("{name} benchmark does not support this ride step"),
        };
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

fn ride_file(name: &str) -> RideFile {
    let json = match name {
        "Timber Tumbler" => TIMBER_TUMBLER,
        "Velocity Viper" => VELOCITY_VIPER,
        "Briar Blitz" => BRIAR_BLITZ,
        _ => panic!("unknown benchmark ride {name}"),
    };
    serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid ride file for {name}: {error}"))
}

#[derive(Clone, Copy)]
struct Sample { force: [f64; 3], flags: [bool; 3], has_reading: bool, speed: f64 }
fn values(force: GForces) -> [f64; 3] { [force.vertical, force.lateral, force.longitudinal] }
fn record_interval(samples: &mut BTreeMap<usize, Sample>, old_distance: f64, old_force: [f64; 3], old_flags: [bool; 3], distance: f64, force: [f64; 3], flags: [bool; 3]) {
    if distance <= old_distance { return; }
    for mark in (old_distance.ceil() as usize)..=(distance.floor() as usize) {
        let fraction = ((mark as f64 - old_distance) / (distance - old_distance)).clamp(0.0, 1.0);
        let interpolated = std::array::from_fn(|axis| old_force[axis] + fraction * (force[axis] - old_force[axis]));
        samples.entry(mark).or_insert(Sample { force: interpolated, flags: std::array::from_fn(|axis| old_flags[axis] || flags[axis]), has_reading: false, speed: 0.0 });
    }
}
fn preview(pieces: &[Piece], model: &CoasterModel, step: f64) -> BTreeMap<usize, Sample> {
    let circuit = Circuit::new(pieces.to_vec()).unwrap();
    let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), pieces[0].length() / 2.0, 0.0);
    let mut evaluator = ForceEvaluator::new(model.comfortable_force_limits, step);
    let mut samples = BTreeMap::new();
    let mut previous: Option<(f64, [f64; 3], [bool; 3])> = None;
    let mut flags: BTreeMap<usize, [bool; 3]> = BTreeMap::new();
    let mut stationary = 0;
    let mut furthest = (0.0_f64, 0_usize);
    for _ in 0..(1000.0 / step) as usize {
        train.step_preview(&circuit, step);
        if train.distance() >= circuit.length() { break; }
        for frame in train.take_readings() {
            let reading = frame[0];
            if reading.distance > furthest.0 { furthest = (reading.distance, reading.piece_index); }
            let evaluated = evaluator.update(reading.g);
            let force = values(evaluated.filtered);
            let metre = reading.distance.floor() as usize;
            let entry = flags.entry(metre).or_insert([false; 3]);
            for axis in 0..3 { entry[axis] |= !evaluated.axis_within_limits[axis]; }
            let current_flags = evaluated.axis_within_limits.map(|within| !within);
            if let Some((old_distance, old_force, old_flags)) = previous {
                record_interval(&mut samples, old_distance, old_force, old_flags, reading.distance, force, current_flags);
            }
            previous = Some((reading.distance, force, current_flags));
        }
        if train.speed().abs() < 0.01 { stationary += 1; } else { stationary = 0; }
        if !train.notes().is_empty() || stationary >= (2.0 / step) as usize { println!("termination step={step} distance={:.3} speed={:.3} notes={:?} stationary_steps={stationary} furthest={:.3} piece={:?}", train.distance(), train.speed(), train.notes(), furthest.0, pieces[furthest.1].id); break; }
    }
    let empty = samples.keys().filter(|metre| !flags.contains_key(metre)).count();
    println!("step={step} sampled_metres_without_reading={empty}");
    for (metre, sample) in &mut samples { sample.has_reading = flags.contains_key(metre);  }
    samples
}
fn compare(pieces: &[Piece], model: &CoasterModel, step: f64) {
    let fast = preview(pieces, model, 0.005);
    let slow = preview(pieces, model, step);
    let mut maxima = [0.0_f64; 3];
    let mut differing = 0;
    let mut over = 0;
    let mut under = 0;
    let mut mixed = 0;
    let mut common = 0;
    let mut matched_flags = 0;
    let mut all_differing = 0;
    let mut all_over = 0;
    let mut all_under = 0;
    let mut all_mixed = 0;
    for (metre, fine) in &fast {
        let Some(coarse) = slow.get(metre) else { continue };
        common += 1;
        for axis in 0..3 { maxima[axis] = maxima[axis].max((fine.force[axis] - coarse.force[axis]).abs()); }
        if fine.flags.iter().any(|flag| *flag) != coarse.flags.iter().any(|flag| *flag) {
            let plus = coarse.flags.iter().any(|flag| *flag);
            let minus = fine.flags.iter().any(|flag| *flag);
            all_differing += 1;
            match (plus, minus) { (true, false) => all_over += 1, (false, true) => all_under += 1, (true, true) => all_mixed += 1, _ => unreachable!() }
            if fine.has_reading && coarse.has_reading {
                differing += 1;
                match (plus, minus) { (true, false) => over += 1, (false, true) => under += 1, (true, true) => mixed += 1, _ => unreachable!() }
            }
        }
        if fine.has_reading && coarse.has_reading { matched_flags += 1; }
    }
    println!("comparison step={step} common={common} matched_flag_metres={matched_flags} fine={} coarse={} max_g={maxima:?} differing_metres={differing} preview_only={over} test_only={under} mixed={mixed}", fast.len(), slow.len());
    println!("all_sampled_metres differing_metres={all_differing} preview_only={all_over} test_only={all_under} mixed={all_mixed}; flags OR the two readings bracketing each metre");
}
// The fine samples have 5 ms filter spacing. Their positions subdivide the
// coarse trajectory by distance; speed is linearly interpolated over it.
fn fine_geometry(pieces: &[Piece], model: &CoasterModel, coarse: bool) -> Vec<BTreeMap<usize, Sample>> {
    let circuit = Circuit::new(pieces.to_vec()).unwrap();
    let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), pieces[0].length() / 2.0, 0.0);
    let mut evaluators: Vec<_> = (0..model.train.cars).map(|_| ForceEvaluator::new(model.comfortable_force_limits, 0.005)).collect();
    let mut samples = vec![BTreeMap::new(); model.train.cars as usize];
    let mut previous: Vec<Option<(f64, [f64; 3], [bool; 3], f64)>> = vec![None; model.train.cars as usize];
    let step = if coarse { 0.05 } else { 0.005 };
    let mut stationary = 0;
    for _ in 0..(1000.0 / step) as usize {
        let old_distance = train.distance();
        let old_speed = train.speed();
        if coarse { train.step_preview(&circuit, step); } else { train.step(&circuit); }
        let readings = train.take_readings();
        if train.distance() >= circuit.length() { break; }
        if !train.notes().is_empty() { break; }
        let new_distance = train.distance();
        let new_speed = train.speed();
        let count = if coarse { 10 } else { 1 };
        for sample_index in 1..=count {
            let fraction = sample_index as f64 / count as f64;
            let distance = old_distance + fraction * (new_distance - old_distance);
            let speed = old_speed + fraction * (new_speed - old_speed);
            let acceleration = (new_speed - old_speed) / step;
            let forces: Vec<GForces> = if coarse { (0..model.train.cars as usize).map(|car| train.measurement_force(&circuit, distance, speed, acceleration, car)).collect() }
                else { readings[0].iter().map(|reading| reading.g).collect() };
            for (car, (force, evaluator)) in forces.into_iter().zip(&mut evaluators).enumerate() {
                let evaluated = evaluator.update(force);
                let distance = distance - car as f64 * model.train.length_metres / f64::from(model.train.cars);
                if distance < 0.0 { continue; }
                let force = values(evaluated.filtered);
                let flags = evaluated.axis_within_limits.map(|within| !within);
                if let Some((old_distance, old_force, old_flags, old_speed)) = previous[car] {
                    if distance > old_distance {
                        for mark in (old_distance.ceil() as usize)..=(distance.floor() as usize) {
                            let fraction = (mark as f64 - old_distance) / (distance - old_distance);
                            samples[car].entry(mark).or_insert(Sample {
                                force: std::array::from_fn(|axis| old_force[axis] + fraction * (force[axis] - old_force[axis])),
                                flags: std::array::from_fn(|axis| old_flags[axis] || flags[axis]),
                                has_reading: true,
                                speed: old_speed + fraction * (speed - old_speed),
                            });
                        }
                    }
                }
                previous[car] = Some((distance, force, flags, speed));
            }
        }
        if train.speed().abs() < 0.01 { stationary += 1; } else { stationary = 0; }
        if !train.notes().is_empty() || stationary >= (2.0 / step) as usize { break; }
    }
    samples
}

fn compare_geometry(pieces: &[Piece], model: &CoasterModel) {
    let reference = fine_geometry(pieces, model, false);
    let prototype = fine_geometry(pieces, model, true);
    let mut maximum_force = [0.0_f64; 3];
    let mut maximum_speed = 0.0_f64;
    let mut maximum_force_details = [(0_usize, 0_usize, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64); 3];
    let mut maximum_speed_details = (0_usize, 0_usize, 0.0_f64, 0.0_f64);
    let mut disagreements = BTreeMap::<usize, (bool, bool)>::new();
    let mut common = 0;
    for (car, (reference_car, prototype_car)) in reference.iter().zip(&prototype).enumerate() {
        for (metre, fine) in reference_car {
            let Some(coarse) = prototype_car.get(metre) else { continue };
            common += 1;
            let speed_error = (fine.speed - coarse.speed).abs();
            if speed_error > maximum_speed {
                maximum_speed = speed_error;
                maximum_speed_details = (car, *metre, fine.speed, coarse.speed);
            }
            for axis in 0..3 {
                let error = (fine.force[axis] - coarse.force[axis]).abs();
                if error > maximum_force[axis] {
                    maximum_force[axis] = error;
                    maximum_force_details[axis] = (car, *metre, fine.force[axis], coarse.force[axis], fine.speed, coarse.speed);
                }
            }
            let entry = disagreements.entry(*metre).or_default();
            entry.0 |= fine.flags.iter().any(|flag| *flag);
            entry.1 |= coarse.flags.iter().any(|flag| *flag);
        }
    }
    let over = disagreements.values().filter(|(fine, coarse)| !fine && *coarse).count();
    let under = disagreements.values().filter(|(fine, coarse)| *fine && !coarse).count();
    println!("fine_geometry cars={} common_car_metres={common} max_g={maximum_force:?} max_speed_error_mps={maximum_speed:.6} flag_disagreements_over={over} under={under}", reference.len());
    println!("max_force_details axis=[vertical,lateral,longitudinal] (car,metre,reference_g,prototype_g,reference_speed,prototype_speed)={maximum_force_details:?}");
    println!("max_speed_details (car,metre,reference,prototype)={maximum_speed_details:?}");
    println!("flag_disagreements (metre,reference,prototype)={:?}", disagreements.iter().filter(|(_, (fine, coarse))| fine != coarse).collect::<Vec<_>>());
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
    let coarse_comparison = std::env::args().any(|argument| argument == "--coarse-comparison");
    let flags_only = std::env::args().any(|argument| argument == "--flags-only");
    let comparison_only = std::env::args().any(|argument| argument == "--comparison-only");
    assert!(!(flags_only && comparison_only));
    for (name, model) in [("Timber Tumbler", hybrid_model()), ("Velocity Viper", multi_launch_model()), ("Briar Blitz", wooden_twister_model())] {
        let pieces = build_player_circuit(name, model.clone());
        println!("{name}");
        if !flags_only {
            if coarse_comparison { compare(&pieces, &model, 0.05); compare(&pieces, &model, 0.025); }
            else { compare_geometry(&pieces, &model); }
        }
        if !comparison_only { test_run(&pieces, &model); }
        if !flags_only && !comparison_only {
            timing(&pieces, &model, false);
            timing(&pieces, &model, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_force_matches_physics_for_every_car() {
        let model = multi_launch_model();
        let pieces = build_player_circuit("Velocity Viper", model.clone());
        let circuit = Circuit::new(pieces.clone()).unwrap();
        let mut train = Train::new(&circuit, &model.train, PhysicsConfig::default(), pieces[0].length() / 2.0, 0.0);
        for _ in 0..10 {
            let previous_speed = train.speed();
            train.step(&circuit);
            let readings = train.take_readings();
            let acceleration = (train.speed() - previous_speed) / 0.005;
            for (car, reading) in readings[0].iter().enumerate() {
                assert_eq!(train.measurement_force(&circuit, train.distance(), train.speed(), acceleration, car), reading.g);
            }
        }
    }

    #[test]
    fn interpolation_covers_skipped_metres_and_preserves_first_passage() {
        let mut samples = BTreeMap::new();
        record_interval(&mut samples, 0.5, [0.0; 3], [false; 3], 3.5, [3.0; 3], [true, false, false]);
        assert_eq!(samples.keys().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(samples[&2].force, [1.5; 3]);
        assert_eq!(samples[&2].flags, [true, false, false]);
        record_interval(&mut samples, 0.5, [100.0; 3], [false; 3], 3.5, [103.0; 3], [false; 3]);
        assert_eq!(samples[&2].force, [1.5; 3]);
        assert_eq!(samples[&2].flags, [true, false, false]);
        record_interval(&mut samples, 3.5, [3.0; 3], [false; 3], 0.5, [0.0; 3], [false; 3]);
        assert_eq!(samples[&2].force, [1.5; 3]);
        assert_eq!(samples.len(), 3);
    }
}
