use super::ElementSpec;

const ELEMENT_SPEC_DECIMAL_SCALE: f64 = 1_000_000.0;
pub(super) const ELEMENT_SPEC_HALF_DECIMAL_STEP: f64 = 0.5 / ELEMENT_SPEC_DECIMAL_SCALE;

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
        ElementSpec::Loop {
            design_speed_mps,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            height_m,
            span_m,
            profile,
            ..
        } => {
            *design_speed_mps = round_element_value(*design_speed_mps);
            *target_peak_g = round_element_value(*target_peak_g);
            *delivered_peak_g = round_element_value(*delivered_peak_g);
            *delivered_top_g = round_element_value(*delivered_top_g);
            *height_m = round_element_value(*height_m);
            *span_m = round_element_value(*span_m);
            profile.round_to_nine_decimal_places();
        }
        ElementSpec::ZeroGRoll {
            design_speed_mps,
            target_peak_g,
            delivered_peak_g,
            delivered_crest_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            profile,
            ..
        } => {
            *design_speed_mps = round_element_value(*design_speed_mps);
            *target_peak_g = round_element_value(*target_peak_g);
            *delivered_peak_g = round_element_value(*delivered_peak_g);
            *delivered_crest_g = round_element_value(*delivered_crest_g);
            *peak_roll_rate_deg_s = round_element_value(*peak_roll_rate_deg_s);
            *height_m = round_element_value(*height_m);
            *span_m = round_element_value(*span_m);
            profile.round_to_nine_decimal_places();
        }
        ElementSpec::RavenTurn {
            design_speed_mps,
            target_peak_g,
            delivered_peak_g,
            delivered_top_g,
            peak_roll_rate_deg_s,
            height_m,
            span_m,
            roll_dx_m,
            roll_lateral_m,
            half_loop,
            ..
        } => {
            *design_speed_mps = round_element_value(*design_speed_mps);
            *target_peak_g = round_element_value(*target_peak_g);
            *delivered_peak_g = round_element_value(*delivered_peak_g);
            *delivered_top_g = round_element_value(*delivered_top_g);
            *peak_roll_rate_deg_s = round_element_value(*peak_roll_rate_deg_s);
            *height_m = round_element_value(*height_m);
            *span_m = round_element_value(*span_m);
            *roll_dx_m = round_element_value(*roll_dx_m);
            *roll_lateral_m = round_element_value(*roll_lateral_m);
            half_loop.round_to_nine_decimal_places();
        }
    }
}
