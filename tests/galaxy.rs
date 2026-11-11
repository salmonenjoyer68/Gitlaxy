use std::collections::BTreeMap;

use chrono::{Days, NaiveDate};
use gitlaxy::galaxy::{
    GalaxyDevelopmentError, RecentActivity, RecentStreak, STAGE_MINIMUM_CONTRIBUTIONS,
    calculate_stage, derive_galaxy_development,
};
use gitlaxy::history::{
    AccountSnapshot, ConflictingRepository, ContributionDay, ContributionHistory,
    ContributionHistoryError, LanguageColor,
};

#[test]
fn identity_survives_growth_and_language_changes_but_distinguishes_accounts() {
    let mut account_snapshot = build_snapshot_with_counts(&[1; 7]);
    let original_development = derive_galaxy_development(&account_snapshot).unwrap();
    account_snapshot.repositories.clear();
    account_snapshot.contribution_history.as_mut().unwrap().days[0].contribution_count += 5;
    let changed_development = derive_galaxy_development(&account_snapshot).unwrap();
    assert_eq!(
        changed_development.identity_seed,
        original_development.identity_seed
    );
    assert_ne!(
        changed_development.language_shares,
        original_development.language_shares
    );
    assert_eq!(
        changed_development.total_contributions,
        original_development.total_contributions + 5
    );
    account_snapshot.account_id += 1;
    assert_ne!(
        derive_galaxy_development(&account_snapshot)
            .unwrap()
            .identity_seed,
        original_development.identity_seed
    );
}

#[test]
fn development_is_reproducible_after_reordering_and_duplicate_observations() {
    let mut account_snapshot = build_snapshot_with_counts(&[1; 15]);
    let original_development = derive_galaxy_development(&account_snapshot).unwrap();
    account_snapshot
        .repositories
        .push(account_snapshot.repositories[0].clone());
    account_snapshot.repositories.reverse();
    let contribution_days = &mut account_snapshot.contribution_history.as_mut().unwrap().days;
    contribution_days.push(contribution_days[0].clone());
    contribution_days.reverse();
    assert_eq!(
        derive_galaxy_development(&account_snapshot).unwrap(),
        original_development
    );
}

#[test]
fn twelve_week_streaks_retain_different_participation_with_equal_totals() {
    for (weekly_counts, expected_active_days) in
        [([1, 1, 1, 1, 1, 0, 0], 5), ([0, 0, 0, 0, 0, 5, 0], 1)]
    {
        let account_snapshot = build_snapshot_with_counts(&weekly_counts.repeat(12));
        let galaxy_development = derive_galaxy_development(&account_snapshot).unwrap();
        assert_eq!(galaxy_development.total_contributions, 60);
        assert_eq!(
            galaxy_development
                .streaks
                .longest
                .iter()
                .map(|contribution_week| contribution_week.active_day_count)
                .collect::<Vec<_>>(),
            vec![expected_active_days; 12]
        );
        assert_eq!(
            galaxy_development.streaks.recent,
            Some(RecentStreak::Current(galaxy_development.streaks.longest))
        );
    }
}

#[test]
fn inactive_weeks_end_the_current_streak_without_erasing_or_restarting_it() {
    let mut daily_counts = vec![1; 7];
    daily_counts.extend([0; 7]);
    let unfinished_week_development =
        derive_galaxy_development(&build_snapshot_with_counts(&daily_counts)).unwrap();
    assert_eq!(
        unfinished_week_development.streaks.recent,
        Some(RecentStreak::Current(
            unfinished_week_development.streaks.longest.clone()
        ))
    );
    daily_counts.push(0);
    let ended_development =
        derive_galaxy_development(&build_snapshot_with_counts(&daily_counts)).unwrap();
    assert_eq!(
        ended_development.streaks.longest,
        unfinished_week_development.streaks.longest
    );
    assert_eq!(
        ended_development.streaks.recent,
        Some(RecentStreak::Ended {
            weeks: unfinished_week_development.streaks.longest,
        })
    );
    daily_counts.extend([0; 14]);
    let quiet_development =
        derive_galaxy_development(&build_snapshot_with_counts(&daily_counts)).unwrap();
    assert_eq!(quiet_development.streaks, ended_development.streaks);
    assert_eq!(
        quiet_development.total_contributions,
        ended_development.total_contributions
    );
}

