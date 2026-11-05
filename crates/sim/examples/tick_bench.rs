use std::env;
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use theme_park_sim::demo_world::demo_world;
use theme_park_sim::executor::{Executor, ThreadedExecutor};
use theme_park_sim::guests::behavior::{Facility, KnownPlace, PlaceKind};
use theme_park_sim::guests::park::ParkGuests;
use theme_park_sim::guests::walking::WalkingGuest;
use theme_park_sim::world::Position;

const DEFAULT_GUEST_COUNT: usize = 10_000;
const WARMUP_TICKS: usize = 20;
const DEFAULT_MEASURED_TICKS: usize = 200;
const PARTITION_SIZE: usize = 256;
const TARGET_WORK_NANOS: f64 = 1_000.0;
const CALIBRATION_GUEST_COUNT: usize = 25_000;
const PILOT_ITERATIONS: usize = 64;
const USAGE: &str = "Usage: tick_bench [--work none|placeholder|1us|both] [--agents N] [--ticks N]";

#[derive(Clone, Copy)]
enum WorkSelection {
    None,
    OneMicrosecond,
    Both,
}

struct Options {
    guest_count: usize,
    measured_ticks: usize,
    work_selection: WorkSelection,
}

struct WorkExecutor<E> {
    inner: E,
    iterations_per_guest: usize,
}

impl<E> WorkExecutor<E> {
    fn new(inner: E, iterations_per_guest: usize) -> Self {
        Self {
            inner,
            iterations_per_guest,
        }
    }
}

impl<E: Executor<WalkingGuest> + Sync> Executor<WalkingGuest> for WorkExecutor<E> {
    fn execute<F>(&self, guest_count: usize, process: F) -> Vec<WalkingGuest>
    where
        F: Fn(usize) -> WalkingGuest + Sync,
    {
        self.inner.execute(guest_count, |index| {
            let guest = process(index);
            if self.iterations_per_guest > 0 {
                black_box(run_guest_work(index, self.iterations_per_guest));
            }
            guest
        })
    }
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Guests: {}; measured ticks: {}; warmup ticks: {}",
        options.guest_count, options.measured_ticks, WARMUP_TICKS
    );
    println!(
        "{:<14} {:>8} {:>16} {:>20}",
        "Work", "Threads", "Ticks/second", "Speedup vs 1 thread"
    );

    let one_microsecond_iterations = match options.work_selection {
        WorkSelection::OneMicrosecond | WorkSelection::Both => {
            Some(calibrate_one_microsecond_work())
        }
        WorkSelection::None => None,
    };
    match options.work_selection {
        WorkSelection::None => benchmark_workload(0, "guests", &options),
        WorkSelection::OneMicrosecond => benchmark_workload(
            one_microsecond_iterations.expect("workload was calibrated"),
            "guests + 1us",
            &options,
        ),
        WorkSelection::Both => {
            benchmark_workload(0, "guests", &options);
            benchmark_workload(
                one_microsecond_iterations.expect("workload was calibrated"),
                "guests + 1us",
                &options,
            );
        }
    }

    ExitCode::SUCCESS
}

fn parse_options() -> Result<Option<Options>, String> {
    let mut options = Options {
        guest_count: DEFAULT_GUEST_COUNT,
        measured_ticks: DEFAULT_MEASURED_TICKS,
        work_selection: WorkSelection::Both,
    };
    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--work" => {
                options.work_selection = match arguments.next().as_deref() {
                    Some("none" | "placeholder") => WorkSelection::None,
                    Some("1us") => WorkSelection::OneMicrosecond,
                    Some("both") => WorkSelection::Both,
                    Some(value) => return Err(format!("unsupported work level: {value}")),
                    None => return Err("--work requires a value".to_owned()),
                };
            }
            "--agents" => options.guest_count = parse_positive_usize("--agents", arguments.next())?,
            "--ticks" => {
                options.measured_ticks = parse_positive_usize("--ticks", arguments.next())?
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }

    Ok(Some(options))
}

fn parse_positive_usize(option: &str, value: Option<String>) -> Result<usize, String> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{option} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{option} must be a positive integer"));
    }
    Ok(parsed)
}

#[inline(never)]
fn run_guest_work(guest_index: usize, iterations: usize) -> u64 {
    let mut value = (guest_index as u64) ^ 0x9e3779b97f4a7c15;
    for _ in 0..iterations {
        value = value
            .wrapping_add(0x9e3779b97f4a7c15)
            .rotate_left(17)
            .wrapping_mul(0xbf58476d1ce4e5b9);
    }
    value
}

fn calibrate_one_microsecond_work() -> usize {
    let pilot_nanos_per_guest = measure_guest_work(PILOT_ITERATIONS);
    let estimated_iterations =
        ((TARGET_WORK_NANOS * PILOT_ITERATIONS as f64 / pilot_nanos_per_guest).round() as usize)
            .max(1);
    let first_measurement = measure_guest_work(estimated_iterations);
    ((estimated_iterations as f64 * TARGET_WORK_NANOS / first_measurement).round() as usize).max(1)
}

fn measure_guest_work(iterations_per_guest: usize) -> f64 {
    let start = Instant::now();
    for guest_index in 0..CALIBRATION_GUEST_COUNT {
        black_box(run_guest_work(guest_index, iterations_per_guest));
    }
    start.elapsed().as_nanos() as f64 / CALIBRATION_GUEST_COUNT as f64
}

fn benchmark_workload(iterations_per_guest: usize, work_name: &str, options: &Options) {
    let results = [1, 2, 4, 8]
        .map(|thread_count| measure_ticks_per_second(thread_count, iterations_per_guest, options));
    let baseline_ticks_per_second = results[0].1;
    for (thread_count, ticks_per_second) in results {
        println!(
            "{work_name:<14} {thread_count:>8} {ticks_per_second:>16.2} {:>19.2}x",
            ticks_per_second / baseline_ticks_per_second
        );
    }
}

fn measure_ticks_per_second(
    thread_count: usize,
    iterations_per_guest: usize,
    options: &Options,
) -> (usize, f64) {
    let world = demo_world();
    let entrance = Position {
        x: 218,
        y: 256,
        height: 8,
    };
    let facilities: Vec<_> = [
        PlaceKind::Ride,
        PlaceKind::Food,
        PlaceKind::Drink,
        PlaceKind::Restroom,
        PlaceKind::Bench,
    ]
    .into_iter()
    .enumerate()
    .map(|(id, kind)| Facility {
        exit_position: None,
        place: KnownPlace {
            id: id as u32,
            position: Position {
                x: 219 + id as u16,
                ..entrance
            },
            kind,
            minimum_height_cm: 0,
            intensity: 20,
            nausea: if kind == PlaceKind::Ride { 10 } else { 0 },
            ride_type: 0,
            theme: 0,
            price: 0,
        },
        capacity: 24,
        duration_ticks: 900,
    })
    .collect();
    let executor = WorkExecutor::new(
        ThreadedExecutor::new(thread_count, PARTITION_SIZE),
        iterations_per_guest,
    );
    let mut simulation = ParkGuests::new(
        42,
        options.guest_count,
        &world,
        entrance,
        options.guest_count,
    );

    for _ in 0..WARMUP_TICKS {
        simulation.tick_with(&world, &facilities, 100, None, &executor);
    }

    let start = Instant::now();
    for _ in 0..options.measured_ticks {
        simulation.tick_with(&world, &facilities, 100, None, &executor);
    }
    let elapsed = start.elapsed();
    black_box(simulation.walking().state_hash());
    (
        thread_count,
        options.measured_ticks as f64 / elapsed.as_secs_f64(),
    )
}
