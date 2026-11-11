use std::collections::{BTreeMap, BTreeSet};

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{GalaxyDevelopment, derive_galaxy_development};
use gitlaxy::history::AccountSnapshot;
use gitlaxy::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, render_galaxy_still};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");

#[test]
fn the_still_is_fully_opaque_on_the_exact_theme_background() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, theme);
        let pixels = opaque_pixels(&galaxy_frame);
        assert_eq!(
            pixels.len(),
            (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize
        );
        assert_eq!(pixels[0], theme.background());
        assert!(
            pixels
                .iter()
                .filter(|pixel| **pixel != theme.background())
                .count()
                > 1_000
        );
    }
}

#[test]
fn both_themes_draw_a_blue_galaxy_with_no_other_hue() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, theme);
        let background = theme.background();
        let mut galaxy_colors = BTreeSet::new();
        for pixel in opaque_pixels(&galaxy_frame) {
            if pixel == background {
                continue;
            }
            assert!(
                pixel[2] >= pixel[1] && pixel[1] >= pixel[0],
                "{pixel:?} is not blue"
            );
            galaxy_colors.insert(pixel);
        }
        assert!(galaxy_colors.len() > 10);
    }
}

#[test]
fn the_same_development_and_theme_render_identical_pixels() {
    let galaxy_development = read_development();
    for theme in [Theme::Dark, Theme::Light] {
        assert_eq!(
            render_galaxy_still(&galaxy_development, Colorway::Blue, theme),
            render_galaxy_still(&galaxy_development, Colorway::Blue, theme)
        );
    }
    assert_ne!(
        render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark),
        render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Light)
    );
    let mut other_snapshot = read_snapshot();
    other_snapshot.account_id += 1;
    let other_development = derive_galaxy_development(&other_snapshot).unwrap();
    assert_ne!(
        render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark),
        render_galaxy_still(&other_development, Colorway::Blue, Theme::Dark)
    );
}

#[test]
fn ordered_dithering_stays_under_the_gif_palette_ceiling() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, theme);
        let colors: BTreeSet<[u8; 4]> = opaque_pixels(&galaxy_frame).into_iter().collect();
        assert!(colors.len() <= 255, "{} colors", colors.len());
    }
}

#[test]
fn the_galaxy_is_centered_with_a_clear_margin() {
    let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, Theme::Dark);
    let background = Theme::Dark.background();
    let (mut left, mut top, mut right, mut bottom) =
        (FRAME_WIDTH_PIXELS, FRAME_HEIGHT_PIXELS, 0, 0);
    for (pixel_index, pixel) in opaque_pixels(&galaxy_frame).into_iter().enumerate() {
        if pixel == background {
            continue;
        }
        let pixel_x = pixel_index as u32 % FRAME_WIDTH_PIXELS;
        let pixel_y = pixel_index as u32 / FRAME_WIDTH_PIXELS;
        left = left.min(pixel_x);
        top = top.min(pixel_y);
        right = right.max(pixel_x);
        bottom = bottom.max(pixel_y);
    }
    assert!(left > 5 && top > 5);
    assert!(right < FRAME_WIDTH_PIXELS - 6 && bottom < FRAME_HEIGHT_PIXELS - 6);
    assert!(right - left < FRAME_WIDTH_PIXELS * 9 / 10);
    assert!(((left + right) / 2).abs_diff(FRAME_WIDTH_PIXELS / 2) < 10);
    assert!(((top + bottom) / 2).abs_diff(FRAME_HEIGHT_PIXELS / 2) < 10);
}

#[test]
fn contribution_totals_do_not_change_the_fixed_stage_still() {
    let original_snapshot = read_snapshot();
    let busy_snapshot = {
        let mut snapshot = original_snapshot.clone();
        let history = snapshot.contribution_history.as_mut().unwrap();
        history.days.last_mut().unwrap().contribution_count += 5_000;
        snapshot
    };
    assert_ne!(
        derive_galaxy_development(&original_snapshot)
            .unwrap()
            .total_contributions,
        derive_galaxy_development(&busy_snapshot)
            .unwrap()
            .total_contributions
    );
    assert_eq!(
        render_galaxy_still(
            &derive_galaxy_development(&original_snapshot).unwrap(),
            Colorway::Blue,
            Theme::Dark
        ),
        render_galaxy_still(
            &derive_galaxy_development(&busy_snapshot).unwrap(),
            Colorway::Blue,
            Theme::Dark
        )
    );
}

#[test]
fn each_colorway_draws_a_distinct_dominant_palette_in_both_themes() {
    let galaxy_development = read_development();
    for theme in [Theme::Dark, Theme::Light] {
        let mut dominants = BTreeSet::new();
        for colorway in Colorway::ALL {
            let galaxy_frame = render_galaxy_still(&galaxy_development, colorway, theme);
            let background = theme.background();
            let mut counts = BTreeMap::new();
            for pixel in opaque_pixels(&galaxy_frame) {
                if pixel_distance(pixel, background) > 24 {
                    *counts.entry(pixel).or_insert(0usize) += 1;
                }
            }
            let (&dominant, &count) = counts
                .iter()
                .max_by_key(|(_, count)| **count)
                .expect("a galaxy has many non-background pixels");
            assert!(
                count > 100,
                "{} on {theme:?} has too few dominant pixels",
                colorway.name()
            );
            assert!(
                dominants.insert(dominant),
                "{} shares its dominant {dominant:?} on {theme:?}",
                colorway.name()
            );
        }
        assert_eq!(dominants.len(), Colorway::ALL.len());
    }
}

fn pixel_distance(left: [u8; 4], right: [u8; 4]) -> u32 {
    left.into_iter()
        .zip(right)
        .map(|(left_channel, right_channel)| u32::from(left_channel.abs_diff(right_channel)).pow(2))
        .sum()
}

fn read_snapshot() -> AccountSnapshot {
    serde_json::from_str(HISTORY_FIXTURE).unwrap()
}

fn read_development() -> GalaxyDevelopment {
    derive_galaxy_development(&read_snapshot()).unwrap()
}

fn opaque_pixels(galaxy_frame: &GalaxyFrame) -> Vec<[u8; 4]> {
    let bytes = galaxy_frame.as_rgba_bytes();
    assert_eq!(
        bytes.len(),
        (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS * 4) as usize
    );
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|channels| {
            let [red, green, blue, alpha] = channels;
            assert_eq!(*alpha, 255, "galaxy frames are fully opaque");
            [*red, *green, *blue, *alpha]
        })
        .collect()
}
