//! Platform-independent floating-point transcendental functions.
//!
//! The standard library delegates these operations to the host's `libm`, whose
//! results can vary by a few ULPs between operating systems and glibc versions.
//! Simulation outputs use these wrappers so native builds share the same
//! implementation as wasm builds.

#[inline]
pub fn sin(value: f64) -> f64 {
    libm::sin(value)
}

#[inline]
pub fn cos(value: f64) -> f64 {
    libm::cos(value)
}

#[inline]
pub fn sin_cos(value: f64) -> (f64, f64) {
    libm::sincos(value)
}

#[inline]
pub fn tan(value: f64) -> f64 {
    libm::tan(value)
}

#[inline]
pub fn asin(value: f64) -> f64 {
    libm::asin(value)
}

#[inline]
pub fn acos(value: f64) -> f64 {
    libm::acos(value)
}

#[inline]
pub fn atan(value: f64) -> f64 {
    libm::atan(value)
}

#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

#[inline]
pub fn exp(value: f64) -> f64 {
    libm::exp(value)
}

#[inline]
pub fn ln(value: f64) -> f64 {
    libm::log(value)
}

#[inline]
pub fn powf(value: f64, exponent: f64) -> f64 {
    libm::pow(value, exponent)
}

#[inline]
pub fn powi(value: f64, exponent: i32) -> f64 {
    let reciprocal = exponent < 0;
    let mut exponent = exponent.unsigned_abs();
    let mut factor = value;
    let mut result = 1.0;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result *= factor;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor *= factor;
        }
    }
    if reciprocal { 1.0 / result } else { result }
}

#[inline]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

#[inline]
pub fn sin_f32(value: f32) -> f32 {
    libm::sinf(value)
}

#[inline]
pub fn cos_f32(value: f32) -> f32 {
    libm::cosf(value)
}

#[inline]
pub fn sin_cos_f32(value: f32) -> (f32, f32) {
    libm::sincosf(value)
}

#[inline]
pub fn tan_f32(value: f32) -> f32 {
    libm::tanf(value)
}

#[inline]
pub fn asin_f32(value: f32) -> f32 {
    libm::asinf(value)
}

#[inline]
pub fn acos_f32(value: f32) -> f32 {
    libm::acosf(value)
}

#[inline]
pub fn atan_f32(value: f32) -> f32 {
    libm::atanf(value)
}

#[inline]
pub fn atan2_f32(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

#[inline]
pub fn exp_f32(value: f32) -> f32 {
    libm::expf(value)
}

#[inline]
pub fn ln_f32(value: f32) -> f32 {
    libm::logf(value)
}

#[inline]
pub fn powf_f32(value: f32, exponent: f32) -> f32 {
    libm::powf(value, exponent)
}

#[inline]
pub fn powi_f32(value: f32, exponent: i32) -> f32 {
    let reciprocal = exponent < 0;
    let mut exponent = exponent.unsigned_abs();
    let mut factor = value;
    let mut result = 1.0;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result *= factor;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor *= factor;
        }
    }
    if reciprocal { 1.0 / result } else { result }
}

#[inline]
pub fn hypot_f32(x: f32, y: f32) -> f32 {
    libm::hypotf(x, y)
}

#[cfg(test)]
mod tests {
    use super::{cos, hypot_f32, powi, powi_f32, sin, sin_cos};

    #[test]
    fn trigonometric_wrappers_match_the_recorded_libm_bits() {
        let value = f64::from_bits(0x3fc0_7ea3_1ae8_ab71);
        let (sine, cosine) = sin_cos(value);

        assert_eq!(sin(value).to_bits(), 0x3fc0_72f5_bc71_89c4);
        assert_eq!(cos(value).to_bits(), 0x3fef_bc13_5335_81bb);
        assert_eq!(sine.to_bits(), 0x3fc0_72f5_bc71_89c4);
        assert_eq!(cosine.to_bits(), 0x3fef_bc13_5335_81bb);
        assert_eq!(hypot_f32(3.0, 4.0), 5.0);
    }

    #[test]
    fn integer_power_wrapper_is_deterministic_for_signed_exponents() {
        assert_eq!(powi(1.5, 3), 3.375);
        assert_eq!(powi(2.0, -3), 0.125);
        assert_eq!(powi(-2.0, 3), -8.0);
        assert_eq!(powi(2.0, 0), 1.0);
        assert_eq!(powi_f32(2.0, -2), 0.25);
    }
}
