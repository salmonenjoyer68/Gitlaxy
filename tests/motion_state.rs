use std::sync::LazyLock;

use gitlaxy::colorway::Colorway;
use gitlaxy::galaxy::{GalaxyDevelopment, MAX_GROWTH_CONTRIBUTIONS, derive_galaxy_development};
use gitlaxy::history::AccountSnapshot;
use gitlaxy::motion::{flare_frames, flare_intensity, twinkle_scale};
use gitlaxy::motion_state::{MotionState, select_motion_state};
use gitlaxy::render::{GalaxyFrame, LOOP_FRAME_COUNT, render_galaxy_frame, render_galaxy_still};
use gitlaxy::theme::Theme;

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");
const FLARE_HALF_WIDTH: u32 = 3;

/// The 13/14-day boundary is the whole decision: thirteen quiet days is still active.
#[test]
fn fourteen_quiet_days_select_quiet_and_thirteen_select_active() {
    assert_eq!(
        select_motion_state(&read_development(13)),
        MotionState::Active
    );
    assert_eq!(
        select_motion_state(&read_development(14)),
        MotionState::Quiet
    );
    assert_eq!(
        select_motion_state(&read_development(0)),
        MotionState::Active
    );
    assert_eq!(
        select_motion_state(&read_development(u32::MAX)),
        MotionState::Quiet
    );
}

/// Quiet is dimmer than Active at every sampled frame, and sparser in twinkling stars.
#[test]
fn quiet_is_dimmer_and_twinkles_sparsely() {
    let active = read_development(0);
    let quiet = read_development(14);
    for frame_index in [0, 17, 42, 76, 101] {
        let active_brightness = total_blue(&render(&active, frame_index));
        let quiet_brightness = total_blue(&render(&quiet, frame_index));
        assert!(
            quiet_brightness * 10 < active_brightness * 9,
            "quiet frame {frame_index} is not measurably dimmer: {quiet_brightness} vs {active_brightness}"
        );
    }

    let twinkling = |state: MotionState| {
        (0..64)
            .filter(|&star_index| {
                let brightnesses: Vec<f64> = (0..LOOP_FRAME_COUNT)
                    .map(|frame_index| {
                        twinkle_scale(active.identity_seed, star_index, frame_index, state)
                    })
                    .collect();
                let brightest = brightnesses
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max);
                let darkest = brightnesses.iter().copied().fold(f64::INFINITY, f64::min);
                brightest - darkest > 1e-9
            })
            .count()
    };
    let active_twinkle = twinkling(MotionState::Active);
    let quiet_twinkle = twinkling(MotionState::Quiet);
    assert!(
        quiet_twinkle * 2 < active_twinkle,
        "quiet twinkle is not sparser: {quiet_twinkle} of {active_twinkle} stars"
    );
}

/// Active has exactly two seeded flares per loop, and each seed chooses its own phases.
#[test]
fn active_has_exactly_two_flare_frames_per_loop() {
    for identity_seed in [7, 42, 0xdead_beef, u64::MAX] {
        let flares = flare_frames(identity_seed);
        assert_ne!(flares[0], flares[1], "the two flares coincide");

        let peaks: Vec<u32> = (0..LOOP_FRAME_COUNT)
            .filter(|&frame_index| {
                let intensity = flare_intensity(identity_seed, frame_index, MotionState::Active);
                intensity > 0.0
                    && intensity
                        > flare_intensity(
                            identity_seed,
                            (frame_index + 1) % LOOP_FRAME_COUNT,
                            MotionState::Active,
                        )
                    && intensity
                        > flare_intensity(
                            identity_seed,
                            (frame_index + LOOP_FRAME_COUNT - 1) % LOOP_FRAME_COUNT,
                            MotionState::Active,
                        )
            })
            .collect();
        assert_eq!(
            peaks, flares,
            "flare peaks are not exactly the flare frames"
        );
        assert!(
            (0..LOOP_FRAME_COUNT).all(|frame_index| flare_intensity(
                identity_seed,
                frame_index,
                MotionState::Quiet
            ) == 0.0),
            "quiet must not flare"
        );
    }
}

