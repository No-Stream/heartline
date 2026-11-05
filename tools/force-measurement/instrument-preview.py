"""Expose the preview step and add an evaluator in a disposable source tree."""
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
module = root / "crates/sim/src/physics/mod.rs"
source = module.read_text()
source = source.replace("mod comfort;", "pub mod force_evaluation;\nmod comfort;")
source = source.replace("    fn step_preview(", "    pub fn step_preview(")
source = source.replace(
    "pub use preview::{PreviewStatus, TrackPreview, preview_open_track};",
    "pub use preview::{PreviewStatus, TrackPreview, preview_open_track, preview_at_step};",
)
module.write_text(source)
preview = root / "crates/sim/src/physics/preview.rs"
source = preview.read_text()
source = source.replace("pub(super) fn preview_at_step(", "pub fn preview_at_step(")
source = source.replace(
    "    time_step: f64,\n) -> Option<TrackPreview>",
    "    time_step: f64,\n    limits: Option<crate::track::ComfortableForceLimits>,\n) -> Option<TrackPreview>",
)
source = source.replace("        PREVIEW_TIME_STEP,\n    )", "        PREVIEW_TIME_STEP,\n        None,\n    )")
source = source.replace(
    "    let mut stationary_steps = 0;",
    "    let mut evaluator = limits.map(|limits| super::force_evaluation::ForceEvaluator::new(limits, time_step));\n    let mut stationary_steps = 0;",
)
source = source.replace(
    "        recorder.record(&train.take_readings());",
    "        let mut readings = train.take_readings();\n        if let Some(evaluator) = evaluator.as_mut() {\n            for frame in &mut readings {\n                frame[0].g = evaluator.update(frame[0].g).filtered;\n            }\n        }\n        recorder.record(&readings);",
)
preview.write_text(source)
