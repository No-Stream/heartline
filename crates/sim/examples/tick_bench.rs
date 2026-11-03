use std::env;
use std::hint::black_box;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use theme_park_sim::{Agent, Executor, Simulation, ThreadedExecutor};

const DEFAULT_AGENT_COUNT: usize = 10_000;
const WARMUP_TICKS: usize = 20;
const DEFAULT_MEASURED_TICKS: usize = 200;
const PARTITION_SIZE: usize = 256;
const THREAD_COUNTS: [usize; 4] = [1, 2, 4, 8];
const TARGET_WORK_NANOS: f64 = 1_000.0;
const CALIBRATION_AGENT_COUNT: usize = 25_000;
const PILOT_ITERATIONS: usize = 64;

const USAGE: &str = "Usage: tick_bench [--work placeholder|1us|both] [--agents N] [--ticks N]";

#[derive(Clone, Copy)]
enum WorkSelection {
    Placeholder,
    OneMicrosecond,
    Both,
}

struct Options {
    agent_count: usize,
    measured_ticks: usize,
    work_selection: WorkSelection,
}

#[derive(Clone, Copy)]
enum Workload {
    Placeholder,
    OneMicrosecond {
        iterations_per_agent: usize,
        measured_nanos_per_agent: f64,
    },
}

struct BenchmarkResult {
    thread_count: usize,
    ticks_per_second: f64,
}

struct WorkExecutor<E> {
    inner: E,
    iterations_per_agent: Option<usize>,
    execution_count: AtomicUsize,
}

impl<E> WorkExecutor<E> {
    fn new(inner: E, iterations_per_agent: Option<usize>) -> Self {
        Self {
            inner,
            iterations_per_agent,
            execution_count: AtomicUsize::new(0),
        }
    }
}