#[test]
fn returning_streak_changes_the_record_only_after_strictly_beating_it() {
    let mut daily_counts = vec![1; 14];
    daily_counts.extend([0; 7]);
    daily_counts.push(1);
    for (expected_current_weeks, expected_record_days) in
        [(1, &[7, 7][..]), (2, &[7, 7][..]), (3, &[1, 1, 1][..])]
    {
        let galaxy_development =
            derive_galaxy_development(&build_snapshot_with_counts(&daily_counts)).unwrap();
        let Some(RecentStreak::Current(current_weeks)) = galaxy_development.streaks.recent else {
            panic!("returning activity must produce a current streak");
        };
        assert_eq!(current_weeks.len(), expected_current_weeks);
        assert_eq!(
            galaxy_development
                .streaks
                .longest
                .iter()
                .map(|contribution_week| contribution_week.active_day_count)
                .collect::<Vec<_>>(),
            expected_record_days
        );
        daily_counts.extend([0; 6]);
        daily_counts.push(1);
    }
}

#[test]
fn a_shorter_ended_streak_remains_available_alongside_the_permanent_record() {
    let mut daily_counts = vec![1; 14];
    daily_counts.extend([0; 7]);
    daily_counts.push(1);
    daily_counts.extend([0; 14]);
    let galaxy_development =
        derive_galaxy_development(&build_snapshot_with_counts(&daily_counts)).unwrap();
    assert_eq!(galaxy_development.streaks.longest.len(), 2);
    let Some(RecentStreak::Ended { weeks: ended_weeks }) = galaxy_development.streaks.recent else {
        panic!("the latest shorter streak must remain available for fading");
    };
    assert_eq!(ended_weeks.len(), 1);
    assert_eq!(ended_weeks[0].active_day_count, 1);
}

#[test]
fn empty_observations_produce_no_streaks_but_unavailable_history_fails() {
    let mut account_snapshot = build_snapshot_with_counts(&[0; 14]);
    let inactive_development = derive_galaxy_development(&account_snapshot).unwrap();
    assert_eq!(inactive_development.total_contributions, 0);
    assert!(inactive_development.streaks.longest.is_empty());
    assert!(inactive_development.streaks.recent.is_none());
    account_snapshot
        .contribution_history
        .as_mut()
        .unwrap()
        .days
        .clear();
    // Observed quiet days are recent activity; an empty history has none to report.
    let empty_development = derive_galaxy_development(&account_snapshot).unwrap();
    assert_eq!(empty_development.total_contributions, 0);
    assert_eq!(empty_development.streaks, inactive_development.streaks);
    account_snapshot.contribution_history = None;
    assert_eq!(
        derive_galaxy_development(&account_snapshot),
        Err(GalaxyDevelopmentError::ContributionHistory(
            ContributionHistoryError::MissingHistory
        ))
    );
}

#[test]
fn conflicting_repositories_cannot_produce_a_partial_development_model() {
    let mut account_snapshot = build_snapshot_with_counts(&[1]);
    let mut conflicting_repository = account_snapshot.repositories[0].clone();
    conflicting_repository.is_fork = !conflicting_repository.is_fork;
    let expected_error = ConflictingRepository {
        repository_id: conflicting_repository.repository_id,
    };
    account_snapshot.repositories.push(conflicting_repository);
    assert_eq!(
        derive_galaxy_development(&account_snapshot),
        Err(GalaxyDevelopmentError::Repositories(expected_error))
    );
}

#[test]
fn development_carries_the_color_of_each_counted_language() {
    let mut account_snapshot = build_snapshot_with_counts(&[1; 7]);
    let recorded_colors = account_snapshot.language_colors.as_mut().unwrap();
    recorded_colors.insert("C++".to_owned(), None);
    recorded_colors.insert("Go".to_owned(), Some(LanguageColor([0x00, 0xad, 0xd8])));
    assert_eq!(
        derive_galaxy_development(&account_snapshot)
            .unwrap()
            .language_colors,
        BTreeMap::from([
            ("C++".to_owned(), None),
            ("Python".to_owned(), Some(LanguageColor([0x35, 0x72, 0xa5]))),
            ("Rust".to_owned(), Some(LanguageColor([0xde, 0xa5, 0x84]))),
        ])
    );
}

