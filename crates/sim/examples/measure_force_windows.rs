use theme_park_sim::physics::rated_piece::{rate_piece, unpowered};
use theme_park_sim::track::catalog::unmeasured_catalog;
use theme_park_sim::track::{
    AirtimeRating, HillFeel, Piece, PieceId, hybrid_model, multi_launch_model, wooden_twister_model,
};

fn refine_boundary(
    mut lower: f64,
    mut upper: f64,
    lower_edge: bool,
    passes: impl Fn(f64) -> bool,
) -> f64 {
    for _ in 0..12 {
        let middle = (lower + upper) / 2.0;
        if passes(middle) == lower_edge {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    if lower_edge {
        (upper * 100.0).ceil() / 100.0
    } else {
        (lower * 100.0).floor() / 100.0
    }
}

fn failure(piece: &Piece, speed: f64) -> Option<String> {
    let mut evaluated = false;
    for model in [hybrid_model(), multi_launch_model(), wooden_twister_model()] {
        if !model.allows_piece(piece) {
            continue;
        }
        evaluated = true;
        for cars in model.cars_per_train_range() {
            let rating =
                rate_piece(&model, cars, piece, speed, unpowered()).unwrap_or_else(|error| {
                    panic!(
                        "{:?}, {} {cars} cars, {speed} m/s: {error}",
                        piece.id, model.name
                    )
                });
            if !rating.passed {
                return Some(format!("{} {cars} cars: {rating:?}", model.name));
            }
            if rating.exceeds_sustained_limits(&model.comfortable_force_limits) {
                return Some(format!(
                    "{} {cars} cars: filtered extrema {:?} to {:?}, limits {:?}",
                    model.name,
                    rating.minimum_filtered_force,
                    rating.maximum_filtered_force,
                    model.comfortable_force_limits
                ));
            }
        }
    }
    assert!(evaluated, "no allowed model for {:?}", piece.id);
    None
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let selected_piece = arguments.next();
    assert!(
        arguments.next().is_none(),
        "usage: measure_force_windows [piece-id]"
    );
    let update = std::env::var_os("UPDATE_RATED_WINDOWS").is_some();
    assert!(
        !update || selected_piece.is_none(),
        "generation requires every template"
    );
    let mut matched = false;
    let mut generated = Vec::new();
    let mut acceptance_failures = Vec::new();
    let templates = unmeasured_catalog();
    for piece in templates
        .iter()
        .filter(|piece| piece.rated_speeds.is_some())
    {
        if selected_piece
            .as_ref()
            .is_some_and(|name| *name != format!("{:?}", piece.id))
        {
            continue;
        }
        matched = true;
        let old = piece.rated_speeds.unwrap();
        let ceiling = 100.0_f64.max(3.0 * old.maximum_entry);
        println!(
            "START {:?} entry {:?} exit {:?} old {:.6} {:.6} ceiling {:.2}",
            piece.id, piece.entry, piece.exit, old.minimum_entry, old.maximum_entry, ceiling
        );
        let mut first_pass = None;
        let mut last_pass = None;
        let mut first_failure = None;
        let mut gap = None;
        let mut after_pass_failure = false;
        for index in 1..=(ceiling / 0.25).ceil() as usize {
            let speed = index as f64 * 0.25;
            match failure(piece, speed) {
                None => {
                    if after_pass_failure && gap.is_none() {
                        gap = Some((last_pass.unwrap(), speed));
                    }
                    first_pass.get_or_insert(speed);
                    last_pass = Some(speed);
                }
                Some(diagnostic) => {
                    if index > 1 && first_failure.is_none() {
                        first_failure = Some((speed, diagnostic.clone()));
                    }
                    if first_pass.is_some() {
                        after_pass_failure = true;
                    }
                }
            }
        }
        let old_min_failure = failure(piece, old.minimum_entry);
        let old_max_failure = failure(piece, old.maximum_entry);
        println!(
            "RESULT {:?} entry {:?} exit {:?} first {first_pass:?} last {last_pass:?} gap {gap:?} first_failure {first_failure:?} old_min_failure {old_min_failure:?} old_max_failure {old_max_failure:?}",
            piece.id, piece.entry, piece.exit
        );
        assert!(
            first_pass.is_some() && gap.is_none(),
            "{:?}: no contiguous window",
            piece.id
        );
        let first = first_pass.unwrap();
        let last = last_pass.unwrap();
        let measured_minimum = if first > 0.25 {
            refine_boundary(first - 0.25, first, true, |speed| {
                failure(piece, speed).is_none()
            })
        } else {
            first
        };
        let minimum = old.minimum_entry.max(measured_minimum);
        let reached_ceiling = last == (ceiling / 0.25).ceil() * 0.25;
        assert!(
            !reached_ceiling || old_max_failure.is_none(),
            "{:?}: old maximum fails inside a passing grid window: {old_max_failure:?}",
            piece.id
        );
        let upper_failure = if reached_ceiling {
            None
        } else {
            failure(piece, last + 0.25)
        };
        let design_cap = reached_ceiling;
        let maximum = if design_cap {
            old.maximum_entry
        } else {
            refine_boundary(last, last + 0.25, false, |speed| {
                failure(piece, speed).is_none()
            })
        };
        assert!(minimum <= maximum, "{:?}: no rated window", piece.id);
        assert!(failure(piece, minimum).is_none());
        assert!(failure(piece, maximum).is_none());
        println!(
            "REFINED {:?} minimum {minimum:.6} maximum {maximum:.6} design_cap {design_cap} boundary_failure {upper_failure:?}",
            piece.id
        );
        if update {
            let loop_piece = matches!(
                piece.id,
                PieceId::Loop
                    | PieceId::LoopRight
                    | PieceId::LargeLoop
                    | PieceId::LargeLoopRight
                    | PieceId::SteelLoop
                    | PieceId::SteelLoopRight
            );
            let required_width = if loop_piece { 1.5 } else { 2.0 };
            if maximum - minimum < required_width {
                acceptance_failures.push(format!(
                    "{:?}: width {} is below {required_width}",
                    piece.id,
                    maximum - minimum
                ));
            }
            if piece
                .family
                .is_some_and(|family| family.feel == HillFeel::Ejector && family.twist.is_none())
            {
                for model in [hybrid_model(), multi_launch_model(), wooden_twister_model()] {
                    if !model.allows_piece(piece) {
                        continue;
                    }
                    let cars = model.train.cars;
                    let has_great_airtime = (0..=((maximum - minimum) / 0.25).floor() as usize)
                        .map(|index| minimum + index as f64 * 0.25)
                        .chain(std::iter::once(maximum))
                        .any(|speed| {
                            let rating =
                                rate_piece(&model, cars, piece, speed, unpowered()).unwrap();
                            model
                                .comfortable_force_limits
                                .classify_airtime(rating.minimum_filtered_vertical)
                                == AirtimeRating::Great
                        });
                    if !has_great_airtime {
                        acceptance_failures.push(format!(
                            "{:?} {} default {cars} cars: no Great airtime on sampled grid; feasibility unresolved",
                            piece.id, model.name
                        ));
                    }
                }
            }
            let lower_source = if measured_minimum <= old.minimum_entry {
                "EnergyMinimum"
            } else if failure(piece, first - 0.25).is_some_and(|diagnostic| {
                diagnostic.contains("minimum clearance speed")
                    || diagnostic.contains("stalled or rolled back")
            }) {
                "ClearanceMinimum"
            } else {
                "ForceBound"
            };
            let upper_source = if design_cap {
                "DesignCap"
            } else if upper_failure
                .as_ref()
                .is_some_and(|diagnostic| diagnostic.contains("clearing speed exceeds 22 m/s"))
            {
                "ClearanceMaximum"
            } else {
                "ForceBound"
            };
            generated.push(format!("    (PieceId::{:?}, Slope::{:?}, Slope::{:?}, RatedWindowSource::{lower_source}, RatedWindowSource::{upper_source}, {measured_minimum:?}, {minimum:?}, {maximum:?}),", piece.id, piece.entry.slope, piece.exit.slope));
        }
    }
    assert!(matched, "unknown piece or no rated pieces");
    if update {
        assert!(
            acceptance_failures.is_empty(),
            "rated window acceptance failures:\n{}",
            acceptance_failures.join("\n")
        );
        let date = std::process::Command::new("date")
            .args(["-u", "+%F"])
            .output()
            .expect("run date for table provenance");
        assert!(date.status.success(), "date command failed");
        let date = std::str::from_utf8(&date.stdout)
            .expect("date output is UTF-8")
            .trim();
        let contents = format!(
            "// Generated by measure_force_windows; {date}; 5 Hz filter, 0.5 g allowance for 0.2 s.\n\
             pub(super) const MEASURED_RATED_WINDOWS: &[(PieceId, Slope, Slope, RatedWindowSource, RatedWindowSource, f64, f64, f64)] = &[\n{}\n];\n",
            generated.join("\n")
        );
        std::fs::write("crates/sim/src/track/rated_windows.generated.rs", contents)
            .expect("write generated rated windows");
    }
}

#[cfg(test)]
mod tests {
    use super::refine_boundary;
    use std::collections::HashSet;
    use theme_park_sim::physics::rated_piece::PieceRating;
    use theme_park_sim::track::catalog;

    #[test]
    fn rejects_any_filtered_peak_past_a_sustained_limit() {
        let limits = theme_park_sim::track::hybrid_model().comfortable_force_limits;
        let mut rating = PieceRating {
            passed: true,
            entry_speed: 1.0,
            minimum_speed: 1.0,
            minimum_filtered_vertical: limits.minimum_vertical_g,
            minimum_filtered_force: [
                limits.minimum_vertical_g,
                -limits.maximum_lateral_g,
                -limits.maximum_longitudinal_g,
            ],
            maximum_filtered_force: [
                limits.maximum_vertical_g,
                limits.maximum_lateral_g,
                limits.maximum_longitudinal_g,
            ],
            violation: None,
            failure: None,
        };
        assert!(!rating.exceeds_sustained_limits(&limits));
        for axis in 0..3 {
            rating.minimum_filtered_force[axis] -= 0.001;
            assert!(rating.exceeds_sustained_limits(&limits));
            rating.minimum_filtered_force[axis] += 0.001;
            rating.maximum_filtered_force[axis] += 0.001;
            assert!(rating.exceeds_sustained_limits(&limits));
            rating.maximum_filtered_force[axis] -= 0.001;
        }
    }

    #[test]
    fn refines_both_edges_and_rounds_into_the_passing_window() {
        let passes = |speed: f64| (5.123..=20.987).contains(&speed);
        assert_eq!(refine_boundary(5.0, 5.25, true, passes), 5.13);
        assert_eq!(refine_boundary(20.75, 21.0, false, passes), 20.98);
    }

    #[test]
    fn rated_template_keys_distinguish_catalog_variants() {
        let mut keys = HashSet::new();
        for piece in catalog()
            .iter()
            .filter(|piece| piece.rated_speeds.is_some())
        {
            let key = format!(
                "{:?} {:?} {:?}",
                piece.id, piece.entry.slope, piece.exit.slope
            );
            assert!(keys.insert(key.clone()), "duplicate template key: {key}");
        }
        eprintln!("rated templates: {}", keys.len());
    }
}
