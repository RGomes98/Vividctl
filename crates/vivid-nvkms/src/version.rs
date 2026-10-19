//! Driver versions, for choosing an ABI generation.

use std::{error::Error, fmt, str::FromStr};

/// A driver version such as `595.99.02` or `570.207`, as three numbers.
///
/// Two-part versions have a patch of 0. Leading zeros do not matter (`99.02` is 99, 2). This is for
/// comparing and ordering only: the string sent to the driver is the original text, which must
/// match the driver's own byte for byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    /// A version from its three numbers.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// The major (series) number, such as 595.
    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The text is not a driver version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionError(String);

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a driver version like 595.99.02", self.0)
    }
}

impl Error for VersionError {}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || VersionError(text.to_owned());
        let mut numbers: Vec<u32> = Vec::with_capacity(3);
        for part in text.trim().split('.') {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            numbers.push(part.parse().map_err(|_| bad())?);
        }
        match numbers.as_slice() {
            [major, minor] => Ok(Self::new(*major, *minor, 0)),
            [major, minor, patch] => Ok(Self::new(*major, *minor, *patch)),
            _ => Err(bad()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Version, VersionError> {
        text.parse()
    }

    #[test]
    fn parses_three_and_two_part_versions() {
        assert_eq!(parse("595.99.02"), Ok(Version::new(595, 99, 2)));
        assert_eq!(parse("570.207"), Ok(Version::new(570, 207, 0)));
        assert_eq!(parse("565.77"), Ok(Version::new(565, 77, 0)));
    }

    #[test]
    fn ignores_surrounding_whitespace_such_as_the_newline_in_sysfs() {
        assert_eq!(parse("580.178.04\n"), Ok(Version::new(580, 178, 4)));
        assert_eq!(parse("  535.43.02 "), Ok(Version::new(535, 43, 2)));
    }

    #[test]
    fn rejects_everything_else() {
        for text in [
            "",
            ".",
            "595",
            "595.",
            ".99",
            "1.2.3.4",
            "595.99.02-beta",
            "abc",
            "595.-1",
            "595.99.0x2",
            "99999999999.1",
        ] {
            assert!(parse(text).is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn orders_numerically_not_as_text() {
        assert!(Version::new(570, 86, 15) < Version::new(570, 207, 0));
        assert!(Version::new(570, 195, 3) < Version::new(570, 207, 0));
        assert!(Version::new(535, 309, 1) < Version::new(545, 23, 6));
        assert!(parse("580.9.01").expect("valid") < parse("580.10.01").expect("valid"));
    }

    #[test]
    fn the_error_shows_the_offending_text() {
        assert_eq!(
            parse("nope").map_err(|e| e.to_string()),
            Err("\"nope\" is not a driver version like 595.99.02".to_owned())
        );
    }
}