/// The rendered brightness peaks where the analytic flares say it should.
#[test]
fn rendered_active_frames_peak_at_the_flare_frames() {
    let active = read_development(0);
    let brightness: Vec<u64> = (0..LOOP_FRAME_COUNT)
        .map(|frame_index| total_blue(&ACTIVE_FRAMES[frame_index as usize]))
        .collect();
    let flares = flare_frames(active.identity_seed);
    for &flare in &flares {
        for (frame_index, &value) in brightness.iter().enumerate() {
            let outside_every_pulse = flares.iter().all(|&other| {
                circular_frame_distance(frame_index as u32, other) > FLARE_HALF_WIDTH
            });
            if outside_every_pulse {
                assert!(
                    brightness[flare as usize] > value,
                    "flare frame {flare} is dimmer than frame {frame_index}"
                );
            }
        }
        let neighbours = [
            (flare + LOOP_FRAME_COUNT - 1) % LOOP_FRAME_COUNT,
            (flare + 1) % LOOP_FRAME_COUNT,
        ];
        for neighbour in neighbours {
            assert!(
                brightness[flare as usize] > brightness[neighbour as usize],
                "frame {flare} is not a brightness peak against frame {neighbour}"
            );
        }
    }
}

/// Both states close the loop exactly and repeat byte for byte.
#[test]
fn both_states_are_seamless_and_deterministic() {
    for quiet_day_count in [0, 14] {
        let development = read_development(quiet_day_count);
        let state = select_motion_state(&development);
        assert_eq!(
            render_galaxy_frame(&development, Colorway::Blue, Theme::Dark, LOOP_FRAME_COUNT),
            render_galaxy_frame(&development, Colorway::Blue, Theme::Dark, 0),
            "state {state:?} jumps at the loop seam"
        );
        assert_eq!(
            render_galaxy_frame(&development, Colorway::Blue, Theme::Dark, 37),
            render_galaxy_frame(&development, Colorway::Blue, Theme::Dark, 37),
            "state {state:?} is not deterministic"
        );
        let identity_seed = development.identity_seed;
        assert_eq!(
            flare_intensity(identity_seed, LOOP_FRAME_COUNT, state).to_bits(),
            flare_intensity(identity_seed, 0, state).to_bits()
        );
        assert_eq!(
            twinkle_scale(identity_seed, 3, LOOP_FRAME_COUNT, state).to_bits(),
            twinkle_scale(identity_seed, 3, 0, state).to_bits()
        );
    }
}

/// The still stays exactly loop frame zero under either motion state.
#[test]
fn the_still_is_loop_frame_zero_in_both_states() {
    for quiet_day_count in [0, 13, 14] {
        let development = read_development(quiet_day_count);
        for theme in [Theme::Dark, Theme::Light] {
            assert_eq!(
                render_galaxy_still(&development, Colorway::Blue, theme),
                render_galaxy_frame(&development, Colorway::Blue, theme, 0)
            );
        }
    }
}

static ACTIVE_FRAMES: LazyLock<Vec<GalaxyFrame>> = LazyLock::new(|| {
    let development = read_development(0);
    (0..LOOP_FRAME_COUNT)
        .map(|frame_index| render(&development, frame_index))
        .collect()
});

fn render(development: &GalaxyDevelopment, frame_index: u32) -> GalaxyFrame {
    render_galaxy_frame(development, Colorway::Blue, Theme::Dark, frame_index)
}

fn read_development(quiet_day_count: u32) -> GalaxyDevelopment {
    let account_snapshot: AccountSnapshot = serde_json::from_str(HISTORY_FIXTURE).unwrap();
    let mut development = derive_galaxy_development(&account_snapshot).unwrap();
    // The fixture's own total is stage one; appearance tests want a full galaxy.
    development.total_contributions = MAX_GROWTH_CONTRIBUTIONS;
    development.recent_activity.quiet_day_count = quiet_day_count;
    development
}

fn circular_frame_distance(from: u32, to: u32) -> u32 {
    let forward = (to + LOOP_FRAME_COUNT - from) % LOOP_FRAME_COUNT;
    forward.min(LOOP_FRAME_COUNT - forward)
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
