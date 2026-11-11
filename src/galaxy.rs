//! Reproducible galaxy development from account observations, independent of rendering.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use chrono::{Datelike, NaiveDate};

use crate::history::{
    AccountSnapshot, ConflictingRepository, ContributionHistoryError, ContributionSummary,
    ContributionWeek, LanguageColor, calculate_language_shares, summarize_contributions,
};

/// Derived inputs for appearance; source observations remain in the account snapshot.
#[derive(Debug, PartialEq)]
pub struct GalaxyDevelopment {
    /// The stable numeric GitHub account ID.
    pub identity_seed: u64,
    /// Each counted language's share of eligible repositories, from zero to one, by name.
    pub language_shares: BTreeMap<String, f64>,
    /// GitHub's color for each language in `language_shares`; `None` means it has no color.
    pub language_colors: BTreeMap<String, Option<LanguageColor>>,
    /// Unweighted contributions available to the renderer's growth curve.
    pub total_contributions: u64,
    /// The date every time-based measurement is taken against.
    pub observed_on: NaiveDate,
    /// The longest and the most recent weekly streaks.
    pub streaks: StreakDevelopment,
    /// Counts around the observation date that motion selection reads.
    pub recent_activity: RecentActivity,
}

/// Recent activity as of the observation date, as counts rather than judgments.
#[derive(Debug, Eq, PartialEq)]
pub struct RecentActivity {
    /// Days in the run of contribution-free days that ends on the observation date;
    /// an account that has never contributed counts every observed day.
    pub quiet_day_count: u32,
}

/// Weekly streaks: runs of consecutive Monday-Sunday weeks with at least one active day.
#[derive(Debug, Eq, PartialEq)]
pub struct StreakDevelopment {
    /// The earliest longest run, replaced only by a strictly longer run.
    pub longest: Vec<ContributionWeek>,
    /// None means no observed active week, not missing history.
    pub recent: Option<RecentStreak>,
}

/// The most recent streak, still running or ended.
#[derive(Debug, Eq, PartialEq)]
pub enum RecentStreak {
    /// Still running through the observation week; carries its weeks, oldest first.
    Current(Vec<ContributionWeek>),
    /// Ended by a completed week without activity.
    Ended {
        /// The streak's weeks, oldest first.
        weeks: Vec<ContributionWeek>,
    },
}

/// Derive one account's identity, language composition, and contribution development.
///
/// Any active day sustains a Monday-Sunday weekly streak. A completed inactive
/// week ends it; the observation week is still in progress and cannot end it.
/// Each streak retains weekly participation rather than just its length.
/// Returning activity replaces the recent streak without discarding the record.
///
/// History owns input validation and language eligibility. Its errors propagate;
/// unavailable history never becomes an undeveloped galaxy. Confirmed empty history
/// produces zero contributions and no streaks. Every counted language needs a
/// recorded color, so a snapshot without colors fails instead of producing an
/// uncolored galaxy. Inputs are borrowed without mutation; this operation performs
/// no I/O and reads no clock. It assigns no visual thresholds.
pub fn derive_galaxy_development(
    account_snapshot: &AccountSnapshot,
) -> Result<GalaxyDevelopment, GalaxyDevelopmentError> {
    let contribution_summary = summarize_contributions(account_snapshot)
        .map_err(GalaxyDevelopmentError::ContributionHistory)?;
    let language_shares = calculate_language_shares(account_snapshot)
        .map_err(GalaxyDevelopmentError::Repositories)?;
    let recorded_colors = account_snapshot
        .language_colors
        .as_ref()
        .ok_or(GalaxyDevelopmentError::MissingLanguageColors)?;
    let language_colors = language_shares
        .keys()
        .map(|language_name| {
            recorded_colors
                .get(language_name)
                .map(|language_color| (language_name.clone(), *language_color))
                .ok_or_else(|| GalaxyDevelopmentError::MissingLanguageColor(language_name.clone()))
        })
        .collect::<Result<_, _>>()?;
    Ok(GalaxyDevelopment {
        identity_seed: account_snapshot.account_id,
        language_shares,
        language_colors,
        total_contributions: contribution_summary.total_contributions,
        observed_on: contribution_summary.observed_on,
        streaks: calculate_streak_development(&contribution_summary),
        recent_activity: measure_recent_activity(&contribution_summary),
    })
}

/// Growth stops here; larger totals keep the maximum growth fraction.
pub const MAX_GROWTH_CONTRIBUTIONS: u64 = 100_000;

