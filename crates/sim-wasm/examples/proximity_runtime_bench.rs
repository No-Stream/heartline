include!("../src/lib.rs");

mod proximity_benchmark_tests {
    use std::hint::black_box;
    use std::time::Instant;

    use crate::WasmSimulation;
    use theme_park_sim::track::proximity::scan_proximity_with_supports;

    pub fn completed_coaster_proximity_cost() {
        let mut simulation = WasmSimulation::new(42, 0);
        simulation.construction_start(100, 100, 40, 0);
        for piece in [
            "LiftLevelToGentleUp",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "Lift",
            "LiftGentleUpToLevel",
            "LiftFlat",
            "Straight",
            "Straight",
            "TurnLargeEntry",
            "TurnLargeExit",
            "LevelToGentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDown",
            "GentleDownToLevel",
            "Loop",
            "TurnLargeEntry",
            "TurnLargeExit",
        ] {
            simulation.construction_select(piece);
            simulation.construction_append();
            assert!(
                simulation.construction_message().is_empty(),
                "{piece}: {}",
                simulation.construction_message()
            );
        }
        assert!(simulation.construction_can_close());
        simulation.construction_close();
        simulation.construction_test();
        let mut ride_ticks = 0;
        for _ in 0..9000 {
            simulation.tick();
            ride_ticks += 1;
            if simulation.construction_report.is_some() {
                break;
            }
        }
        let report = simulation
            .construction_report
            .clone()
            .expect("completed report");
        assert!(report.completed);
        simulation.support_render_cache.clear();
        for label in ["cold supports", "cached supports"] {
            let start = Instant::now();
            black_box(simulation.construction_placed_supports());
            let obstacles = simulation.proximity_obstacles();
            let supports = simulation.proximity_supports();
            let support_ms = start.elapsed().as_secs_f64() * 1000.0;
            let construction = simulation.construction.as_ref().unwrap();
            let start = Instant::now();
            let geometry = scan_proximity_with_supports(
                &simulation.world,
                construction.pieces(),
                &construction.model().train,
                simulation.circuit.pieces(),
                &obstacles,
                &supports,
            );
            let scan_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let mut event_count = 0;
            for _ in 0..construction.model().train.cars {
                let mut car_report = report.clone();
                car_report.proximity_events.clear();
                let events = geometry.events_for_run(&car_report.samples);
                event_count += events.len();
                car_report.add_proximity_events(&events);
                black_box(car_report);
            }
            let report_ms = start.elapsed().as_secs_f64() * 1000.0;
            println!(
                "{label}: {:.3} ms supports and obstacle collection, {:.3} ms scan, {:.3} ms for {} reports and event application; {} pieces, {} other pieces, {} obstacles, {} support segments, {} samples, {} events",
                support_ms,
                scan_ms,
                report_ms,
                construction.model().train.cars,
                construction.pieces().len(),
                simulation.circuit.pieces().len(),
                obstacles.len(),
                supports.len(),
                report.samples.len(),
                event_count
            );
            println!(
                "{label}: {:.3} ms total completion work, {:.3} ms/30 Hz tick amortized over {ride_ticks} ticks",
                support_ms + scan_ms + report_ms,
                (support_ms + scan_ms + report_ms) / f64::from(ride_ticks)
            );
        }
    }
}

fn main() {
    proximity_benchmark_tests::completed_coaster_proximity_cost();
}
