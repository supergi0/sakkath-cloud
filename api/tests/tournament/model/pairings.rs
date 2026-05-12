use api::helpers::rounds::RoundPairingDiagnostics;
use api::helpers::sorting::{TeamSortData, sort_teams_with_seed as live_sort_teams_with_seed};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct SortMetrics {
    pub(crate) team_id: i64,
    pub(crate) init_rank: i64,
    pub(crate) wins: i64,
    pub(crate) losses: i64,
    pub(crate) draws: i64,
    pub(crate) points: i64,
    pub(crate) points_for: i64,
    pub(crate) points_against: i64,
    pub(crate) spirit_avg: f64,
    pub(crate) round_results: Vec<i8>,
    pub(crate) opponents: Vec<i64>,
    pub(crate) h2h: HashMap<i64, i8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedSwissRound {
    pub(crate) naive_had_rematch: bool,
    pub(crate) diagnostics: RoundPairingDiagnostics,
}

pub(crate) fn sort_metrics_with_seed(metrics: &mut [SortMetrics], seed: u64) {
    let mut live_metrics: Vec<TeamSortData> = metrics.iter().map(as_live_sort_data).collect();
    live_sort_teams_with_seed(&mut live_metrics, seed);

    let order_by_team: HashMap<i64, usize> = live_metrics
        .iter()
        .enumerate()
        .map(|(index, metric)| (metric.team_id, index))
        .collect();

    metrics.sort_by_key(|metric| {
        order_by_team
            .get(&metric.team_id)
            .copied()
            .unwrap_or(usize::MAX)
    });
}

pub(crate) fn build_scoring_groups(sorted: &[SortMetrics]) -> Vec<Vec<i64>> {
    if sorted.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    let mut current_points = sorted[0].points;
    let mut current_group = Vec::new();

    for team in sorted {
        if team.points != current_points {
            groups.push(current_group);
            current_group = Vec::new();
            current_points = team.points;
        }
        current_group.push(team.team_id);
    }
    groups.push(current_group);

    fix_odd_groups(&mut groups);
    groups
        .into_iter()
        .filter(|group| !group.is_empty())
        .collect()
}

pub(crate) fn naive_pairings(groups: &[Vec<i64>]) -> Vec<(i64, i64)> {
    let mut pairings = Vec::new();
    for group in groups {
        let half = group.len() / 2;
        for index in 0..half {
            pairings.push((group[index], group[half + index]));
        }
    }
    pairings
}

fn as_live_sort_data(metrics: &SortMetrics) -> TeamSortData {
    TeamSortData {
        team_id: metrics.team_id,
        name: String::new(),
        abbreviation: None,
        small_logo: None,
        init_rank: metrics.init_rank,
        wins: metrics.wins,
        losses: metrics.losses,
        draws: metrics.draws,
        points: metrics.points,
        points_for: metrics.points_for,
        points_against: metrics.points_against,
        spirit_avg: metrics.spirit_avg,
        round_results: metrics.round_results.clone(),
        opponents: metrics.opponents.clone(),
        h2h: metrics.h2h.clone(),
    }
}

fn fix_odd_groups(groups: &mut [Vec<i64>]) {
    for index in 0..groups.len() {
        if !groups[index].len().is_multiple_of(2) && index + 1 < groups.len() {
            let overflow = groups[index]
                .pop()
                .expect("odd group should have a trailing team");
            groups[index + 1].insert(0, overflow);
        }
    }
}
