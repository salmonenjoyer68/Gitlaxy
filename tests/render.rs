use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Days, NaiveDate};
use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{
    GalaxyDevelopment, MAX_GROWTH_CONTRIBUTIONS, RecentStreak, StreakDevelopment, calculate_stage,
    derive_galaxy_development,
};
use gitlaxy::history::{AccountSnapshot, ContributionWeek};
use gitlaxy::markings::{STAR_DENSITY_FULL_WEEKS, TRAIL_FULL_WEEKS};
use gitlaxy::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, render_galaxy_frame, render_galaxy_still,
};
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
    let mut other_development = derive_galaxy_development(&other_snapshot).unwrap();
    other_development.total_contributions = MAX_GROWTH_CONTRIBUTIONS;
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
    let galaxy_frame = render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark);
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
    let galaxy_frame = render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark);
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
    let mut previous_extent = 0;
    for (stage_index, total) in totals.into_iter().enumerate() {
        let galaxy_development = development_with_total(total);
        assert_eq!(calculate_stage(total), stage_index as u8 + 1);
        let galaxy_frame = render_galaxy_still(&galaxy_development, Colorway::Blue, Theme::Dark);
        let (left, top, right, bottom) = opaque_bounds(&galaxy_frame, Theme::Dark.background());
        let extent = (right - left + 1).max(bottom - top + 1);
        assert!(
            extent > previous_extent,
            "stage {} spans {extent} pixels, no more than the previous {previous_extent}",
            stage_index + 1
        );
        previous_extent = extent;
    }
}

#[test]
fn still_and_loop_frames_render_the_development_stage() {
    let spark_development = development_with_total(0);
    let full_development = development_with_total(MAX_GROWTH_CONTRIBUTIONS);
    for frame_index in [0, 1, 60, 119] {
        let spark_frame =
            render_galaxy_frame(&spark_development, Colorway::Blue, Theme::Dark, frame_index);
        let (left, _, right, _) = opaque_bounds(&spark_frame, Theme::Dark.background());
        assert!(
            right - left < 20,
            "frame {frame_index} of stage one is not a spark"
        );
        let full_frame =
            render_galaxy_frame(&full_development, Colorway::Blue, Theme::Dark, frame_index);
        let (left, _, right, _) = opaque_bounds(&full_frame, Theme::Dark.background());
        assert!(
            right - left + 1 >= 180,
            "frame {frame_index} of stage five is too small"
        );
    }
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

#[test]
fn a_current_streak_draws_a_longer_trail_in_stills_and_loop_frames() {
    let no_streak = development_with_streaks(0, 0);
    let short_streak = development_with_streaks(2, 0);
    let long_streak = development_with_streaks(TRAIL_FULL_WEEKS, 0);
    let capped_streak = development_with_streaks(TRAIL_FULL_WEEKS * 4, 0);
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in [0, 60] {
            let no_streak_frame =
                render_galaxy_frame(&no_streak, Colorway::Blue, theme, frame_index);
            let short_streak_frame =
                render_galaxy_frame(&short_streak, Colorway::Blue, theme, frame_index);
            let long_streak_frame =
                render_galaxy_frame(&long_streak, Colorway::Blue, theme, frame_index);
            let short_trail = differing_pixels(&no_streak_frame, &short_streak_frame);
            let long_trail = differing_pixels(&no_streak_frame, &long_streak_frame);
            assert!(
                short_trail > 0,
                "frame {frame_index} of a two-week streak shows no trail"
            );
            assert!(
                long_trail > short_trail,
                "frame {frame_index}: {long_trail} trail pixels are not more than {short_trail}"
            );
            let capped_streak_frame =
                render_galaxy_frame(&capped_streak, Colorway::Blue, theme, frame_index);
            assert_eq!(long_streak_frame, capped_streak_frame);
        }
    }
}

#[test]
fn a_longest_streak_raises_star_density_in_stills_and_loop_frames() {
    let short_record = development_with_streaks(0, 1);
    let long_record = development_with_streaks(0, STAR_DENSITY_FULL_WEEKS);
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in [0, 60] {
            let short_frame =
                render_galaxy_frame(&short_record, Colorway::Blue, theme, frame_index);
            let long_frame = render_galaxy_frame(&long_record, Colorway::Blue, theme, frame_index);
            assert_ne!(short_frame, long_frame);
            assert!(
                non_background_pixels(&long_frame, theme.background())
                    > non_background_pixels(&short_frame, theme.background()),
                "frame {frame_index} did not gain stars on {theme:?}"
            );
        }
    }
}

#[test]
fn a_streak_snapshot_renders_byte_identical_frames() {
    let development = development_with_streaks(5, TRAIL_FULL_WEEKS);
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in [0, 37] {
            assert_eq!(
                render_galaxy_frame(&development, Colorway::Blue, theme, frame_index)
                    .as_rgba_bytes(),
                render_galaxy_frame(&development, Colorway::Blue, theme, frame_index)
                    .as_rgba_bytes()
            );
        }
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

fn non_background_pixels(galaxy_frame: &GalaxyFrame, background: [u8; 4]) -> usize {
    opaque_pixels(galaxy_frame)
        .into_iter()
        .filter(|pixel| *pixel != background)
        .count()
}

fn differing_pixels(left: &GalaxyFrame, right: &GalaxyFrame) -> usize {
    left.as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(right.as_rgba_bytes().as_chunks::<4>().0)
        .filter(|(left_pixel, right_pixel)| left_pixel != right_pixel)
        .count()
}

fn development_with_streaks(current_weeks: usize, longest_weeks: usize) -> GalaxyDevelopment {
    let mut galaxy_development = development_with_total(MAX_GROWTH_CONTRIBUTIONS);
    galaxy_development.streaks = StreakDevelopment {
        longest: streak_weeks(longest_weeks),
        recent: (current_weeks > 0).then(|| RecentStreak::Current(streak_weeks(current_weeks))),
    };
    galaxy_development
}

fn streak_weeks(week_count: usize) -> Vec<ContributionWeek> {
    let first_monday: NaiveDate = "2025-01-06".parse().unwrap();
    (0..week_count)
        .map(|week_index| ContributionWeek {
            iso_week: first_monday
                .checked_add_days(Days::new(week_index as u64 * 7))
                .unwrap()
                .iso_week(),
            active_day_count: 1,
            observed_day_count: 7,
        })
        .collect()
}
