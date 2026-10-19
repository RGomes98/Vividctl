//! Pure color math: Digital Vibrance to saturation mapping used by previews and shaders.
//!
//! The mapping is **provisional**. NVIDIA does not document what a vibrance level does to the
//! image, and the calibration against real hardware is SP-08. Until then the curve below is a
//! simple, monotonic, exactly invertible stand-in: it is enough to drive a compositor shader and
//! to read the value back, and it is not a claim about what the NVIDIA display engine does.

#![forbid(unsafe_code)]

use vivid_core::Vibrance;

/// The saturation factor at which an image is left alone.
pub const NEUTRAL_SATURATION: f64 = 1.0;

/// The saturation reached at [`Vibrance::MAX`]. A guess, to be replaced by the SP-08 calibration.
pub const MAX_SATURATION: f64 = 2.0;

/// The saturation at [`Vibrance::MIN`]: no colour at all, a grayscale image.
pub const MIN_SATURATION: f64 = 0.0;

/// The saturation factor for a vibrance level: `1.0` at `0`, [`MIN_SATURATION`] at
/// [`Vibrance::MIN`] and [`MAX_SATURATION`] at [`Vibrance::MAX`], linear on each side.
#[must_use]
pub fn saturation_for(vibrance: Vibrance) -> f64 {
    let level: f64 = f64::from(vibrance.get());
    if level < 0.0 {
        NEUTRAL_SATURATION + level / -f64::from(Vibrance::MIN.get()) * (NEUTRAL_SATURATION - MIN_SATURATION)
    } else {
        NEUTRAL_SATURATION + level / f64::from(Vibrance::MAX.get()) * (MAX_SATURATION - NEUTRAL_SATURATION)
    }
}

/// The vibrance level closest to a saturation factor. It is the inverse of [`saturation_for`]:
/// every level survives the round trip. A factor outside the mapped range gives the nearest edge.
#[must_use]
pub fn vibrance_for(saturation: f64) -> Vibrance {
    let level: f64 = if saturation < NEUTRAL_SATURATION {
        (saturation - NEUTRAL_SATURATION) / (NEUTRAL_SATURATION - MIN_SATURATION) * -f64::from(Vibrance::MIN.get())
    } else {
        (saturation - NEUTRAL_SATURATION) / (MAX_SATURATION - NEUTRAL_SATURATION) * f64::from(Vibrance::MAX.get())
    };
    // A NaN has no meaning here; treating it as neutral is safer than an arbitrary edge.
    if level.is_nan() {
        return Vibrance::NEUTRAL;
    }
    // Both ends are clamped to the Vibrance range, so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    Vibrance::clamped(level.round() as i64)
}

#[cfg(test)]
// The tests index the windows of a slice they just built and narrow to f32 on purpose.
#[allow(clippy::indexing_slicing, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    fn every_level() -> impl Iterator<Item = Vibrance> {
        (i64::from(Vibrance::MIN)..=i64::from(Vibrance::MAX)).filter_map(|v| Vibrance::new(v).ok())
    }

    #[test]
    fn neutral_leaves_the_image_alone() {
        assert!((saturation_for(Vibrance::NEUTRAL) - NEUTRAL_SATURATION).abs() < f64::EPSILON);
    }

    #[test]
    fn the_edges_map_to_the_documented_extremes() {
        assert!((saturation_for(Vibrance::MIN) - MIN_SATURATION).abs() < f64::EPSILON);
        assert!((saturation_for(Vibrance::MAX) - MAX_SATURATION).abs() < f64::EPSILON);
    }

    #[test]
    fn more_vibrance_never_means_less_saturation() {
        let values: Vec<f64> = every_level().map(saturation_for).collect();
        assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn every_level_survives_the_round_trip() {
        for level in every_level() {
            assert_eq!(vibrance_for(saturation_for(level)), level, "level {level}");
        }
    }

    #[test]
    fn every_level_survives_a_round_trip_through_a_float_32_noise_floor() {
        // A readback goes through a D-Bus double, which is exact; this guards the rounding margin
        // against a backend that stores single precision.
        for level in every_level() {
            let stored: f64 = f64::from(saturation_for(level) as f32);
            assert_eq!(vibrance_for(stored), level, "level {level}");
        }
    }

    #[test]
    fn saturation_outside_the_mapped_range_gives_the_nearest_edge() {
        assert_eq!(vibrance_for(-3.0), Vibrance::MIN);
        assert_eq!(vibrance_for(9.0), Vibrance::MAX);
        assert_eq!(vibrance_for(f64::INFINITY), Vibrance::MAX);
        assert_eq!(vibrance_for(f64::NEG_INFINITY), Vibrance::MIN);
    }

    #[test]
    fn nan_is_neutral() {
        assert_eq!(vibrance_for(f64::NAN), Vibrance::NEUTRAL);
    }

    #[test]
    fn a_boost_stays_inside_what_the_extension_accepts() {
        // The extension refuses anything outside 0.0..=4.0.
        assert!(every_level().map(saturation_for).all(|s| (0.0..=4.0).contains(&s)));
    }
}
