use std::collections::BTreeSet;
use std::f64::consts::TAU;
use std::sync::LazyLock;

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{GalaxyDevelopment, derive_galaxy_development};
use gitlaxy::history::AccountSnapshot;
use gitlaxy::motion::{
    frame_rotation, mote_brightness, mote_count, mote_phase, mote_radius_share, twinkle_scale,
};
use gitlaxy::motion_state::MotionState;
use gitlaxy::render::{
    FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, GalaxyFrame, LOOP_FRAME_COUNT, render_galaxy_frame,
    render_galaxy_still,
};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");
const PIXEL_COUNT: usize = (FRAME_WIDTH_PIXELS * FRAME_HEIGHT_PIXELS) as usize;

/// The still is the base pose of the loop, pixel for pixel, in both themes.
#[test]
fn the_still_is_exactly_the_first_loop_frame() {
    let galaxy_development = read_development();
    for theme in [Theme::Dark, Theme::Light] {
        assert_eq!(
            render_galaxy_still(&galaxy_development, Colorway::Blue, theme),
            render_galaxy_frame(&galaxy_development, Colorway::Blue, theme, 0)
        );
    }
}

/// The analytically next frame after the last one is the first frame again: rotation
/// and twinkle both close without a jump.
#[test]
fn the_analytic_next_frame_wraps_to_the_first() {
    let galaxy_development = read_development();
    assert_eq!(
        render_galaxy_frame(
            &galaxy_development,
            Colorway::Blue,
            Theme::Dark,
            LOOP_FRAME_COUNT
        ),
        render_galaxy_frame(&galaxy_development, Colorway::Blue, Theme::Dark, 0)
    );
    assert_eq!(
        frame_rotation(LOOP_FRAME_COUNT).to_bits(),
        frame_rotation(0).to_bits()
    );
    assert_eq!(
        twinkle_scale(
            galaxy_development.identity_seed,
            0,
            LOOP_FRAME_COUNT,
            MotionState::Active
        )
        .to_bits(),
        twinkle_scale(galaxy_development.identity_seed, 0, 0, MotionState::Active).to_bits()
    );
}

/// Every frame stays inside one GIF palette; the encoder still fails loudly on overflow.
#[test]
fn every_loop_frame_stays_under_the_gif_palette_ceiling() {
    let galaxy_development = read_development();
    for theme in [Theme::Dark, Theme::Light] {
        for frame_index in 0..LOOP_FRAME_COUNT {
            let colors: BTreeSet<[u8; 4]> =
                render_galaxy_frame(&galaxy_development, Colorway::Blue, theme, frame_index)
                    .as_rgba_bytes()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .copied()
                    .collect();
            assert!(
                colors.len() <= 255,
                "frame {frame_index} of {theme:?} has {} colors",
                colors.len()
            );
        }
    }
}

/// The largest change between neighbours bounds the loop seam, so playback never jumps.
#[test]
fn the_loop_seam_is_no_larger_than_an_adjacent_frame_change() {
    let changes: Vec<usize> = (0..LOOP_FRAME_COUNT as usize)
        .map(|frame_index| {
            differing_pixels(
                &DARK_FRAMES[frame_index],
                &DARK_FRAMES[(frame_index + 1) % LOOP_FRAME_COUNT as usize],
            )
        })
        .collect();
    let largest_adjacent = changes[..changes.len() - 1].iter().copied().max().unwrap();
    let seam = changes[changes.len() - 1];
    assert!(
        seam <= largest_adjacent,
        "the seam changes {seam} pixels, more than the largest adjacent change {largest_adjacent}"
    );
    assert!(
        largest_adjacent < PIXEL_COUNT / 4,
        "an adjacent frame change of {largest_adjacent} pixels is too large"
    );
    assert!(
        changes.iter().all(|&change| change > 100),
        "frames should visibly move: {changes:?}"
    );
}

/// The three-armed pattern turns clockwise, one whole turn per loop. The third angular
/// harmonic of the arm brightness advances by the rotation angle each frame.
#[test]
fn the_galaxy_turns_clockwise_through_one_turn_per_loop() {
    let arm_phases: Vec<f64> = DARK_FRAMES.iter().map(measured_arm_phase).collect();
    let expected_step = frame_rotation(1) - frame_rotation(0);
    let mut total_step = 0.0;
    for adjacent_pair in arm_phases.windows(2) {
        let mut step = adjacent_pair[1] - adjacent_pair[0];
        // A three-armed pattern repeats every TAU / 3, so fold the step into that span.
        while step <= -TAU / 6.0 {
            step += TAU / 3.0;
        }
        while step > TAU / 6.0 {
            step -= TAU / 3.0;
        }
        assert!(
            step > 0.0,
            "the galaxy turned counter-clockwise between frames: {step} radians"
        );
        total_step += step;
    }
    assert!(
        (total_step - TAU).abs() < 0.2,
        "the arms advanced {total_step} radians over the loop, not one full turn"
    );
    assert!(
        (expected_step - TAU / f64::from(LOOP_FRAME_COUNT)).abs() < 1e-12,
        "one loop must be exactly one turn"
    );
}

