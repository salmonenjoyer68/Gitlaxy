use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use chrono::{Datelike, Days, NaiveDate};
use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{
    GalaxyDevelopment, RecentStreak, StreakDevelopment, calculate_stage, derive_galaxy_development,
};
use gitlaxy::history::{AccountSnapshot, ContributionWeek};
use gitlaxy::markings::{STAR_DENSITY_FULL_WEEKS, TRAIL_FULL_WEEKS, calculate_star_count};
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
fn the_blue_colorway_stays_dominant_behind_pigment_accents() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, theme);
        let background = theme.background();
        let mut galaxy_colors = BTreeSet::new();
        let mut blue_pixels = 0;
        let mut accent_pixels = 0;
        for pixel in opaque_pixels(&galaxy_frame) {
            if pixel == background {
                continue;
            }
            if pixel[2] >= pixel[1] && pixel[1] >= pixel[0] {
                blue_pixels += 1;
                galaxy_colors.insert(pixel);
            } else {
                accent_pixels += 1;
            }
        }
        assert!(galaxy_colors.len() > 10);
        assert!(
            blue_pixels > accent_pixels,
            "the blue ramp covers {blue_pixels} pixels but accents cover {accent_pixels}"
        );
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
    other_development.total_contributions = 1_500;
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
    // The spark is the nucleus plus the star splats seeded around it; the widest splat
    // reaches about 15 pixels across at this scale.
    assert!(
        width <= 16 && height <= 16,
        "stage one should be a spark, not a {width} by {height} galaxy"
    );
}

