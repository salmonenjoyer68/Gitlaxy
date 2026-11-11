//! The six galaxy colorways and their per-theme color ramps.
//!
//! Palettes are sampled from the reference galaxy sheets in `docs/`: each colorway is
//! one horizontal band of six stages, and its anchors are the dominant colors of that
//! band. The renderer interpolates the anchors into a 128-level ordered-dither ramp that
//! starts at the exact theme background.

use crate::theme::Theme;

/// One of the six galaxy colorways a render or the command line can choose.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum Colorway {
    /// Cool blue, the default colorway.
    #[default]
    Blue,
    /// Red and rose.
    Red,
    /// Orange and amber.
    Orange,
    /// Violet and lilac.
    Purple,
    /// Green and teal.
    Green,
    /// Pink and mauve.
    Pink,
}

impl Colorway {
    /// Every colorway, in the order the reference sheets stack their bands.
    pub const ALL: [Self; 6] = [
        Self::Blue,
        Self::Red,
        Self::Orange,
        Self::Purple,
        Self::Green,
        Self::Pink,
    ];

    /// Parse a command-line colorway name, or `None` when the name is unknown.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|colorway| colorway.name() == name)
    }

    /// This colorway's name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Purple => "purple",
            Self::Green => "green",
            Self::Pink => "pink",
        }
    }

    /// The exact colors this colorway blends through on `theme`.
    ///
    /// The first anchor is the theme background and the last is the brightest core
    /// color, so the renderer can walk from background to core without knowing the
    /// theme. On the light theme the galaxy darkens as it brightens, as ink on paper.
    pub fn palette(self, theme: Theme) -> Palette {
        let ramp = match theme {
            Theme::Dark => self.dark_ramp(),
            Theme::Light => self.light_ramp(),
        };
        let background = theme.background();
        let mut anchors = [[0; 3]; 7];
        anchors[0] = [background[0], background[1], background[2]];
        for (index, color) in ramp.into_iter().enumerate() {
            let anchor_index = match theme {
                Theme::Dark => index + 1,
                Theme::Light => 6 - index,
            };
            anchors[anchor_index] = color;
        }
        Palette { anchors }
    }

    fn dark_ramp(self) -> [[u8; 3]; 6] {
        match self {
            Self::Blue => [
                [0x05, 0x12, 0x29],
                [0x1f, 0x39, 0x6a],
                [0x32, 0x59, 0x94],
                [0x4b, 0x82, 0xbf],
                [0x75, 0xb7, 0xe4],
                [0xba, 0xeb, 0xf9],
            ],
            Self::Red => [
                [0x10, 0x0d, 0x27],
                [0x2e, 0x29, 0x5c],
                [0x6b, 0x22, 0x3f],
                [0x52, 0x57, 0x9b],
                [0xad, 0x49, 0x58],
                [0xe2, 0xa7, 0xa0],
            ],
            Self::Orange => [
                [0x1c, 0x15, 0x1c],
                [0x60, 0x39, 0x38],
                [0x38, 0x50, 0x78],
                [0x8a, 0x65, 0x62],
                [0xc6, 0x61, 0x25],
                [0xf3, 0xb0, 0x62],
            ],
            Self::Purple => [
                [0x16, 0x0a, 0x30],
                [0x22, 0x10, 0x46],
                [0x36, 0x1c, 0x67],
                [0x56, 0x30, 0x98],
                [0x85, 0x5e, 0xd5],
                [0xd1, 0xc6, 0xe8],
            ],
            Self::Green => [
                [0x08, 0x21, 0x21],
                [0x1b, 0x4c, 0x43],
                [0x34, 0x73, 0x5b],
                [0x65, 0x91, 0x60],
                [0x4f, 0xb1, 0x89],
                [0x90, 0xdd, 0xa9],
            ],
            Self::Pink => [
                [0x1a, 0x09, 0x1d],
                [0x32, 0x13, 0x37],
                [0x55, 0x26, 0x57],
                [0x7c, 0x44, 0x79],
                [0xa1, 0x6c, 0x9a],
                [0xd1, 0xac, 0xc9],
            ],
        }
    }

    fn light_ramp(self) -> [[u8; 3]; 6] {
        match self {
            Self::Blue => [
                [0x23, 0x3a, 0x69],
                [0x32, 0x56, 0x8e],
                [0x44, 0x73, 0xae],
                [0x5d, 0x95, 0xcc],
                [0x89, 0xc4, 0xe9],
                [0xc7, 0xef, 0xfa],
            ],
            Self::Red => [
                [0x32, 0x28, 0x55],
                [0x7f, 0x2d, 0x47],
                [0x45, 0x53, 0x95],
                [0x7f, 0x65, 0x96],
                [0xc8, 0x62, 0x63],
                [0xd8, 0xc1, 0xc8],
            ],
            Self::Orange => [
                [0x2e, 0x34, 0x56],
                [0x51, 0x50, 0x6c],
                [0xa2, 0x55, 0x3b],
                [0x71, 0x7b, 0x9a],
                [0xe4, 0x89, 0x3e],
                [0xe9, 0xd9, 0xbb],
            ],
            Self::Purple => [
                [0x24, 0x14, 0x3e],
                [0x37, 0x1f, 0x61],
                [0x4f, 0x30, 0x84],
                [0x6e, 0x49, 0xae],
                [0x93, 0x70, 0xda],
                [0xd1, 0xc6, 0xe8],
            ],
            Self::Green => [
                [0x30, 0x65, 0x51],
                [0x3d, 0x92, 0x74],
                [0x67, 0x91, 0x5f],
                [0x69, 0xc2, 0x91],
                [0x9a, 0xe4, 0xb4],
                [0xd0, 0xf8, 0xe0],
            ],
            Self::Pink => [
                [0x68, 0x34, 0x63],
                [0x8a, 0x52, 0x82],
                [0xb2, 0x68, 0xa4],
                [0xa2, 0x7a, 0x9a],
                [0xc1, 0x9a, 0xb9],
                [0xec, 0xca, 0xe5],
            ],
        }
    }
}

/// Seven RGB anchors ordered from the exact theme background to the brightest color.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Palette {
    /// The anchor colors the renderer interpolates into its 128-level ramp.
    pub anchors: [[u8; 3]; 7],
}
