use chrono::{Datelike, Days, NaiveDate};
use gitlaxy::galaxy::{
    GalaxyDevelopment, RecentStreak, StreakDevelopment, derive_galaxy_development,
};
use gitlaxy::history::{AccountSnapshot, ContributionWeek};
use gitlaxy::markings::{
    BASE_STAR_COUNT, EXTRA_STAR_COUNT, STAR_DENSITY_FULL_WEEKS, TRAIL_FULL_WEEKS,
    calculate_star_count, calculate_star_density_gain, calculate_streak_marks,
    calculate_trail_length, calculate_trail_strength,
};

const HISTORY_FIXTURE: &str = include_str!("fixtures/account-with-history.json");

#[test]
fn no_streak_leaves_no_trail_and_base_star_density() {
    let marks = calculate_streak_marks(&development_with_streaks(0, 0));
    assert_eq!(marks.trail_length, 0.0);
    assert_eq!(marks.trail_strength, 0.0);
    assert_eq!(marks.star_density_gain, 0.0);
    assert_eq!(calculate_star_count(0), BASE_STAR_COUNT);
}

#[test]
fn a_current_streak_extends_and_brightens_the_trail_up_to_a_cap() {
    let mut previous_length = 0.0;
    let mut previous_strength = 0.0;
    for week_count in [1, 2, 5, TRAIL_FULL_WEEKS] {
        let marks = calculate_streak_marks(&development_with_streaks(week_count, 0));
        assert!(
            marks.trail_length > previous_length,
            "{week_count} weeks did not lengthen the trail"
        );
        assert!(
            marks.trail_strength > previous_strength,
            "{week_count} weeks did not brighten the trail"
        );
        previous_length = marks.trail_length;
        previous_strength = marks.trail_strength;
    }
    let full_marks = calculate_streak_marks(&development_with_streaks(TRAIL_FULL_WEEKS, 0));
    assert_eq!(full_marks.trail_length, 1.0);
    assert_eq!(
        full_marks,
        calculate_streak_marks(&development_with_streaks(TRAIL_FULL_WEEKS * 4, 0))
    );
    assert_eq!(
        calculate_trail_length(TRAIL_FULL_WEEKS * 4),
        calculate_trail_length(TRAIL_FULL_WEEKS)
    );
    assert_eq!(
        calculate_trail_strength(TRAIL_FULL_WEEKS * 4),
        calculate_trail_strength(TRAIL_FULL_WEEKS)
    );
}

#[test]
fn an_ended_streak_lights_no_comet_trail_but_keeps_raising_star_density() {
    let mut development = development_with_streaks(0, 4);
    development.streaks.recent = Some(RecentStreak::Ended {
        weeks: streak_weeks(4),
    });
    let marks = calculate_streak_marks(&development);
    assert_eq!(marks.trail_length, 0.0);
    assert_eq!(marks.trail_strength, 0.0);
    assert!(marks.star_density_gain > 0.0);
    assert!(calculate_star_count(4) > BASE_STAR_COUNT);
}

#[test]
fn the_longest_streak_raises_star_count_up_to_a_cap() {
    let mut previous_count = calculate_star_count(0);
    let mut previous_gain = calculate_star_density_gain(0);
    for week_count in [1, 4, 13, STAR_DENSITY_FULL_WEEKS] {
        let count = calculate_star_count(week_count);
        let gain = calculate_star_density_gain(week_count);
        assert!(count > previous_count, "{week_count} weeks added no stars");
        assert!(
            gain > previous_gain,
            "{week_count} weeks did not raise density"
        );
        previous_count = count;
        previous_gain = gain;
    }
    assert_eq!(
        calculate_star_count(STAR_DENSITY_FULL_WEEKS),
        BASE_STAR_COUNT + EXTRA_STAR_COUNT
    );
    assert_eq!(
        calculate_star_count(STAR_DENSITY_FULL_WEEKS * 4),
        calculate_star_count(STAR_DENSITY_FULL_WEEKS)
    );
    assert_eq!(
        calculate_star_density_gain(STAR_DENSITY_FULL_WEEKS * 4),
        1.0
    );
}

fn development_with_streaks(current_weeks: usize, longest_weeks: usize) -> GalaxyDevelopment {
    let mut development = read_development();
    development.streaks = StreakDevelopment {
        longest: streak_weeks(longest_weeks),
        recent: (current_weeks > 0).then(|| RecentStreak::Current(streak_weeks(current_weeks))),
    };
    development
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

fn read_development() -> GalaxyDevelopment {
    let account_snapshot: AccountSnapshot = serde_json::from_str(HISTORY_FIXTURE).unwrap();
    derive_galaxy_development(&account_snapshot).unwrap()
}