#[test]
fn snapshots_without_language_colors_ask_for_a_new_collection() {
    let mut account_snapshot = build_snapshot_with_counts(&[1; 7]);
    account_snapshot
        .language_colors
        .as_mut()
        .unwrap()
        .remove("Python");
    assert_eq!(
        derive_galaxy_development(&account_snapshot),
        Err(GalaxyDevelopmentError::MissingLanguageColor(
            "Python".to_owned()
        ))
    );
    account_snapshot.language_colors = None;
    let missing_colors = derive_galaxy_development(&account_snapshot);
    assert_eq!(
        missing_colors,
        Err(GalaxyDevelopmentError::MissingLanguageColors)
    );
    assert!(
        missing_colors
            .unwrap_err()
            .to_string()
            .contains("gitlaxy collect")
    );
}

#[test]
fn recent_activity_counts_the_quiet_run_through_the_observation_date() {
    let quiet_days_for = |daily_counts: &[u32]| {
        derive_galaxy_development(&build_snapshot_with_counts(daily_counts))
            .unwrap()
            .recent_activity
            .quiet_day_count
    };
    // Two full weeks, then a quiet Monday and Tuesday.
    assert_eq!(
        quiet_days_for(&[vec![1; 7], vec![0, 0, 0, 0, 0, 0, 2], vec![0, 0]].concat()),
        2
    );
    // Active on the observation date, partway through the first week.
    assert_eq!(quiet_days_for(&[0, 0, 3]), 0);
    // Never active: every observed day is quiet.
    assert_eq!(quiet_days_for(&[0; 5]), 5);
    // Observed on a Sunday, the full observation week is still in progress.
    assert_eq!(quiet_days_for(&[1, 2, 3, 4, 5, 6, 7]), 0);
    let mut empty_snapshot = build_snapshot_with_counts(&[1]);
    empty_snapshot
        .contribution_history
        .as_mut()
        .unwrap()
        .days
        .clear();
    assert_eq!(
        derive_galaxy_development(&empty_snapshot)
            .unwrap()
            .recent_activity,
        RecentActivity { quiet_day_count: 0 }
    );
}

#[test]
fn stages_start_at_their_contribution_minimums() {
    for (stage_index, &minimum) in STAGE_MINIMUM_CONTRIBUTIONS.iter().enumerate() {
        assert_eq!(calculate_stage(minimum), stage_index as u8 + 1);
        if minimum > 0 {
            assert_eq!(calculate_stage(minimum - 1), stage_index as u8);
        }
    }
}

#[test]
fn stage_boundaries_cover_zero_max_and_beyond() {
    assert_eq!(calculate_stage(0), 1);
    assert_eq!(calculate_stage(249), 1);
    assert_eq!(calculate_stage(250), 2);
    assert_eq!(calculate_stage(749), 2);
    assert_eq!(calculate_stage(750), 3);
    assert_eq!(calculate_stage(1_249), 3);
    assert_eq!(calculate_stage(1_250), 4);
    assert_eq!(calculate_stage(1_499), 4);
    assert_eq!(calculate_stage(1_500), 5);
    assert_eq!(calculate_stage(u64::MAX), 5);
}

fn build_snapshot_with_counts(daily_counts: &[u32]) -> AccountSnapshot {
    let first_monday: NaiveDate = "2025-12-15".parse().unwrap();
    let contribution_days: Vec<_> = daily_counts
        .iter()
        .enumerate()
        .map(|(day_offset, contribution_count)| ContributionDay {
            date: first_monday
                .checked_add_days(Days::new(day_offset as u64))
                .unwrap(),
            contribution_count: *contribution_count,
        })
        .collect();
    let mut account_snapshot: AccountSnapshot =
        serde_json::from_str(include_str!("fixtures/account.json")).unwrap();
    account_snapshot.contribution_history = Some(ContributionHistory {
        observed_on: contribution_days.last().unwrap().date,
        days: contribution_days,
    });
    account_snapshot.language_colors = Some(BTreeMap::from([
        ("C++".to_owned(), Some(LanguageColor([0xf3, 0x4b, 0x7d]))),
        ("Python".to_owned(), Some(LanguageColor([0x35, 0x72, 0xa5]))),
        ("Rust".to_owned(), Some(LanguageColor([0xde, 0xa5, 0x84]))),
    ]));
    account_snapshot
}
