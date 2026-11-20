use theme_park_sim::track::ride_file::{
    RideFile, RideFileFormat, RideHeading, RideShapeStep, RideStation, RideStep,
};
use theme_park_sim::track::shaped::{
    ElementIntent, ElementSpec, HillSizeHint, ResolvedHillProfile, ResolvedTurnGeometry,
    SpeedTurnKnot, TurnSizeHint,
};
use theme_park_sim::track::{Hand, HillFeel};

fn ride_with_non_decimal_shaped_geometry() -> RideFile {
    let profile: ResolvedHillProfile = serde_json::from_value(serde_json::json!({
        "points": [{
            "distance": 0.123456789012345,
            "x": 0.234567890123456,
            "z": 0.345678901234567,
            "angle": 0.456789012345678,
            "heart_curvature": 0.567890123456789,
            "curvature": 0.678901234567891
        }],
        "cached_minimum_entry": 0.789012345678912,
        "cached_minimum_radius": 1.890123456789123
    }))
    .unwrap();

    RideFile {
        format: RideFileFormat::HeartlineRide,
        version: 1,
        name: "Exact shaped geometry".to_owned(),
        model: "hyper".to_owned(),
        station: RideStation {
            x: 0,
            y: 0,
            z: 0,
            heading: RideHeading::East,
        },
        cars_per_train: 8,
        trains: 1,
        steps: vec![
            RideStep::Shape(RideShapeStep {
                intent: ElementIntent::Hill {
                    feel: HillFeel::Floater,
                    strength_g: Some(-0.5123456789012345),
                    size: HillSizeHint::Height(36.12345678901234),
                },
                spec: ElementSpec::Hill {
                    design_speed_mps: 34.12345678901234,
                    cars: 8,
                    target_g: -0.5123456789012345,
                    delivered_g: -0.4987654321098765,
                    height_m: 36.12345678901234,
                    span_m: 64.98765432109876,
                    profile,
                },
            }),
            RideStep::Shape(RideShapeStep {
                intent: ElementIntent::Turn {
                    hand: Hand::Left,
                    size: TurnSizeHint::Radius(64.12345678901234),
                },
                spec: ElementSpec::Turn {
                    design_speed_mps: 28.12345678901234,
                    cars: 8,
                    hand: Hand::Left,
                    vertical_g: 3.123456789012345,
                    lateral_g: 0.0123456789012345,
                    radius_m: 64.12345678901234,
                    bank_degrees: 65.12345678901234,
                    geometry: ResolvedTurnGeometry {
                        peak_bank: 65.12345678901234_f64.to_radians(),
                        length: 100.12345678901234,
                        knots: vec![
                            SpeedTurnKnot {
                                x: 0.123456789012345,
                                y: 0.234567890123456,
                                heading: 0.345678901234567,
                            },
                            SpeedTurnKnot {
                                x: 64.12345678901234,
                                y: 64.23456789012345,
                                heading: 1.456789012345678,
                            },
                        ],
                    },
                },
            }),
        ],
        expect: None,
    }
}

#[test]
fn shaped_ride_file_preserves_exact_f64_values_through_json_string_and_value() {
    let ride = ride_with_non_decimal_shaped_geometry();
    let serialized = serde_json::to_string(&ride).unwrap();
    let serialized_value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(serialized_value["steps"][0]["spec"]["profile"]["points"][0]["angle"].is_number());
    assert!(serialized_value["steps"][1]["spec"]["geometry"]["peak_bank"].is_number());

    let restored_from_string: RideFile = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored_from_string, ride);

    let restored_from_value: RideFile = serde_json::from_value(serialized_value).unwrap();
    assert_eq!(restored_from_value, ride);
}
