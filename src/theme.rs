//! The two page themes a galaxy is drawn on, and their exact opaque backgrounds.

/// The dark page background, opaque `#0d1117`.
pub const DARK_BACKGROUND: [u8; 4] = [0x0d, 0x11, 0x17, 0xff];
/// The light page background, opaque `#ffffff`.
pub const LIGHT_BACKGROUND: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// One of the two page themes a galaxy can be drawn for.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Theme {
    /// A galaxy on the dark page background.
    Dark,
    /// A galaxy on the light page background.
    Light,
}

impl Theme {
    /// This theme's exact opaque background color.
    pub fn background(self) -> [u8; 4] {
        match self {
            Self::Dark => DARK_BACKGROUND,
            Self::Light => LIGHT_BACKGROUND,
        }
    }

    /// This theme's name in output files and messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}
