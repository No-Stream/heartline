use crate::physics::GRAVITY;
use crate::track::curves::{HEART_HEIGHT_METRES, Piece, Shape};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PredictedHillForces {
    pub maximum_absolute_lateral_g: f64,
    pub minimum_vertical_g: f64,
    pub maximum_vertical_g: f64,
    pub airtime_seconds: f64,
}

/// Estimates point-mass forces at the heart, ignoring drag and rolling resistance.
/// Returns `None` for non-hills or if the estimated speed reaches zero on the hill.
pub fn predicted_hill_forces(piece: &Piece, entry_speed: f64) -> Option<PredictedHillForces> {
    assert!(entry_speed.is_finite() && entry_speed >= 0.0);
    piece.family?;
    let (profile, twist) = match &piece.shape {
        Shape::Profile(profile) => (profile, None),
        Shape::RolledProfile(profile, twist) => (profile, Some(*twist)),
        _ => return None,
    };
    let mut result = PredictedHillForces {
        maximum_absolute_lateral_g: 0.0,
        minimum_vertical_g: f64::INFINITY,
        maximum_vertical_g: f64::NEG_INFINITY,
        airtime_seconds: 0.0,
    };
    let mut previous: Option<(f64, f64, f64, f64)> = None;
    for (distance, rail_height, angle, heart_curvature, rail_curvature) in profile.force_samples() {
        let heart_height = rail_height + HEART_HEIGHT_METRES * (angle.cos() - 1.0);
        let speed_squared = entry_speed.powi(2) - 2.0 * GRAVITY * heart_height;
        if speed_squared <= 0.0 {
            return None;
        }
        let speed = speed_squared.sqrt();
        let roll = twist.map_or(0.0, |twist| twist.sample(distance, piece.mirrored).0);
        let planar_g = angle.cos() + heart_curvature * speed_squared / GRAVITY;
        let vertical_g = planar_g * roll.cos();
        result.maximum_absolute_lateral_g = result
            .maximum_absolute_lateral_g
            .max((planar_g * roll.sin()).abs());
        result.minimum_vertical_g = result.minimum_vertical_g.min(vertical_g);
        result.maximum_vertical_g = result.maximum_vertical_g.max(vertical_g);
        let heart_distance_factor = 1.0 - HEART_HEIGHT_METRES * rail_curvature;
        if let Some((previous_distance, previous_speed, previous_g, previous_factor)) = previous {
            let negative_fraction = if previous_g < 0.0 && vertical_g < 0.0 {
                1.0
            } else if previous_g < 0.0 {
                -previous_g / (vertical_g - previous_g)
            } else if vertical_g < 0.0 {
                -vertical_g / (previous_g - vertical_g)
            } else {
                0.0
            };
            result.airtime_seconds +=
                (distance - previous_distance) * (heart_distance_factor + previous_factor) / 2.0
                    * negative_fraction
                    * 2.0
                    / (speed + previous_speed);
        }
        previous = Some((distance, speed, vertical_g, heart_distance_factor));
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use crate::track::{PieceId, catalog, predicted_hill_forces};

    #[test]
    fn faster_entry_produces_more_airtime_and_lower_crest_force() {
        let hill = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::EjectorHill8x48)
            .unwrap();
        let slow = predicted_hill_forces(hill, 15.0).unwrap();
        let fast = predicted_hill_forces(hill, 18.0).unwrap();
        assert!(fast.minimum_vertical_g < slow.minimum_vertical_g);
        assert!(fast.maximum_vertical_g > slow.maximum_vertical_g);
        assert!(fast.airtime_seconds > slow.airtime_seconds);
        assert!(fast.airtime_seconds > 0.0);
        assert!(predicted_hill_forces(hill, 0.0).is_none());
    }

    #[test]
    fn mirrored_outer_banks_have_equal_predicted_forces() {
        let left = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::EjectorHill8x48OuterLeft)
            .unwrap();
        let right = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::EjectorHill8x48OuterRight)
            .unwrap();
        let straight = catalog()
            .iter()
            .find(|piece| piece.id == PieceId::EjectorHill8x48)
            .unwrap();
        let left_forces = predicted_hill_forces(left, 18.0).unwrap();
        assert_eq!(left_forces, predicted_hill_forces(right, 18.0).unwrap());
        assert_ne!(left_forces, predicted_hill_forces(straight, 18.0).unwrap());
    }
}
