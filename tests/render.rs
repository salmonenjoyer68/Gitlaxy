use std::collections::BTreeSet;

use gitlaxy::galaxy::{
    GalaxyDevelopment, MAX_GROWTH_CONTRIBUTIONS, calculate_stage, derive_galaxy_development,
};
use gitlaxy::history::AccountSnapshot;
use gitlaxy::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, render_galaxy_frame, render_galaxy_still,
};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");

#[test]
fn the_still_is_fully_opaque_on_the_exact_theme_background() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), theme);
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
        let galaxy_frame = render_galaxy_still(&read_development(), theme);
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
            render_galaxy_still(&galaxy_development, theme),
            render_galaxy_still(&galaxy_development, theme)
        );
    }
    assert_ne!(
        render_galaxy_still(&galaxy_development, Theme::Dark),
        render_galaxy_still(&galaxy_development, Theme::Light)
    );
    let mut other_snapshot = read_snapshot();
    other_snapshot.account_id += 1;
    let mut other_development = derive_galaxy_development(&other_snapshot).unwrap();
    other_development.total_contributions = MAX_GROWTH_CONTRIBUTIONS;
    assert_ne!(
        render_galaxy_still(&galaxy_development, Theme::Dark),
        render_galaxy_still(&other_development, Theme::Dark)
    );
}

#[test]
fn ordered_dithering_stays_under_the_gif_palette_ceiling() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), theme);
        let colors: BTreeSet<[u8; 4]> = opaque_pixels(&galaxy_frame).into_iter().collect();
        assert!(colors.len() <= 255, "{} colors", colors.len());
    }
}

#[test]
fn the_galaxy_is_centered_with_a_clear_margin() {
    let galaxy_frame = render_galaxy_still(&read_development(), Theme::Dark);
    let (left, top, right, bottom) = opaque_bounds(&galaxy_frame, Theme::Dark.background());
    assert!(left > 5 && top > 5);
    assert!(right < FRAME_WIDTH_PIXELS - 6 && bottom < FRAME_HEIGHT_PIXELS - 6);
    assert!(right - left < FRAME_WIDTH_PIXELS * 9 / 10);
    assert!(((left + right) / 2).abs_diff(FRAME_WIDTH_PIXELS / 2) < 10);
    assert!(((top + bottom) / 2).abs_diff(FRAME_HEIGHT_PIXELS / 2) < 10);
}

#[test]
fn the_first_stage_is_a_single_spark() {
    let galaxy_development = development_with_total(0);
    assert_eq!(calculate_stage(galaxy_development.total_contributions), 1);
    let galaxy_frame = render_galaxy_still(&galaxy_development, Theme::Dark);
    let (left, top, right, bottom) = opaque_bounds(&galaxy_frame, Theme::Dark.background());
    let width = right - left + 1;
    let height = bottom - top + 1;
    assert!(
        width >= 3 && height >= 3,
        "the spark is {width} by {height} pixels"
    );
    assert!(
        width <= 16 && height <= 16,
        "stage one should be a spark, not a {width} by {height} galaxy"
    );
}

#[test]
fn the_last_stage_fills_about_eighty_five_percent_of_the_canvas() {
    let galaxy_development = development_with_total(MAX_GROWTH_CONTRIBUTIONS);
    assert_eq!(calculate_stage(galaxy_development.total_contributions), 5);
    let galaxy_frame = render_galaxy_still(&galaxy_development, Theme::Dark);
    let (left, _, right, _) = opaque_bounds(&galaxy_frame, Theme::Dark.background());
    let expected_width = f64::from(FRAME_WIDTH_PIXELS) * 0.85;
    let width = f64::from(right - left + 1);
    assert!(
        (width - expected_width).abs() <= expected_width * 0.1,
        "stage five is {width} pixels wide, not about {expected_width}"
    );
}

#[test]
fn increasing_totals_grow_the_galaxy_through_every_stage() {
    let totals = [0, 179, 996, 4_734, MAX_GROWTH_CONTRIBUTIONS];
    let mut previous_width = 0;
    for (stage_index, total) in totals.into_iter().enumerate() {
        let galaxy_development = development_with_total(total);
        assert_eq!(calculate_stage(total), stage_index as u8 + 1);
        let galaxy_frame = render_galaxy_still(&galaxy_development, Theme::Dark);
        let (left, _, right, _) = opaque_bounds(&galaxy_frame, Theme::Dark.background());
        let width = right - left + 1;
        assert!(
            width > previous_width,
            "stage {} is {width} pixels wide, no wider than the previous {previous_width}",
            stage_index + 1
        );
        previous_width = width;
    }
}

#[test]
fn still_and_loop_frames_render_the_development_stage() {
    let spark_development = development_with_total(0);
    let full_development = development_with_total(MAX_GROWTH_CONTRIBUTIONS);
    for frame_index in [0, 1, 60, 119] {
        let spark_frame = render_galaxy_frame(&spark_development, Theme::Dark, frame_index);
        let (left, _, right, _) = opaque_bounds(&spark_frame, Theme::Dark.background());
        assert!(
            right - left < 20,
            "frame {frame_index} of stage one is not a spark"
        );
        let full_frame = render_galaxy_frame(&full_development, Theme::Dark, frame_index);
        let (left, _, right, _) = opaque_bounds(&full_frame, Theme::Dark.background());
        assert!(
            right - left + 1 >= 180,
            "frame {frame_index} of stage five is too small"
        );
    }
}

fn read_snapshot() -> AccountSnapshot {
    serde_json::from_str(HISTORY_FIXTURE).unwrap()
}

/// The fixture's own 14 contributions are stage one; appearance tests want a full galaxy.
fn read_development() -> GalaxyDevelopment {
    development_with_total(MAX_GROWTH_CONTRIBUTIONS)
}

fn development_with_total(total_contributions: u64) -> GalaxyDevelopment {
    let mut galaxy_development = derive_galaxy_development(&read_snapshot()).unwrap();
    galaxy_development.total_contributions = total_contributions;
    galaxy_development
}

/// The inclusive left, top, right, and bottom of pixels that differ from the background.
fn opaque_bounds(galaxy_frame: &GalaxyFrame, background: [u8; 4]) -> (u32, u32, u32, u32) {
    let (mut left, mut top, mut right, mut bottom) =
        (FRAME_WIDTH_PIXELS, FRAME_HEIGHT_PIXELS, 0, 0);
    for (pixel_index, pixel) in opaque_pixels(galaxy_frame).into_iter().enumerate() {
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
    (left, top, right, bottom)
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
