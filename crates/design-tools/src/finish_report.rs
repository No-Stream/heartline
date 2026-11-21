//! Text and JSON for `ride <file> finish`, with a full closed-lap check of each completion.

use std::fmt::Write as _;

use serde_json::{Value, json};
use theme_park_sim::physics::Circuit;
use theme_park_sim::track::construction::Completion;
use theme_park_sim::track::{CoasterModel, Piece};

use crate::report::{
    construction_physics_test_run, default_physics_test_run, force_headroom_results,
    rated_speed_margin_failures, rated_speed_margins,
};

/// The result of closing the circuit with a completion and running the rated-margin and
/// force-headroom rules of `tools/design check` on the whole lap under both physics.
pub(crate) struct CompletionCheck {
    pub(crate) failures: Vec<String>,
}

impl CompletionCheck {
    pub(crate) fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

pub(crate) fn check_completion(
    placed_pieces: &[Piece],
    completion: &Completion,
    model: &CoasterModel,
) -> Result<CompletionCheck, String> {
    let circuit = Circuit::new([placed_pieces, &completion.geometry].concat())
        .map_err(|error| format!("finish option does not close the circuit: {error:?}"))?;
    let mut failures = rated_speed_margin_failures(&rated_speed_margins(&circuit, model)?);
    let limits = model.design_force_targets;
    failures.extend(
        [
            force_headroom_results(
                &default_physics_test_run(&circuit, model),
                limits,
                "default physics",
            ),
            force_headroom_results(
                &construction_physics_test_run(&circuit, model),
                limits,
                "construction-test physics",
            ),
        ]
        .concat()
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.failure()),
    );
    Ok(CompletionCheck { failures })
}

pub(crate) fn render_finish_options(
    completions: &[Completion],
    checks: &[CompletionCheck],
) -> String {
    let mut text = format!(
        "Found {} way(s) to finish the circuit, best first (construction-test physics; 'finish apply <n>' places one):\n",
        completions.len()
    );
    for (completion, check) in completions.iter().zip(checks) {
        let physics = &completion.physics;
        let pieces: Vec<_> = completion
            .geometry
            .iter()
            .map(|piece| format!("{:?}", piece.id))
            .collect();
        writeln!(
            text,
            "{}. {} ({} pieces): arrives at {:.1} m/s; brakes remove {:.0} kJ; force headroom {:+.2} g; rated margin {}; ride check {}",
            completion.score,
            pieces.join(", "),
            pieces.len(),
            physics.arrival_speed_mps,
            physics.brake_energy_kj,
            physics.minimum_force_headroom_g,
            physics
                .minimum_rated_margin_mps
                .map_or_else(|| "none rated".to_owned(), |margin| format!("{margin:+.1} m/s")),
            if check.passed() { "pass" } else { "FAIL" },
        )
        .unwrap();
        for piece in &physics.pieces {
            let peaks = piece.worst_car_peaks;
            writeln!(
                text,
                "   {:?}: {:.1} -> {:.1} m/s; worst car {:+.2}/{:+.2} g vertical, {:.2} g lateral, {:.2} g longitudinal (headroom {:+.2} g){}",
                piece.id,
                piece.entry_speed_mps,
                piece.exit_speed_mps,
                peaks.minimum_vertical_g,
                peaks.maximum_vertical_g,
                peaks.maximum_lateral_g,
                peaks.maximum_longitudinal_g,
                piece.force_headroom_g,
                match (piece.rated_window_mps, piece.rated_margin_mps) {
                    (Some((minimum, maximum)), Some(margin)) => format!(
                        "; rated {minimum:.1}-{maximum:.1} m/s (margin {margin:+.1} m/s)"
                    ),
                    _ => String::new(),
                },
            )
            .unwrap();
        }
        for failure in &check.failures {
            writeln!(text, "   ride check failure: {failure}").unwrap();
        }
    }
    text
}

pub(crate) fn finish_options_json(completions: &[Completion], checks: &[CompletionCheck]) -> Value {
    json!({
        "completions": completions
            .iter()
            .zip(checks)
            .map(|(completion, check)| json!({
                "rank": completion.score,
                "pieces": completion
                    .geometry
                    .iter()
                    .map(|piece| format!("{:?}", piece.id))
                    .collect::<Vec<_>>(),
                "physics": completion.physics,
                "rideCheck": {
                    "passed": check.passed(),
                    "failures": check.failures,
                },
            }))
            .collect::<Vec<_>>(),
    })
}
