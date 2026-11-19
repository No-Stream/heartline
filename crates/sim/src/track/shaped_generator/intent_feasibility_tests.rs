use super::coaster_models;

#[test]
fn mathematically_conflicting_hill_intents_are_detected_for_measured_capping() {
    let models = coaster_models();
    let family = models
        .iter()
        .find(|model| model.key == "family_coaster")
        .expect("family coaster exists");
    let hyper = models
        .iter()
        .find(|model| model.key == "hyper")
        .expect("hyper coaster exists");
    let wooden_twister = models
        .iter()
        .find(|model| model.key == "wooden_twister")
        .expect("wooden twister exists");

    for (model, requested_target_g) in [(family, -0.5), (hyper, -1.2), (wooden_twister, -0.9)] {
        let upper_delivery_g = requested_target_g + super::MAXIMUM_AIRTIME_ERROR_G;
        let required_minimum_g =
            model.comfortable_force_limits.minimum_vertical_g + super::MINIMUM_FORCE_HEADROOM_G;
        assert!(upper_delivery_g < required_minimum_g);
        assert_eq!(
            super::intent_feasibility::conflict(
                model.comfortable_force_limits.minimum_vertical_g,
                requested_target_g,
                super::MINIMUM_FORCE_HEADROOM_G,
                super::MAXIMUM_AIRTIME_ERROR_G,
            ),
            Some((required_minimum_g, upper_delivery_g))
        );
    }
}

#[test]
fn exact_headroom_boundary_is_allowed_and_lower_non_tie_target_is_refused() {
    let minimum_vertical_g = 0.25;
    let headroom_g = 0.125;
    let maximum_airtime_error_g = 0.125;
    let required_minimum_vertical_g = 0.375;
    let boundary_target_g = 0.25;
    assert_eq!(
        boundary_target_g + maximum_airtime_error_g,
        required_minimum_vertical_g
    );
    assert_eq!(
        super::intent_feasibility::conflict(
            minimum_vertical_g,
            boundary_target_g,
            headroom_g,
            maximum_airtime_error_g,
        ),
        None
    );
    let lower_target_g = f64::from_bits(boundary_target_g.to_bits() - 2);
    assert!(lower_target_g + maximum_airtime_error_g < required_minimum_vertical_g);
    assert!(
        super::intent_feasibility::conflict(
            minimum_vertical_g,
            lower_target_g,
            headroom_g,
            maximum_airtime_error_g,
        )
        .is_some()
    );
}
