use std::collections::BTreeMap;

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{GalaxyDevelopment, MAX_GROWTH_CONTRIBUTIONS, derive_galaxy_development};
use gitlaxy::history::{AccountSnapshot, LanguageColor};
use gitlaxy::pigment::{
    Pigment, calculate_pigment_shares, derive_pigment, pigment_color, select_pigment,
};
use gitlaxy::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, LOOP_FRAME_COUNT, render_galaxy_frame,
    render_galaxy_still,
};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");
const PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;

/// Rust, Python, JavaScript, Ruby, and a gray C repository, all counted equally.
fn read_development() -> GalaxyDevelopment {
    let account_snapshot: AccountSnapshot = serde_json::from_str(HISTORY_FIXTURE).unwrap();
    let mut galaxy_development = derive_galaxy_development(&account_snapshot).unwrap();
    galaxy_development.total_contributions = MAX_GROWTH_CONTRIBUTIONS;
    galaxy_development.language_shares = BTreeMap::from([
        ("Rust".to_string(), 0.2),
        ("Python".to_string(), 0.2),
        ("JavaScript".to_string(), 0.2),
        ("Ruby".to_string(), 0.2),
        ("C".to_string(), 0.2),
    ]);
    galaxy_development.language_colors = BTreeMap::from([
        ("Rust".to_string(), Some(LanguageColor([0xde, 0xa5, 0x84]))),
        (
            "Python".to_string(),
            Some(LanguageColor([0x35, 0x72, 0xa5])),
        ),
        (
            "JavaScript".to_string(),
            Some(LanguageColor([0xf1, 0xe0, 0x5a])),
        ),
        ("Ruby".to_string(), Some(LanguageColor([0x70, 0x15, 0x16]))),
        ("C".to_string(), Some(LanguageColor([0x55, 0x55, 0x55]))),
    ]);
    galaxy_development
}

fn without_languages() -> GalaxyDevelopment {
    let mut development = read_development();
    development.language_shares.clear();
    development.language_colors.clear();
    development
}

fn grayscale_development() -> GalaxyDevelopment {
    let mut development = read_development();
    development.language_shares = BTreeMap::from([
        ("DarkGray".to_string(), 0.5),
        ("LightGray".to_string(), 0.5),
    ]);
    development.language_colors = BTreeMap::from([
        (
            "DarkGray".to_string(),
            Some(LanguageColor([0x55, 0x55, 0x55])),
        ),
        (
            "LightGray".to_string(),
            Some(LanguageColor([0xcc, 0xcc, 0xcc])),
        ),
    ]);
    development
}

#[test]
fn language_colors_reach_the_expected_pigment_buckets() {
    assert_eq!(
        derive_pigment(Some(LanguageColor([0xde, 0xa5, 0x84]))),
        Pigment::Orange
    );
    assert_eq!(
        derive_pigment(Some(LanguageColor([0x35, 0x72, 0xa5]))),
        Pigment::Blue
    );
    assert_eq!(
        derive_pigment(Some(LanguageColor([0xf1, 0xe0, 0x5a]))),
        Pigment::Yellow
    );
    assert_eq!(
        derive_pigment(Some(LanguageColor([0x70, 0x15, 0x16]))),
        Pigment::Red
    );
    assert_eq!(
        derive_pigment(Some(LanguageColor([0x55, 0x55, 0x55]))),
        Pigment::Gold
    );
    assert_eq!(
        derive_pigment(Some(LanguageColor([0xcc, 0xcc, 0xcc]))),
        Pigment::White
    );
    assert_eq!(derive_pigment(None), Pigment::White);
}

#[test]
fn the_pigment_mix_covers_every_language_and_ranks_by_prominence() {
    let development = read_development();
    let pigment_shares =
        calculate_pigment_shares(&development.language_shares, &development.language_colors);
    let mut pigments: Vec<Pigment> = pigment_shares
        .iter()
        .map(|pigment_share| pigment_share.pigment)
        .collect();
    pigments.sort();
    assert_eq!(
        pigments,
        vec![
            Pigment::Red,
            Pigment::Gold,
            Pigment::Yellow,
            Pigment::Orange,
            Pigment::Blue,
        ]
    );
    assert_eq!(select_pigment(&pigment_shares, 0.0), Some(Pigment::Red));
    assert_eq!(select_pigment(&pigment_shares, 0.99), Some(Pigment::Blue));
}

#[test]
fn equal_shares_keep_a_stable_pigment_order() {
    let language_shares =
        BTreeMap::from([("Ruby".to_string(), 0.5), ("JavaScript".to_string(), 0.5)]);
    let language_colors = BTreeMap::from([
        ("Ruby".to_string(), Some(LanguageColor([0x70, 0x15, 0x16]))),
        (
            "JavaScript".to_string(),
            Some(LanguageColor([0xf1, 0xe0, 0x5a])),
        ),
    ]);
    let first = calculate_pigment_shares(&language_shares, &language_colors);
    let second = calculate_pigment_shares(&language_shares, &language_colors);
    assert_eq!(first, second);
    assert_eq!(first[0].pigment, Pigment::Red);
    assert_eq!(first[1].pigment, Pigment::Yellow);
}