#[test]
fn the_last_stage_fills_about_eighty_five_percent_of_the_canvas() {
    let galaxy_development = development_with_total(1_500);
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
    let totals = [0, 250, 750, 1_250, 1_500];
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
    let full_development = development_with_total(1_500);
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

/// The broken spiral has three primary arms: across the middle of the disc the third
/// angular harmonic of the arm light outweighs the second.
#[test]
fn the_mid_disc_shows_three_primary_arms() {
    let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, Theme::Dark);
    let second = harmonic_amplitude(&galaxy_frame, 2.0);
    let third = harmonic_amplitude(&galaxy_frame, 3.0);
    assert!(
        third > second,
        "the third harmonic ({third}) is not stronger than the second ({second})"
    );
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

/// Most star light sits beyond the core, so a longest streak's added stars there must
/// gather into compact knots along the three arms rather than spread evenly or run
/// smoothly down each arm. Comparing two star counts isolates the stars from the disc,
/// which never changes between the two renders.
///
/// Two independent measurements pin that down. The angular contrast compares the added
/// light on the arms with the interarm gaps, and the support per star measures how much
/// frame area each added star lights: stars packed into knots overlap, while a smooth
/// arm or a uniform field spreads every star over its own patch.
#[test]
fn outer_star_knots_gather_along_the_three_arms() {
    const BIN_COUNT: usize = 36;
    const INNER_RADIUS: f64 = 40.0;
    const OUTER_RADIUS: f64 = 106.0;
    let short_record = development_with_streaks(0, 1);
    let long_record = development_with_streaks(0, STAR_DENSITY_FULL_WEEKS);
    let short_frame = render_galaxy_still(&short_record, Colorway::Blue, Theme::Dark);
    let long_frame = render_galaxy_still(&long_record, Colorway::Blue, Theme::Dark);
    let short_blue = blue_channel(&short_frame);
    let long_blue = blue_channel(&long_frame);
    let added_light: Vec<f64> = short_blue
        .iter()
        .zip(&long_blue)
        .map(|(short, long)| (long - short).max(0.0))
        .collect();
    let mut angular_light = [0.0; BIN_COUNT];
    for (pixel_index, light) in added_light.iter().enumerate() {
        let (radius, angle) = polar_position(pixel_index);
        if !(INNER_RADIUS..=OUTER_RADIUS).contains(&radius) {
            continue;
        }
        let bin = (angle.rem_euclid(TAU) / TAU * BIN_COUNT as f64) as usize % BIN_COUNT;
        angular_light[bin] += light;
    }
    let mut sorted_light = angular_light;
    sorted_light.sort_by(f64::total_cmp);
    let arm_mean = sorted_light[BIN_COUNT * 3 / 4..].iter().sum::<f64>() / (BIN_COUNT / 4) as f64;
    let gap_mean = sorted_light[..BIN_COUNT / 2].iter().sum::<f64>() / (BIN_COUNT / 2) as f64;
    assert!(
        arm_mean > gap_mean * 8.0,
        "the added stars are not gathered on the arms: arm bins average {arm_mean}, \
         interarm bins average {gap_mean}"
    );
    let total_light: f64 = added_light.iter().sum();
    assert!(total_light > 0.0, "the outer annulus gained no star light");
    let squared_light: f64 = added_light.iter().map(|light| light * light).sum();
    let added_stars =
        (calculate_star_count(STAR_DENSITY_FULL_WEEKS) - calculate_star_count(1)) as f64;
    // The participation ratio counts the effective pixels the added light occupies.
    let support_per_star = total_light * total_light / squared_light / added_stars;
    assert!(
        support_per_star < 3.2,
        "the added stars are spread thin rather than knotted: {support_per_star} pixels \
         of support per star"
    );
}

/// The comet trail must follow luminous arm material, never an interarm gap: its added
/// light sits on material far brighter than the median annulus pixel, and only a small
/// share of it crosses the dim sections of its own arm.
#[test]
fn the_comet_trail_stays_on_luminous_arm_material() {
    const BAND_INNER: f64 = 25.0;
    const BAND_OUTER: f64 = 110.0;
    const BLUR_RADIUS: i64 = 5;
    let no_trail = development_with_streaks(0, 0);
    let full_trail = development_with_streaks(TRAIL_FULL_WEEKS, 0);
    let base_blue = blue_channel(&render_galaxy_still(&no_trail, Colorway::Blue, Theme::Dark));
    let trailed_blue = blue_channel(&render_galaxy_still(
        &full_trail,
        Colorway::Blue,
        Theme::Dark,
    ));
    let added_light: Vec<f64> = base_blue
        .iter()
        .zip(&trailed_blue)
        .map(|(base, trailed)| (trailed - base).max(0.0))
        .collect();
    // A wide box average of the untouched frame leaves the broad arm material, which the
    // narrow trail light must sit on.
    let mut neighbourhood = vec![0.0; base_blue.len()];
    for pixel_y in 0..i64::from(FRAME_HEIGHT_PIXELS) {
        for pixel_x in 0..i64::from(FRAME_WIDTH_PIXELS) {
            let mut sum = 0.0;
            let mut count = 0.0;
            for blur_y in -BLUR_RADIUS..=BLUR_RADIUS {
                for blur_x in -BLUR_RADIUS..=BLUR_RADIUS {
                    let sample_x = (pixel_x + blur_x).clamp(0, i64::from(FRAME_WIDTH_PIXELS) - 1);
                    let sample_y = (pixel_y + blur_y).clamp(0, i64::from(FRAME_HEIGHT_PIXELS) - 1);
                    sum +=
                        base_blue[(sample_y * i64::from(FRAME_WIDTH_PIXELS) + sample_x) as usize];
                    count += 1.0;
                }
            }
            neighbourhood[(pixel_y * i64::from(FRAME_WIDTH_PIXELS) + pixel_x) as usize] =
                sum / count;
        }
    }
    let mut annulus_light = Vec::new();
    let (mut added_total, mut added_on_material) = (0.0, 0.0);
    for (pixel_index, light) in added_light.iter().enumerate() {
        let (radius, _) = polar_position(pixel_index);
        if !(BAND_INNER..=BAND_OUTER).contains(&radius) {
            continue;
        }
        annulus_light.push(neighbourhood[pixel_index]);
        added_total += light;
        added_on_material += light * neighbourhood[pixel_index];
    }
    annulus_light.sort_by(f64::total_cmp);
    let annulus_mean = annulus_light.iter().sum::<f64>() / annulus_light.len() as f64;
    let annulus_median = annulus_light[annulus_light.len() / 2];
    let trail_material = added_on_material / added_total;
    assert!(added_total > 0.0, "the running streak added no trail light");
    // The annulus mixes bright arms with dark gaps, so its median marks interarm
    // material: a trail drawn across a gap would average near it.
    assert!(
        trail_material > annulus_median * 2.0,
        "the trail lights dim material: {trail_material:.1} under the trail against an \
         annulus median of {annulus_median:.1} and mean of {annulus_mean:.1}"
    );
    let mut added_below_median = 0.0;
    for (pixel_index, light) in added_light.iter().enumerate() {
        if neighbourhood[pixel_index] < annulus_median {
            added_below_median += light;
        }
    }
    assert!(
        added_below_median / added_total < 0.3,
        "{:.0}% of the trail light falls on material below the annulus median",
        added_below_median / added_total * 100.0
    );
}

/// The nucleus stays a compact, intense core rather than a soft blob, so the inner arms
/// stay readable beside it.
#[test]
fn the_nucleus_stays_a_compact_core() {
    for theme in [Theme::Dark, Theme::Light] {
        let galaxy_frame = render_galaxy_still(&read_development(), Colorway::Blue, theme);
        let pixels = opaque_pixels(&galaxy_frame);
        let background_blue = f64::from(theme.background()[2]);
        let brightness = |pixel: [u8; 4]| (f64::from(pixel[2]) - background_blue).abs();
        let brightest = pixels
            .iter()
            .map(|pixel| brightness(*pixel))
            .fold(0.0, f64::max);
        let core_pixels = pixels
            .iter()
            .enumerate()
            .filter(|(pixel_index, pixel)| {
                polar_position(*pixel_index).0 <= 16.0 && brightness(**pixel) >= brightest * 0.9
            })
            .count();
        assert!(
            core_pixels > 0,
            "the galaxy has no bright core on {theme:?}"
        );
        assert!(
            core_pixels < 100,
            "the nucleus spreads over {core_pixels} near-brightest pixels on {theme:?}"
        );
    }
}

/// The blue channel of every pixel, the channel a dark-theme galaxy brightens.
fn blue_channel(galaxy_frame: &GalaxyFrame) -> Vec<f64> {
    galaxy_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|channels| f64::from(channels[2]))
        .collect()
}

