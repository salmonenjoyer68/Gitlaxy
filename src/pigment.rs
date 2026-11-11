//! Language pigments: which star colors an account's language mix contributes.
//!
//! Each language's GitHub color maps to the koi pigment nearest in hue: red, orange,
//! yellow, or blue. Grays and languages without a color become white or gold so they
//! stay visible on a galaxy. The language shares weighting each pigment decide, in a
//! fixed order, which accent color a seeded placement point takes. The renderer keeps
//! the chosen colorway dominant and uses these pigments only for a subset of stars and
//! arm accents.

use std::collections::BTreeMap;

use crate::galaxy::GalaxyDevelopment;
use crate::history::LanguageColor;
use crate::theme::Theme;

/// One accent pigment a language can contribute; declaration order breaks ties.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Pigment {
    /// Red, including deep rose hues.
    Red,
    /// White, for grays and languages without a color.
    White,
    /// Gold, for dark grays.
    Gold,
    /// Yellow, including olive and chartreuse hues.
    Yellow,
    /// Orange, including rust and amber hues.
    Orange,
    /// Blue, including teal, indigo, and violet hues.
    Blue,
}

/// One pigment and its share of all counted language weight, from zero through one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PigmentShare {
    /// The accent pigment.
    pub pigment: Pigment,
    /// Its normalized share of the counted languages, in the same units as language shares.
    pub share: f64,
}

/// GitHub's language color as a pigment; grays and missing colors become white or gold.
///
/// Colors with too little chroma or saturation are grayscale: lighter grays become
/// [`Pigment::White`], darker grays become [`Pigment::Gold`]. A language with no
/// recorded color becomes [`Pigment::White`]. Chromatic colors take the nearest of the
/// four koi pigments by hue: red below 18 degrees or from 330 degrees, orange through
/// 45 degrees, yellow through 160 degrees, and blue for the rest.
pub fn derive_pigment(language_color: Option<LanguageColor>) -> Pigment {
    const MIN_CHROMA: f64 = 0.12;
    const MIN_SATURATION: f64 = 0.2;
    const DARK_LIGHTNESS: f64 = 0.5;
    let Some(LanguageColor(color_channels)) = language_color else {
        return Pigment::White;
    };
    let [red, green, blue] = color_channels.map(|color_channel| f64::from(color_channel) / 255.0);
    let max_channel = red.max(green).max(blue);
    let min_channel = red.min(green).min(blue);
    let chroma = max_channel - min_channel;
    let lightness = (max_channel + min_channel) / 2.0;
    if chroma < MIN_CHROMA || chroma / (1.0 - (2.0 * lightness - 1.0).abs()) < MIN_SATURATION {
        return if lightness < DARK_LIGHTNESS {
            Pigment::Gold
        } else {
            Pigment::White
        };
    }
    let hue_degrees = if max_channel == red {
        60.0 * ((green - blue) / chroma).rem_euclid(6.0)
    } else if max_channel == green {
        60.0 * ((blue - red) / chroma + 2.0)
    } else {
        60.0 * ((red - green) / chroma + 4.0)
    };
    match hue_degrees {
        hue_degrees if !(18.0..330.0).contains(&hue_degrees) => Pigment::Red,
        hue_degrees if hue_degrees < 45.0 => Pigment::Orange,
        hue_degrees if hue_degrees < 160.0 => Pigment::Yellow,
        _ => Pigment::Blue,
    }
}

/// Sum each pigment's language shares, largest first, with equal shares broken by
/// [`Pigment`] declaration order.
///
/// Shares are read through both maps, never through a hash map, so insertion order
/// cannot change the result. Shares are normalized by the total; an empty or zero-weight
/// mix produces no pigments. Rounded shares rank first, so sums that differ only in
/// their last floating bits still tie.
pub fn calculate_pigment_shares(
    language_shares: &BTreeMap<String, f64>,
    language_colors: &BTreeMap<String, Option<LanguageColor>>,
) -> Vec<PigmentShare> {
    let total_share: f64 = language_shares.values().sum();
    if total_share <= 0.0 {
        return Vec::new();
    }
    let mut summed_shares: BTreeMap<Pigment, f64> = BTreeMap::new();
    for (language_name, language_share) in language_shares {
        let language_color = language_colors.get(language_name).copied().flatten();
        *summed_shares
            .entry(derive_pigment(language_color))
            .or_insert(0.0) += language_share / total_share;
    }
    let rank_share = |pigment_share: f64| (pigment_share * 1e9).round();
    let mut pigment_shares: Vec<PigmentShare> = summed_shares
        .into_iter()
        .map(|(pigment, share)| PigmentShare { pigment, share })
        .collect();
    pigment_shares.sort_by(|left, right| {
        rank_share(right.share)
            .total_cmp(&rank_share(left.share))
            .then(left.pigment.cmp(&right.pigment))
    });
    pigment_shares
}

