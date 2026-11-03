use super::BlockCommandError;
use super::tests::{block_test_park, construction_inputs, unverified_layout};
use crate::track::blocks::brake_runs;

#[test]
fn block_commands_return_typed_errors_without_overwriting_park_messages() {
    let mut park = block_test_park();
    let block_message = "existing block error";
    let construction_message = "existing construction message";
    park.block_message = block_message.into();
    park.construction_message = construction_message.into();
    let construction = park.construction.take().expect("test coaster exists");

    assert_eq!(
        park.construction_auto_blocks(),
        Err(BlockCommandError::ClosedCircuitRequiredForPlacement)
    );
    assert_eq!(
        park.construction_set_block_sections(Vec::new()),
        Err(BlockCommandError::ClosedCircuitRequiredForEditing)
    );
    assert_eq!(
        park.construction_apply_best_block_sections(),
        Err(BlockCommandError::NoAutomaticSearch)
    );
    assert_eq!(
        park.construction_add_block_section(10.0),
        Err(BlockCommandError::NoBlockLayout)
    );
    assert_eq!(
        park.construction_move_block_section(0, 10.0),
        Err(BlockCommandError::NoBlockLayout)
    );
    assert_eq!(
        park.construction_remove_block_section(0),
        Err(BlockCommandError::NoBlockLayout)
    );
    assert_eq!(park.block_message, block_message);
    assert_eq!(park.construction_message, construction_message);

    park.construction = Some(construction);
    let layout = unverified_layout(&park);
    park.install_block_layout(layout, false);
    park.block_message = block_message.into();
    park.construction_message = construction_message.into();
    assert_eq!(
        park.construction_set_block_sections(vec![f64::NAN]),
        Err(BlockCommandError::Validation(
            "block boundary is outside the circuit"
        ))
    );
    assert_eq!(
        park.construction_move_block_section(usize::MAX, 10.0),
        Err(BlockCommandError::IndexOutOfRange)
    );
    assert_eq!(
        park.construction_remove_block_section(usize::MAX),
        Err(BlockCommandError::IndexOutOfRange)
    );
    assert_eq!(park.block_message, block_message);
    assert_eq!(park.construction_message, construction_message);
}

#[test]
fn asynchronous_validation_error_does_not_replace_completion_status() {
    let mut park = block_test_park();
    let (circuit, train) = construction_inputs(&park);
    let runs = brake_runs(&circuit, train.length_metres);
    let overlapping_pair = runs
        .iter()
        .enumerate()
        .find_map(|(index, first)| {
            runs[index + 1..].iter().find_map(|second| {
                (first.boundary_metres != second.boundary_metres
                    && first
                        .piece_indices
                        .iter()
                        .any(|piece| second.piece_indices.contains(piece)))
                .then_some((first.boundary_metres, second.boundary_metres))
            })
        })
        .expect("fixture has overlapping candidate brakes");
    park.install_block_layout(unverified_layout(&park), false);
    park.construction_set_block_sections(vec![overlapping_pair.0, overlapping_pair.1])
        .expect("candidate boundaries are accepted for asynchronous validation");
    let block_message = "existing block error";
    let construction_message = "existing construction message";
    park.block_message = block_message.into();
    park.construction_message = construction_message.into();

    let mut analysis_complete = false;
    let mut validation_error = None;
    for _ in 0..10_000 {
        let (complete, error) = park.construction_advance_block_analysis(1_000);
        analysis_complete = complete;
        if error.is_some() {
            validation_error = error;
            break;
        }
    }

    assert_eq!(
        validation_error,
        Some(BlockCommandError::Validation(
            "block brake runs cannot overlap"
        ))
    );
    assert!(!analysis_complete);
    assert!(park.block_throughput_estimator.is_some());
    assert_eq!(park.block_message, block_message);
    assert_eq!(park.construction_message, construction_message);

    for _ in 0..1_000 {
        let (complete, error) = park.construction_advance_block_analysis(1_000);
        assert!(error.is_none(), "validation error is returned once");
        if complete {
            analysis_complete = true;
            break;
        }
    }
    assert!(
        analysis_complete,
        "remaining throughput work eventually completes"
    );
}