impl<E: Executor> Executor for WorkExecutor<E> {
    fn execute<F>(&self, agent_count: usize, process: F) -> Vec<Agent>
    where
        F: Fn(usize) -> Agent + Sync,
    {
        let Some(iterations_per_agent) = self.iterations_per_agent else {
            return self.inner.execute(agent_count, process);
        };

        // Simulation currently calls the executor twice per tick, so charge one phase only.
        let apply_work = self.execution_count.fetch_add(1, Ordering::Relaxed) % 2 == 0;
        self.inner.execute(agent_count, |index| {
            let agent = process(index);
            if apply_work {
                black_box(run_agent_work(index, iterations_per_agent));
            }
            agent
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

    let microsecond_workload = match options.work_selection {
        WorkSelection::Placeholder => None,
        WorkSelection::OneMicrosecond | WorkSelection::Both => {
            let workload = calibrate_one_microsecond_workload();
            if let Workload::OneMicrosecond {
                iterations_per_agent,
                measured_nanos_per_agent,
            } = workload
            {
                println!(
                    "1us work calibration: {iterations_per_agent} integer-mix iterations per agent"
                );
                println!("Observed {measured_nanos_per_agent:.1} ns per agent.");
            }
            Some(workload)
        }
    };

    println!(
        "Agents: {}; measured ticks: {}; warmup ticks: {}",
        options.agent_count, options.measured_ticks, WARMUP_TICKS
    );
    println!("Work is added once per agent per tick in the first simulation phase.");
    println!(
        "{:<14} {:>8} {:>16} {:>20}",
        "Work", "Threads", "Ticks/second", "Speedup vs 1 thread"
    );

    match options.work_selection {
        WorkSelection::Placeholder => {
            benchmark_workload(Workload::Placeholder, &options);
        }
        WorkSelection::OneMicrosecond => {
            benchmark_workload(
                microsecond_workload.expect("workload was calibrated"),
                &options,
            );
        }
        WorkSelection::Both => {
            benchmark_workload(Workload::Placeholder, &options);
            benchmark_workload(
                microsecond_workload.expect("workload was calibrated"),
                &options,
            );
        }
    }

    ExitCode::SUCCESS
}

fn parse_options() -> Result<Option<Options>, String> {
    let mut options = Options {
        agent_count: DEFAULT_AGENT_COUNT,
        measured_ticks: DEFAULT_MEASURED_TICKS,
        work_selection: WorkSelection::Both,
    };
    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--work" => {
                options.work_selection = match arguments.next().as_deref() {
                    Some("placeholder") => WorkSelection::Placeholder,
                    Some("1us") => WorkSelection::OneMicrosecond,
                    Some("both") => WorkSelection::Both,
                    Some(value) => return Err(format!("unsupported work level: {value}")),
                    None => return Err("--work requires a value".to_owned()),
                };
            }
            "--agents" => {
                options.agent_count = parse_positive_usize("--agents", arguments.next())?;
            }
            "--ticks" => {
                options.measured_ticks = parse_positive_usize("--ticks", arguments.next())?;
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

fn calibrate_one_microsecond_workload() -> Workload {
    let pilot_nanos_per_agent = measure_agent_work(PILOT_ITERATIONS);
    let estimated_iterations =
        ((TARGET_WORK_NANOS * PILOT_ITERATIONS as f64 / pilot_nanos_per_agent).round() as usize)
            .max(1);
    let first_measurement = measure_agent_work(estimated_iterations);
    let iterations_per_agent =
        ((estimated_iterations as f64 * TARGET_WORK_NANOS / first_measurement).round() as usize)
            .max(1);
    let measured_nanos_per_agent = measure_agent_work(iterations_per_agent);

    Workload::OneMicrosecond {
        iterations_per_agent,
        measured_nanos_per_agent,
    }
}

fn measure_agent_work(iterations_per_agent: usize) -> f64 {
    let start = Instant::now();
    for index in 0..CALIBRATION_AGENT_COUNT {
        black_box(run_agent_work(index, iterations_per_agent));
    }
    start.elapsed().as_nanos() as f64 / CALIBRATION_AGENT_COUNT as f64
}

#[inline(never)]
fn run_agent_work(agent_index: usize, iterations: usize) -> u64 {
    let mut value = (agent_index as u64) ^ 0x9e3779b97f4a7c15;
    for _ in 0..iterations {
        value = value
            .wrapping_add(0x9e3779b97f4a7c15)
            .rotate_left(17)
            .wrapping_mul(0xbf58476d1ce4e5b9);
    }
    value
}

fn benchmark_workload(workload: Workload, options: &Options) {
    let iterations_per_agent = match workload {
        Workload::Placeholder => None,
        Workload::OneMicrosecond {
            iterations_per_agent,
            ..
        } => Some(iterations_per_agent),
    };
    let results = THREAD_COUNTS
        .map(|thread_count| measure_ticks_per_second(thread_count, iterations_per_agent, options));
    let baseline_ticks_per_second = results[0].ticks_per_second;

    for result in results {
        let speedup = result.ticks_per_second / baseline_ticks_per_second;
        println!(
            "{:<14} {:>8} {:>16.2} {:>19.2}x",
            workload.name(),
            result.thread_count,
            result.ticks_per_second,
            speedup
        );
    }
}

fn measure_ticks_per_second(
    thread_count: usize,
    iterations_per_agent: Option<usize>,
    options: &Options,
) -> BenchmarkResult {
    let executor = WorkExecutor::new(
        ThreadedExecutor::new(thread_count, PARTITION_SIZE),
        iterations_per_agent,
    );
    let mut simulation = Simulation::new(42, options.agent_count);

    for _ in 0..WARMUP_TICKS {
        simulation.tick_with(&executor);
    }

    let start = Instant::now();
    for _ in 0..options.measured_ticks {
        simulation.tick_with(&executor);
    }
    let elapsed = start.elapsed();
    black_box(simulation.state_hash());

    BenchmarkResult {
        thread_count,
        ticks_per_second: options.measured_ticks as f64 / elapsed.as_secs_f64(),
    }
}

impl Workload {
    fn name(self) -> &'static str {
        match self {
            Workload::Placeholder => "placeholder",
            Workload::OneMicrosecond { .. } => "1us",
        }
    }
}
