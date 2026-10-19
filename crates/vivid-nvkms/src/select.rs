//! Which ABI generation to try for a driver version (ADR-0004).

use std::fmt;

use crate::{GENERATIONS, Generation, Layout, Version};

/// How sure we are that a generation fits the driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    /// The version is inside the range of one generation in the table.
    Known,
    /// The version is newer than anything in the table. Every generation is a guess, newest first.
    Newer,
    /// The version is older than anything in the table.
    Older,
    /// The version falls in a gap between two generations: either could be right.
    Between,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Known => "known",
            Self::Newer => "newer than the table, validated",
            Self::Older => "older than the table, validated",
            Self::Between => "between two known generations, validated",
        })
    }
}

/// A layout to try, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The generation the layout comes from.
    pub generation: u32,
    /// The layout to try. For a guess it may carry another generation's `SET`/`GET` numbers.
    pub layout:     Layout,
    /// How sure we are.
    pub confidence: Confidence,
}

/// The layouts to try for `version`, most likely first.
///
/// A version inside a generation's range gives exactly that generation: no guessing. Any other
/// version gives every generation, nearest first, and for each one every distinct pair of
/// `SET`/`GET_DPY_ATTRIBUTE` command numbers the table has ever used (its own first). The caller
/// must validate each candidate against the real driver before trusting it.
#[must_use]
pub fn candidates(version: Version) -> Vec<Candidate> {
    candidates_in(GENERATIONS, version)
}

pub(crate) fn candidates_in(table: &[Generation], version: Version) -> Vec<Candidate> {
    if let Some(known) = table.iter().find(|g| g.first <= version && version <= g.last) {
        return vec![Candidate {
            generation: known.id,
            layout:     known.layout,
            confidence: Confidence::Known,
        }];
    }
    let confidence: Confidence = match (table.first(), table.last()) {
        (_, Some(last)) if version > last.last => Confidence::Newer,
        (Some(first), _) if version < first.first => Confidence::Older,
        _ => Confidence::Between,
    };

    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for generation in table {
        let pair: (u32, u32) = (generation.layout.cmd_set_dpy_attribute, generation.layout.cmd_get_dpy_attribute);
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }

    // Nearest first: for a newer version that is the newest generation, for an older one the
    // oldest, and for a gap the neighbours before the rest.
    let mut ordered: Vec<&Generation> = table.iter().collect();
    ordered.sort_by_key(|g| match confidence {
        Confidence::Older => (g.first.major().abs_diff(version.major()), g.id),
        _ => (g.last.major().abs_diff(version.major()), u32::MAX - g.id),
    });

    let mut out: Vec<Candidate> = Vec::new();
    for generation in ordered {
        let own: (u32, u32) = (generation.layout.cmd_set_dpy_attribute, generation.layout.cmd_get_dpy_attribute);
        let others = pairs.iter().copied().filter(|pair| *pair != own);
        for (set, get) in std::iter::once(own).chain(others) {
            let layout: Layout = Layout {
                cmd_set_dpy_attribute: set,
                cmd_get_dpy_attribute: get,
                ..generation.layout
            };
            out.push(Candidate {
                generation: generation.id,
                layout,
                confidence,
            });
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("test version")
    }

    #[test]
    fn every_known_version_selects_exactly_its_generation() {
        for generation in GENERATIONS {
            for edge in [generation.first, generation.last] {
                let found: Vec<Candidate> = candidates(edge);
                assert_eq!(found.len(), 1, "generation {} at {edge}", generation.id);
                assert_eq!(
                    found.first().map(|c| (c.generation, c.confidence)),
                    Some((generation.id, Confidence::Known))
                );
                assert_eq!(found.first().map(|c| c.layout), Some(generation.layout));
            }
        }
    }

    #[test]
    fn the_two_test_machines_select_the_generations_found_in_the_abi_map() {
        // Notebook: 580.178.04 is generation 7. Desktop: 595.99.02 is generation 8.
        assert_eq!(candidates(version("580.178.04")).first().map(|c| c.generation), Some(7));
        assert_eq!(candidates(version("595.99.02")).first().map(|c| c.generation), Some(8));
    }

    #[test]
    fn the_570_series_is_split_at_570_207() {
        assert_eq!(candidates(version("570.195.03")).first().map(|c| c.generation), Some(5));
        assert_eq!(candidates(version("570.207")).first().map(|c| c.generation), Some(6));
    }

    #[test]
    fn a_newer_version_tries_the_newest_generation_first_and_then_all_the_others() {
        let found: Vec<Candidate> = candidates(version("700.1.2"));
        assert_eq!(
            found.first().map(|c| (c.generation, c.confidence)),
            Some((10, Confidence::Newer))
        );
        let generations: Vec<u32> = found.iter().map(|c| c.generation).collect();
        for id in 1..=10 {
            assert!(generations.contains(&id), "generation {id} should be tried");
        }
    }

    #[test]
    fn a_guess_also_tries_the_other_command_pair() {
        let found: Vec<Candidate> = candidates(version("700.1.2"));
        let first_two: Vec<(u32, u32)> = found
            .iter()
            .take(2)
            .map(|c| (c.layout.cmd_set_dpy_attribute, c.layout.cmd_get_dpy_attribute))
            .collect();
        // The newest generation uses 21/22; the other pair in the table is 22/23.
        assert_eq!(first_two, [(21, 22), (22, 23)]);
        assert!(found.iter().take(2).all(|c| c.generation == 10));
    }

    #[test]
    fn an_older_version_tries_the_oldest_generation_first() {
        let found: Vec<Candidate> = candidates(version("470.1.0"));
        assert_eq!(
            found.first().map(|c| (c.generation, c.confidence)),
            Some((1, Confidence::Older))
        );
    }

    #[test]
    fn a_version_in_a_gap_is_a_validated_guess_with_its_neighbours_first() {
        // Between 555.58.02 (generation 2) and 560.28.03 (generation 3).
        let found: Vec<Candidate> = candidates(version("557.1.0"));
        assert_eq!(found.first().map(|c| c.confidence), Some(Confidence::Between));
        let first_generations: Vec<u32> = found.iter().take(4).map(|c| c.generation).collect();
        assert!(
            first_generations.contains(&2) || first_generations.contains(&3),
            "{first_generations:?}"
        );
    }

    #[test]
    fn an_empty_table_gives_nothing_to_try() {
        assert_eq!(candidates_in(&[], version("595.99.02")), Vec::<Candidate>::new());
    }

    #[test]
    fn confidence_reads_well() {
        assert_eq!(Confidence::Known.to_string(), "known");
        assert!(Confidence::Newer.to_string().contains("validated"));
    }
}
