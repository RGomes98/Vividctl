//! The Digital Vibrance value.

use std::fmt;

use crate::error::VibranceOutOfRange;

/// A Digital Vibrance level in NVIDIA's native range, `-1024..=1023`. `0` is neutral.
///
/// Positive values boost saturation, negative values reduce it. The range is the one the NVKMS
/// attribute accepts, so a `Vibrance` can be handed to a backend without further checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vibrance(i16);

impl Vibrance {
    /// The lowest level (most washed out).
    pub const MIN: Self = Self(-1024);
    /// The highest level (most saturated).
    pub const MAX: Self = Self(1023);
    /// No change to the image.
    pub const NEUTRAL: Self = Self(0);

    /// Validates a raw value.
    ///
    /// # Errors
    ///
    /// Returns [`VibranceOutOfRange`] when `value` is outside `-1024..=1023`.
    pub fn new(value: i64) -> Result<Self, VibranceOutOfRange> {
        i16::try_from(value)
            .ok()
            .filter(|v| (Self::MIN.0..=Self::MAX.0).contains(v))
            .map(Self)
            .ok_or(VibranceOutOfRange(value))
    }

    /// Brings any value into range: values below the minimum become [`Self::MIN`], above the
    /// maximum become [`Self::MAX`].
    #[must_use]
    pub fn clamped(value: i64) -> Self {
        let bounded: i64 = value.clamp(i64::from(Self::MIN.0), i64::from(Self::MAX.0));
        // The clamp above keeps the value inside i16, so the conversion cannot fail.
        Self(i16::try_from(bounded).unwrap_or_default())
    }

    /// The raw value.
    #[must_use]
    pub const fn get(self) -> i16 {
        self.0
    }

    /// Whether this is [`Self::NEUTRAL`].
    #[must_use]
    pub const fn is_neutral(self) -> bool {
        self.0 == 0
    }
}

impl TryFrom<i64> for Vibrance {
    type Error = VibranceOutOfRange;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Vibrance> for i64 {
    fn from(value: Vibrance) -> Self {
        Self::from(value.0)
    }
}

impl fmt::Display for Vibrance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_whole_range_including_the_edges() {
        assert_eq!(Vibrance::new(-1024), Ok(Vibrance::MIN));
        assert_eq!(Vibrance::new(0), Ok(Vibrance::NEUTRAL));
        assert_eq!(Vibrance::new(1023), Ok(Vibrance::MAX));
        assert_eq!(Vibrance::new(512).map(Vibrance::get), Ok(512));
    }

    #[test]
    fn rejects_just_outside_the_range() {
        assert_eq!(Vibrance::new(-1025), Err(VibranceOutOfRange(-1025)));
        assert_eq!(Vibrance::new(1024), Err(VibranceOutOfRange(1024)));
    }

    #[test]
    fn rejects_values_that_do_not_fit_in_i16() {
        assert_eq!(Vibrance::new(i64::MAX), Err(VibranceOutOfRange(i64::MAX)));
        assert_eq!(Vibrance::new(i64::MIN), Err(VibranceOutOfRange(i64::MIN)));
        assert_eq!(Vibrance::new(70_000), Err(VibranceOutOfRange(70_000)));
    }

    #[test]
    fn clamping_pins_to_the_nearest_edge() {
        assert_eq!(Vibrance::clamped(-5000), Vibrance::MIN);
        assert_eq!(Vibrance::clamped(5000), Vibrance::MAX);
        assert_eq!(Vibrance::clamped(i64::MIN), Vibrance::MIN);
        assert_eq!(Vibrance::clamped(i64::MAX), Vibrance::MAX);
        assert_eq!(Vibrance::clamped(300).get(), 300);
    }

    #[test]
    fn default_is_neutral() {
        assert_eq!(Vibrance::default(), Vibrance::NEUTRAL);
        assert!(Vibrance::NEUTRAL.is_neutral());
        assert!(!Vibrance::MAX.is_neutral());
    }

    #[test]
    fn converts_to_and_from_i64() {
        assert_eq!(i64::from(Vibrance::MIN), -1024);
        assert_eq!(Vibrance::try_from(7_i64).map(i64::from), Ok(7));
        assert!(Vibrance::try_from(2000_i64).is_err());
    }

    #[test]
    fn orders_by_value_and_prints_the_number() {
        assert!(Vibrance::MIN < Vibrance::NEUTRAL && Vibrance::NEUTRAL < Vibrance::MAX);
        assert_eq!(Vibrance::MIN.to_string(), "-1024");
    }
}
