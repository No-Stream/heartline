use super::{CellFailure, CellRefusal, GridCellOutcome, GridElement, coaster_models};
use crate::track::HillFeel;

#[test]
fn impossible_hill_intents_return_typed_diagnostic_before_profile_synthesis() {
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

    for (model, feel, requested_target_g) in [
        (family, HillFeel::Floater, -0.5),
        (hyper, HillFeel::Ejector, -1.2),
        (wooden_twister, HillFeel::StrongFloater, -0.9),
    ] {
        let minimum_vertical_g = model.comfortable_force_limits.minimum_vertical_g;
        let maximum_intent_vertical_g = requested_target_g + super::MAXIMUM_AIRTIME_ERROR_G;
        let required_minimum_vertical_g = minimum_vertical_g + super::MINIMUM_FORCE_HEADROOM_G;
        assert!(maximum_intent_vertical_g < required_minimum_vertical_g);

        let cell = super::generate_hill_cell(model, feel, requested_target_g, 28.0);
        assert_eq!(
            cell.element,
            GridElement::Hill {
                feel,
                strength_g: requested_target_g,
            }
        );
        assert_eq!(
            cell.outcome,
            GridCellOutcome::Refused(CellRefusal::InfeasibleTarget {
                target_g: requested_target_g,
                delivered_g: None,
                available_headroom_g: Some(maximum_intent_vertical_g - minimum_vertical_g),
                failure: CellFailure::IntentHeadroomConflict {
                    required_minimum_vertical_g,
                    maximum_intent_vertical_g,
                },
            })
        );
    }
}