/// The pigments for one development, ordered by prominence.
pub fn pigment_shares(development: &GalaxyDevelopment) -> Vec<PigmentShare> {
    calculate_pigment_shares(&development.language_shares, &development.language_colors)
}

/// Pick a pigment weighted by share from one deterministic value in zero through one.
///
/// `unit` below a pigment's cumulative share picks it; the highest pigment takes the
/// top of the range. An empty mix has no pigment.
pub fn select_pigment(pigment_shares: &[PigmentShare], unit: f64) -> Option<Pigment> {
    let mut cumulative_share = 0.0;
    let mut last_pigment = None;
    for pigment_share in pigment_shares {
        cumulative_share += pigment_share.share;
        last_pigment = Some(pigment_share.pigment);
        if unit < cumulative_share {
            return last_pigment;
        }
    }
    last_pigment
}

/// One accent color for a pigment at a brightness from zero through one.
///
/// The theme picks the ramp: bright pigments on the dark page, deeper ink on the light
/// page, so grays and gold never disappear into the background. Brightness is quantized
/// to the ramp's four steps, keeping a frame's color count bounded.
pub fn pigment_color(pigment: Pigment, theme: Theme, brightness: f64) -> [u8; 4] {
    let ramp = match theme {
        Theme::Dark => dark_ramp(pigment),
        Theme::Light => light_ramp(pigment),
    };
    let index = (brightness.clamp(0.0, 1.0) * (ramp.len() - 1) as f64).round() as usize;
    let [red, green, blue] = ramp[index];
    [red, green, blue, 0xff]
}

fn dark_ramp(pigment: Pigment) -> &'static [[u8; 3]; 4] {
    match pigment {
        Pigment::Red => &[
            [0x8c, 0x1d, 0x2c],
            [0xc9, 0x31, 0x43],
            [0xf0, 0x54, 0x64],
            [0xff, 0x9b, 0xa5],
        ],
        Pigment::Orange => &[
            [0x8a, 0x45, 0x13],
            [0xc4, 0x62, 0x1b],
            [0xf0, 0x8a, 0x2e],
            [0xff, 0xc0, 0x6b],
        ],
        Pigment::Yellow => &[
            [0x7a, 0x63, 0x12],
            [0xbb, 0x99, 0x1a],
            [0xe8, 0xc8, 0x2c],
            [0xff, 0xf0, 0x8a],
        ],
        Pigment::Blue => &[
            [0x1c, 0x3f, 0x7a],
            [0x28, 0x62, 0xb6],
            [0x3f, 0x8c, 0xe0],
            [0x8c, 0xc8, 0xff],
        ],
        Pigment::White => &[
            [0x6b, 0x72, 0x80],
            [0x9a, 0xa3, 0xb2],
            [0xcb, 0xd2, 0xdd],
            [0xff, 0xff, 0xff],
        ],
        Pigment::Gold => &[
            [0x7d, 0x5a, 0x12],
            [0xb4, 0x86, 0x1a],
            [0xe2, 0xb4, 0x2e],
            [0xff, 0xe0, 0x7a],
        ],
    }
}