#[test]
fn pigment_accents_change_pixels_but_leave_the_colorway_dominant() {
    let development = read_development();
    let plain = without_languages();
    for theme in [Theme::Dark, Theme::Light] {
        for colorway in Colorway::ALL {
            for frame_index in [0, 37] {
                let tinted = render_galaxy_frame(&development, colorway, theme, frame_index);
                let untinted = render_galaxy_frame(&plain, colorway, theme, frame_index);
                let changed = differing_pixels(&tinted, &untinted);
                let non_background = non_background_pixels(&untinted, theme.background());
                assert!(
                    changed > 0,
                    "{} on {theme:?} has no pigment pixels",
                    colorway.name()
                );
                assert!(
                    changed * 2 < non_background,
                    "{} on {theme:?} tints {changed} of {non_background} pixels",
                    colorway.name()
                );
            }
        }
    }
}

#[test]
fn grayscale_only_accounts_render_opaque_white_or_gold_frames() {
    let development = grayscale_development();
    let plain = without_languages();
    let grayscale_colors: Vec<[u8; 4]> = [Pigment::White, Pigment::Gold]
        .into_iter()
        .flat_map(|pigment| [Theme::Dark, Theme::Light].map(move |theme| (pigment, theme)))
        .flat_map(|(pigment, theme)| {
            [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
                .map(move |brightness| pigment_color(pigment, theme, brightness))
        })
        .collect();
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in [0, 60, LOOP_FRAME_COUNT - 1] {
            let frame = render_galaxy_frame(&development, Colorway::Blue, theme, frame_index);
            assert_eq!(frame.as_rgba_bytes().len(), PIXEL_COUNT * 4);
            assert!(
                frame
                    .as_rgba_bytes()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|[_, _, _, alpha]| *alpha == 0xff)
            );
            assert!(non_background_pixels(&frame, theme.background()) > 1_000);
            assert!(
                opaque_pixels(&frame)
                    .into_iter()
                    .any(|pixel| grayscale_colors.contains(&pixel)),
                "frame {frame_index} on {theme:?} shows no white or gold pigment"
            );
            assert!(
                differing_pixels(
                    &frame,
                    &render_galaxy_frame(&plain, Colorway::Blue, theme, frame_index)
                ) > 0
            );
        }
    }
}

#[test]
fn pigments_are_visible_in_every_loop_frame() {
    let development = read_development();
    let plain = without_languages();
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in 0..LOOP_FRAME_COUNT {
            let tinted = render_galaxy_frame(&development, Colorway::Blue, theme, frame_index);
            let untinted = render_galaxy_frame(&plain, Colorway::Blue, theme, frame_index);
            assert!(
                differing_pixels(&tinted, &untinted) > 0,
                "frame {frame_index} on {theme:?} has no pigment pixels"
            );
        }
    }
}

#[test]
fn the_same_snapshot_places_pigments_identically() {
    let development = read_development();
    let plain = without_languages();
    for theme in [Theme::Dark, Theme::Light] {
        assert_eq!(
            render_galaxy_still(&development, Colorway::Blue, theme),
            render_galaxy_still(&development, Colorway::Blue, theme)
        );
        let tinted_changed = changed_pixel_indices(
            &render_galaxy_still(&development, Colorway::Blue, theme),
            &render_galaxy_still(&plain, Colorway::Blue, theme),
        );
        let other_colorway_changed = changed_pixel_indices(
            &render_galaxy_still(&development, Colorway::Red, theme),
            &render_galaxy_still(&plain, Colorway::Red, theme),
        );
        assert_eq!(tinted_changed, other_colorway_changed);
    }
}

fn differing_pixels(earlier: &GalaxyFrame, later: &GalaxyFrame) -> usize {
    earlier
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(later.as_rgba_bytes().as_chunks::<4>().0)
        .filter(|(earlier_pixel, later_pixel)| earlier_pixel != later_pixel)
        .count()
}

fn changed_pixel_indices(earlier: &GalaxyFrame, later: &GalaxyFrame) -> Vec<usize> {
    earlier
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(later.as_rgba_bytes().as_chunks::<4>().0)
        .enumerate()
        .filter(|(_, (earlier_pixel, later_pixel))| earlier_pixel != later_pixel)
        .map(|(pixel_index, _)| pixel_index)
        .collect()
}

fn non_background_pixels(galaxy_frame: &GalaxyFrame, background: [u8; 4]) -> usize {
    opaque_pixels(galaxy_frame)
        .into_iter()
        .filter(|pixel| *pixel != background)
        .count()
}

fn opaque_pixels(galaxy_frame: &GalaxyFrame) -> Vec<[u8; 4]> {
    galaxy_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|channels| {
            let [red, green, blue, alpha] = channels;
            [*red, *green, *blue, *alpha]
        })
        .collect()
}
