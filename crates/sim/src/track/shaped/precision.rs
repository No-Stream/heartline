use super::ElementSpec;

const ELEMENT_SPEC_DECIMAL_SCALE: f64 = 1_000_000.0;

pub(super) fn round_element_value(value: f64) -> f64 {
    (value * ELEMENT_SPEC_DECIMAL_SCALE).round() / ELEMENT_SPEC_DECIMAL_SCALE
}

pub(crate) fn canonicalize_element_spec(spec: &mut ElementSpec) {
    match spec {
        ElementSpec::Hill {
            design_speed_mps,
            target_g,
            delivered_g,
            height_m,
            span_m,
            profile,
            ..
        } => {
            *design_speed_mps = round_element_value(*design_speed_mps);
            *target_g = round_element_value(*target_g);
            *delivered_g = round_element_value(*delivered_g);
            *height_m = round_element_value(*height_m);
            *span_m = round_element_value(*span_m);
            profile.round_to_nine_decimal_places();
        }
        ElementSpec::Turn {
            design_speed_mps,
            vertical_g,
            lateral_g,
            radius_m,
            bank_degrees,
            geometry,
            ..
        } => {
            *design_speed_mps = round_element_value(*design_speed_mps);
            *vertical_g = round_element_value(*vertical_g);
            *lateral_g = round_element_value(*lateral_g);
            *radius_m = round_element_value(*radius_m);
            *bank_degrees = round_element_value(*bank_degrees);
            geometry.peak_bank = round_element_value(geometry.peak_bank);
            geometry.length = round_element_value(geometry.length);
            for knot in &mut geometry.knots {
                knot.x = round_element_value(knot.x);
                knot.y = round_element_value(knot.y);
                knot.heading = round_element_value(knot.heading);
            }
        }
    }
}
