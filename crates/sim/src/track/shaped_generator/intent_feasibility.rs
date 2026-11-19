pub(super) fn conflict(
    minimum_vertical_g: f64,
    requested_target_g: f64,
    minimum_force_headroom_g: f64,
    maximum_airtime_error_g: f64,
) -> Option<(f64, f64)> {
    assert!(minimum_vertical_g.is_finite());
    assert!(requested_target_g.is_finite());
    assert!(minimum_force_headroom_g.is_finite() && minimum_force_headroom_g >= 0.0);
    assert!(maximum_airtime_error_g.is_finite() && maximum_airtime_error_g >= 0.0);

    let required_minimum_vertical_g = minimum_vertical_g + minimum_force_headroom_g;
    let maximum_intent_vertical_g = requested_target_g + maximum_airtime_error_g;
    (maximum_intent_vertical_g < required_minimum_vertical_g)
        .then_some((required_minimum_vertical_g, maximum_intent_vertical_g))
}