/// One pixel's distance from the canvas center and its angle around it.
fn polar_position(pixel_index: usize) -> (f64, f64) {
    let center = f64::from(FRAME_WIDTH_PIXELS) / 2.0;
    let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - center;
    let offset_y = (pixel_index as u32 / FRAME_WIDTH_PIXELS) as f64 - center;
    (offset_x.hypot(offset_y), offset_y.atan2(offset_x))
}

/// How much light the frame's mid-disc annulus holds in one angular harmonic.
fn harmonic_amplitude(galaxy_frame: &GalaxyFrame, harmonic: f64) -> f64 {
    let (mut sum_x, mut sum_y) = (0.0, 0.0);
    for (pixel_index, pixel) in opaque_pixels(galaxy_frame).iter().enumerate() {
        let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - 128.0;
        let offset_y = (pixel_index as u32 / FRAME_WIDTH_PIXELS) as f64 - 128.0;
        let radius = offset_x.hypot(offset_y);
        if !(30.0..=62.0).contains(&radius) {
            continue;
        }
        let weight = f64::from(pixel[2].saturating_sub(0x17));
        let angle = harmonic * offset_y.atan2(offset_x);
        sum_x += weight * angle.cos();
        sum_y += weight * angle.sin();
    }
    sum_x.hypot(sum_y)
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
    development_with_total(1_500)
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
    let mut galaxy_development = development_with_total(1_500);
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
