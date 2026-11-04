//! The paint a coaster's track, supports and trains are printed in, chosen from a fixed palette.
//!
//! The simulation stores only palette names; the renderer owns the actual inks (`web/src/coaster-colors.ts`), and the
//! two lists must name the same colours in the same order.

use crate::state_hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoasterColor {
    Coral,
    Mustard,
    Timber,
    Walnut,
    Rose,
    Plum,
    Slate,
    Sky,
    Teal,
    Sage,
    Cream,
    Charcoal,
}

impl CoasterColor {
    pub const ALL: [Self; 12] = [
        Self::Coral,
        Self::Mustard,
        Self::Timber,
        Self::Walnut,
        Self::Rose,
        Self::Plum,
        Self::Slate,
        Self::Sky,
        Self::Teal,
        Self::Sage,
        Self::Cream,
        Self::Charcoal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Coral => "coral",
            Self::Mustard => "mustard",
            Self::Timber => "timber",
            Self::Walnut => "walnut",
            Self::Rose => "rose",
            Self::Plum => "plum",
            Self::Slate => "slate",
            Self::Sky => "sky",
            Self::Teal => "teal",
            Self::Sage => "sage",
            Self::Cream => "cream",
            Self::Charcoal => "charcoal",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.name() == name)
    }
}

/// One coaster's colour scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoasterColors {
    pub track: CoasterColor,
    pub supports: CoasterColor,
    pub trains: CoasterColor,
}

impl CoasterColors {
    /// Parses three palette names, as the construction panel sends them; `None` if any is not in the palette.
    pub fn from_names(track: &str, supports: &str, trains: &str) -> Option<Self> {
        Some(Self {
            track: CoasterColor::from_name(track)?,
            supports: CoasterColor::from_name(supports)?,
            trains: CoasterColor::from_name(trains)?,
        })
    }

    /// The track, supports and train colour names, separated by tabs.
    pub fn names(self) -> String {
        format!(
            "{}\t{}\t{}",
            self.track.name(),
            self.supports.name(),
            self.trains.name()
        )
    }

    pub(crate) fn hash_into(self, hash: &mut StateHasher) {
        hash.write_u8(self.track as u8);
        hash.write_u8(self.supports as u8);
        hash.write_u8(self.trains as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_name_round_trips_and_is_unique() {
        for color in CoasterColor::ALL {
            assert_eq!(CoasterColor::from_name(color.name()), Some(color));
        }
        let mut names: Vec<_> = CoasterColor::ALL.iter().map(|color| color.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CoasterColor::ALL.len());
    }

    #[test]
    fn schemes_parse_from_names_and_refuse_colours_outside_the_palette() {
        let scheme = CoasterColors::from_names("teal", "cream", "coral").expect("palette colours");
        assert_eq!(scheme.names(), "teal\tcream\tcoral");
        assert_eq!(CoasterColors::from_names("teal", "#ffffff", "coral"), None);
        assert_eq!(CoasterColors::from_names("Teal", "cream", "coral"), None);
        assert_eq!(CoasterColors::from_names("", "cream", "coral"), None);
    }
}