/// How many visible growth stages a galaxy has, from a spark to a full disc.
pub const STAGE_COUNT: u8 = 5;

/// Map total contributions to a growth fraction from zero through one.
/// Gains diminish until MAX_GROWTH_CONTRIBUTIONS; larger totals stay at one.
/// The caller supplies the observed total; elapsed time does not affect growth.
pub fn calculate_growth_fraction(total_contributions: u64) -> f64 {
    const CONTRIBUTION_SCALE: f64 = 50.0;
    let bounded_total = total_contributions.min(MAX_GROWTH_CONTRIBUTIONS);
    (1.0 + bounded_total as f64 / CONTRIBUTION_SCALE).log2()
        / (1.0 + MAX_GROWTH_CONTRIBUTIONS as f64 / CONTRIBUTION_SCALE).log2()
}

/// Select the visible growth stage from total contributions.
///
/// The stages are equal fifths of [`calculate_growth_fraction`]: stage one spans growth
/// from zero up to (but not including) one fifth, and so on. The boundary totals are
/// 0-178 (stage 1), 179-995 (stage 2), 996-4733 (stage 3), 4734-21825 (stage 4), and
/// 21826 and above (stage 5). Totals at or beyond [`MAX_GROWTH_CONTRIBUTIONS`] stay at
/// the last stage. The caller supplies the observed total; elapsed time does not matter.
pub fn calculate_stage(total_contributions: u64) -> u8 {
    let stage =
        1.0 + (calculate_growth_fraction(total_contributions) * f64::from(STAGE_COUNT)).floor();
    (stage as u8).clamp(1, STAGE_COUNT)
}

/// Why a snapshot produced no development. Each message names the repair.
#[derive(Debug, Eq, PartialEq)]
pub enum GalaxyDevelopmentError {
    /// The contribution history failed its rules.
    ContributionHistory(ContributionHistoryError),
    /// Two observations of one repository disagree.
    Repositories(ConflictingRepository),
    /// The snapshot predates language colors.
    MissingLanguageColors,
    /// The snapshot has no color entry for this counted language.
    MissingLanguageColor(String),
}

impl fmt::Display for GalaxyDevelopmentError {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContributionHistory(history_error) => write!(output_formatter, "{history_error}"),
            Self::Repositories(repository_error) => write!(output_formatter, "{repository_error}"),
            Self::MissingLanguageColors => write!(
                output_formatter,
                "the snapshot has no language colors; collect it again with `gitlaxy collect`"
            ),
            Self::MissingLanguageColor(language_name) => write!(
                output_formatter,
                "the snapshot has no color for {language_name}; collect it again with `gitlaxy collect`"
            ),
        }
    }
}

impl Error for GalaxyDevelopmentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ContributionHistory(history_error) => Some(history_error),
            Self::Repositories(repository_error) => Some(repository_error),
            Self::MissingLanguageColors | Self::MissingLanguageColor(_) => None,
        }
    }
}

fn measure_recent_activity(contribution_summary: &ContributionSummary) -> RecentActivity {
    RecentActivity {
        quiet_day_count: contribution_summary
            .daily_counts
            .values()
            .rev()
            .take_while(|&&contribution_count| contribution_count == 0)
            .count() as u32,
    }
}

fn calculate_streak_development(contribution_summary: &ContributionSummary) -> StreakDevelopment {
    let mut longest_week_range = 0..0;
    let mut recent_week_range = 0..0;
    let mut current_start_week_index = None;
    let observation_week = contribution_summary.observed_on.iso_week();
    for (week_index, contribution_week) in contribution_summary.weeks.iter().enumerate() {
        if contribution_week.active_day_count > 0 {
            let start_week_index = *current_start_week_index.get_or_insert(week_index);
            recent_week_range = start_week_index..week_index + 1;
            if recent_week_range.len() > longest_week_range.len() {
                longest_week_range = recent_week_range.clone();
            }
        } else if contribution_week.iso_week != observation_week {
            // A completed inactive week ends the run; the observation week cannot.
            current_start_week_index = None;
        }
    }
    let recent_streak = if recent_week_range.is_empty() {
        None
    } else {
        let recent_weeks = contribution_summary.weeks[recent_week_range].to_vec();
        Some(if current_start_week_index.is_some() {
            RecentStreak::Current(recent_weeks)
        } else {
            RecentStreak::Ended {
                weeks: recent_weeks,
            }
        })
    };
    StreakDevelopment {
        longest: contribution_summary.weeks[longest_week_range].to_vec(),
        recent: recent_streak,
    }
}