fn light_ramp(pigment: Pigment) -> &'static [[u8; 3]; 4] {
    match pigment {
        Pigment::Red => &[
            [0x5c, 0x12, 0x1c],
            [0x8b, 0x1e, 0x2b],
            [0xb8, 0x2c, 0x3c],
            [0xdd, 0x62, 0x70],
        ],
        Pigment::Orange => &[
            [0x6b, 0x33, 0x0a],
            [0x9a, 0x4b, 0x10],
            [0xc7, 0x66, 0x1a],
            [0xe8, 0x9a, 0x4a],
        ],
        Pigment::Yellow => &[
            [0x5c, 0x48, 0x08],
            [0x87, 0x6a, 0x0c],
            [0xb0, 0x8e, 0x14],
            [0xd6, 0xb8, 0x4a],
        ],
        Pigment::Blue => &[
            [0x12, 0x2f, 0x5c],
            [0x1c, 0x48, 0x8a],
            [0x2a, 0x68, 0xbb],
            [0x5c, 0x96, 0xdd],
        ],
        Pigment::White => &[
            [0x55, 0x5b, 0x66],
            [0x7a, 0x82, 0x8d],
            [0xa2, 0xa9, 0xb4],
            [0xc8, 0xce, 0xd6],
        ],
        Pigment::Gold => &[
            [0x5c, 0x41, 0x08],
            [0x87, 0x60, 0x0c],
            [0xb0, 0x82, 0x14],
            [0xd6, 0xac, 0x4a],
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shares(entries: &[(&str, f64)]) -> BTreeMap<String, f64> {
        entries
            .iter()
            .map(|(language_name, share)| (language_name.to_string(), *share))
            .collect()
    }

    fn colors(
        entries: &[(&str, Option<LanguageColor>)],
    ) -> BTreeMap<String, Option<LanguageColor>> {
        entries
            .iter()
            .map(|(language_name, color)| (language_name.to_string(), *color))
            .collect()
    }

    fn color(hex_digits: [u8; 3]) -> Option<LanguageColor> {
        Some(LanguageColor(hex_digits))
    }

    #[test]
    fn language_colors_map_to_the_nearest_pigment() {
        assert_eq!(derive_pigment(color([0xde, 0xa5, 0x84])), Pigment::Orange);
        assert_eq!(derive_pigment(color([0x35, 0x72, 0xa5])), Pigment::Blue);
        assert_eq!(derive_pigment(color([0xf1, 0xe0, 0x5a])), Pigment::Yellow);
        assert_eq!(derive_pigment(color([0x70, 0x15, 0x16])), Pigment::Red);
        assert_eq!(derive_pigment(color([0x55, 0x55, 0x55])), Pigment::Gold);
        assert_eq!(derive_pigment(color([0xcc, 0xcc, 0xcc])), Pigment::White);
        assert_eq!(derive_pigment(None), Pigment::White);
    }

    #[test]
    fn equal_shares_add_per_pigment_and_rank_by_share_then_pigment_order() {
        let pigment_shares = calculate_pigment_shares(
            &shares(&[
                ("Rust", 0.25),
                ("Ruby", 0.25),
                ("Python", 0.25),
                ("JavaScript", 0.25),
            ]),
            &colors(&[
                ("Rust", color([0xde, 0xa5, 0x84])),
                ("Ruby", color([0x70, 0x15, 0x16])),
                ("Python", color([0xde, 0xa5, 0x84])),
                ("JavaScript", color([0xf1, 0xe0, 0x5a])),
            ]),
        );
        assert_eq!(
            pigment_shares,
            vec![
                PigmentShare {
                    pigment: Pigment::Orange,
                    share: 0.5
                },
                PigmentShare {
                    pigment: Pigment::Red,
                    share: 0.25
                },
                PigmentShare {
                    pigment: Pigment::Yellow,
                    share: 0.25
                },
            ]
        );
    }

    #[test]
    fn a_tie_between_equal_rounded_shares_uses_pigment_order_stably() {
        let language_shares = shares(&[("Red", 0.3), ("JavaScript", 0.3), ("Rust", 0.4)]);
        let language_colors = colors(&[
            ("Red", color([0x70, 0x15, 0x16])),
            ("JavaScript", color([0xf1, 0xe0, 0x5a])),
            ("Rust", None),
        ]);
        let first = calculate_pigment_shares(&language_shares, &language_colors);
        let second = calculate_pigment_shares(&language_shares, &language_colors);
        assert_eq!(first, second);
        assert_eq!(first[0].pigment, Pigment::White);
        assert_eq!(first[1].pigment, Pigment::Red);
        assert_eq!(first[2].pigment, Pigment::Yellow);
    }

    #[test]
    fn an_empty_or_zero_weight_mix_has_no_pigments() {
        assert!(calculate_pigment_shares(&shares(&[]), &colors(&[])).is_empty());
        assert!(calculate_pigment_shares(&shares(&[("Rust", 0.0)]), &colors(&[])).is_empty());
    }

    #[test]
    fn weighted_selection_covers_the_share_boundaries() {
        let pigment_shares = vec![
            PigmentShare {
                pigment: Pigment::Red,
                share: 0.75,
            },
            PigmentShare {
                pigment: Pigment::Blue,
                share: 0.25,
            },
        ];
        assert_eq!(select_pigment(&pigment_shares, 0.0), Some(Pigment::Red));
        assert_eq!(select_pigment(&pigment_shares, 0.75), Some(Pigment::Blue));
        assert_eq!(select_pigment(&pigment_shares, 1.0), Some(Pigment::Blue));
        assert_eq!(select_pigment(&[], 0.5), None);
    }

    #[test]
    fn every_pigment_stays_on_a_four_step_ramp_in_both_themes() {
        for pigment in [
            Pigment::Red,
            Pigment::White,
            Pigment::Gold,
            Pigment::Yellow,
            Pigment::Orange,
            Pigment::Blue,
        ] {
            for theme in [Theme::Dark, Theme::Light] {
                let dim = pigment_color(pigment, theme, 0.0);
                let bright = pigment_color(pigment, theme, 1.0);
                assert_ne!(dim, bright);
                assert_eq!(dim[3], 0xff);
                assert_eq!(bright[3], 0xff);
            }
        }
    }
}