/// Twinkle varies a star's brightness across the loop and returns exactly at its end,
/// while the star's position is unchanged.
#[test]
fn twinkle_varies_and_returns_without_moving_stars() {
    let galaxy_development = read_development();
    let identity_seed = galaxy_development.identity_seed;
    let brightnesses: Vec<f64> = (0..=LOOP_FRAME_COUNT)
        .map(|frame_index| twinkle_scale(identity_seed, 7, frame_index, MotionState::Active))
        .collect();
    let darkest = brightnesses.iter().copied().fold(f64::INFINITY, f64::min);
    let brightest = brightnesses
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        brightest - darkest > 0.1,
        "star brightness barely changes: {darkest}..{brightest}"
    );
    assert_eq!(brightnesses[LOOP_FRAME_COUNT as usize], brightnesses[0]);

    let painted_brightness: Vec<u64> = DARK_FRAMES.iter().map(total_blue).collect();
    let darkest = *painted_brightness.iter().min().unwrap();
    let brightest = *painted_brightness.iter().max().unwrap();
    assert!(
        brightest - darkest > 2_000,
        "twinkle is invisible in the rendered frames: {darkest}..{brightest}"
    );
}

/// Dust motes fall from the rim to the core once per loop, invisible at both ends, and
/// the phase wraps exactly onto the next loop.
#[test]
fn dust_motes_fall_from_the_rim_to_the_core_each_loop() {
    assert_eq!(
        mote_phase(LOOP_FRAME_COUNT, 0.3).to_bits(),
        mote_phase(0, 0.3).to_bits()
    );
    assert_eq!(mote_phase(LOOP_FRAME_COUNT / 2, 0.0), 0.5);
    assert_eq!(mote_radius_share(0.0), 1.0);
    assert_eq!(mote_radius_share(1.0), 0.0);
    assert_eq!(mote_brightness(0.0), 0.0);
    assert!(mote_brightness(1.0) < 1e-12);
    assert!(mote_brightness(0.25) < mote_brightness(0.5));
    assert!(mote_brightness(0.75) < mote_brightness(0.5));
    assert!(mote_count(MotionState::Active) > mote_count(MotionState::Quiet));
}

static DARK_FRAMES: LazyLock<Vec<GalaxyFrame>> = LazyLock::new(|| {
    let galaxy_development = read_development();
    (0..LOOP_FRAME_COUNT)
        .map(|frame_index| {
            render_galaxy_frame(
                &galaxy_development,
                Colorway::Blue,
                Theme::Dark,
                frame_index,
            )
        })
        .collect()
});

fn read_development() -> GalaxyDevelopment {
    let account_snapshot: AccountSnapshot = serde_json::from_str(HISTORY_FIXTURE).unwrap();
    let mut galaxy_development = derive_galaxy_development(&account_snapshot).unwrap();
    // The fixture's own total is stage one; the loop tests need a full spiral.
    galaxy_development.total_contributions = 1_500;
    // Loop tests measure the active loop; the quiet state is covered separately.
    galaxy_development.recent_activity.quiet_day_count = 0;
    galaxy_development
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

/// The arm angle from the third angular harmonic of brightness in an annulus, which
/// keeps the core and the frame edge from dominating the measurement.
fn measured_arm_phase(galaxy_frame: &GalaxyFrame) -> f64 {
    let mut harmonic_x = 0.0;
    let mut harmonic_y = 0.0;
    for (pixel_index, [_, _, blue, _]) in galaxy_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
    {
        let offset_x = (pixel_index as u32 % FRAME_WIDTH_PIXELS) as f64 - 128.0;
        let offset_y = (pixel_index as u32 / FRAME_WIDTH_PIXELS) as f64 - 128.0;
        let radius = offset_x.hypot(offset_y);
        if !(28.0..=72.0).contains(&radius) {
            continue;
        }
        let weight = f64::from(blue.saturating_sub(0x17));
        let tripled_angle = 3.0 * offset_y.atan2(offset_x);
        harmonic_x += weight * tripled_angle.cos();
        harmonic_y += weight * tripled_angle.sin();
    }
    harmonic_y.atan2(harmonic_x) / 3.0
}

fn total_blue(galaxy_frame: &GalaxyFrame) -> u64 {
    galaxy_frame
        .as_rgba_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|[_, _, blue, _]| u64::from(*blue))
        .sum()
}
