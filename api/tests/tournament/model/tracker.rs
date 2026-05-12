use super::api_types::{
    MatchDetailResponse, ScheduleMatchResponse, ScoreConfirmRowResponse, SpiritScoreRowResponse,
    TeamResponse,
};
use super::pairings::{
    ExpectedSwissRound, SortMetrics, build_scoring_groups, naive_pairings, sort_metrics_with_seed,
};
use crate::tournament::config::deterministic_stage_coin_toss_seed;
use api::helpers::{
    rounds::{
        PlayedMatchResult as LivePlayedMatchResult,
        build_seed_order_after_elimination_results as live_build_seed_order_after_elimination_results,
        generate_round_pairings_with_diagnostics as live_generate_round_pairings_with_diagnostics,
    },
    sorting::TeamSortData,
};
use chrono::{Duration, NaiveDateTime};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct TeamInfo {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) division: i64,
    pub(crate) init_rank: i64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PlayerTotals {
    pub(crate) goals: i64,
    pub(crate) assists: i64,
    pub(crate) blocks: i64,
    pub(crate) turnovers: i64,
    pub(crate) matches: HashSet<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct SpiritSubmission {
    pub(crate) total: i64,
    pub(crate) mvp_player_id: Option<i64>,
    pub(crate) msp_player_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct MatchState {
    pub(crate) id: i64,
    pub(crate) division: i64,
    pub(crate) match_type: i64,
    pub(crate) field_name: String,
    pub(crate) time: String,
    pub(crate) t1_id: i64,
    pub(crate) t2_id: i64,
    pub(crate) t1_score: i64,
    pub(crate) t2_score: i64,
    pub(crate) possession: Option<i64>,
    pub(crate) applied_event_ids: HashSet<i64>,
    pub(crate) spirit_rows: HashMap<(i64, i64), SpiritSubmission>,
    pub(crate) score_confirmations: HashMap<i64, (i64, i64)>,
}

#[derive(Debug, Clone)]
pub(crate) struct TournamentTracker {
    pub(crate) teams: HashMap<i64, TeamInfo>,
    pub(crate) players: HashMap<i64, PlayerTotals>,
    pub(crate) matches: HashMap<i64, MatchState>,
    stage_seed_base: u64,
}

impl TournamentTracker {
    pub(crate) fn new(teams: &[TeamResponse], stage_seed_base: u64) -> Self {
        let teams = teams
            .iter()
            .map(|team| {
                (
                    team.id,
                    TeamInfo {
                        id: team.id,
                        name: team.name.clone(),
                        division: team.division,
                        init_rank: team.init_rank.unwrap_or(i64::MAX),
                    },
                )
            })
            .collect();

        Self {
            teams,
            players: HashMap::new(),
            matches: HashMap::new(),
            stage_seed_base,
        }
    }

    pub(crate) fn register_schedule_match(
        &mut self,
        division: i64,
        schedule_match: &ScheduleMatchResponse,
    ) {
        let state = self
            .matches
            .entry(schedule_match.id)
            .or_insert_with(|| MatchState {
                id: schedule_match.id,
                division,
                match_type: schedule_match.match_type,
                field_name: schedule_match.field_name.clone(),
                time: schedule_match.time.clone(),
                t1_id: schedule_match.t1_id,
                t2_id: schedule_match.t2_id,
                t1_score: schedule_match.t1_score,
                t2_score: schedule_match.t2_score,
                possession: schedule_match.possession,
                applied_event_ids: HashSet::new(),
                spirit_rows: HashMap::new(),
                score_confirmations: HashMap::new(),
            });

        state.match_type = schedule_match.match_type;
        state.field_name = schedule_match.field_name.clone();
        state.time = schedule_match.time.clone();
        state.t1_score = schedule_match.t1_score;
        state.t2_score = schedule_match.t2_score;
        state.possession = schedule_match.possession;
    }

    pub(crate) fn apply_detail(&mut self, match_type: i64, detail: &MatchDetailResponse) {
        let division = self
            .teams
            .get(&detail.t1_id)
            .map(|team| team.division)
            .unwrap_or(0);

        let mut new_events = Vec::new();

        {
            let state = self.matches.entry(detail.id).or_insert_with(|| MatchState {
                id: detail.id,
                division,
                match_type,
                field_name: String::new(),
                time: String::new(),
                t1_id: detail.t1_id,
                t2_id: detail.t2_id,
                t1_score: detail.t1_score,
                t2_score: detail.t2_score,
                possession: detail.possession,
                applied_event_ids: HashSet::new(),
                spirit_rows: HashMap::new(),
                score_confirmations: HashMap::new(),
            });

            state.match_type = match_type;
            state.t1_score = detail.t1_score;
            state.t2_score = detail.t2_score;
            state.possession = detail.possession;

            for event in &detail.events {
                if state.applied_event_ids.insert(event.id) {
                    new_events.push((event.player_id, event.event_type));
                }
            }
        }

        for (player_id, event_type) in new_events {
            self.apply_player_event(detail.id, player_id, event_type);
        }
    }

    pub(crate) fn apply_manual_event(
        &mut self,
        match_id: i64,
        player_id: Option<i64>,
        event_type: i64,
    ) {
        self.apply_player_event(match_id, player_id, event_type);
    }

    pub(crate) fn set_match_state(
        &mut self,
        match_id: i64,
        t1_score: i64,
        t2_score: i64,
        possession: Option<i64>,
    ) {
        if let Some(state) = self.matches.get_mut(&match_id) {
            state.t1_score = t1_score;
            state.t2_score = t2_score;
            state.possession = possession;
        }
    }

    pub(crate) fn record_spirit_row(&mut self, row: &SpiritScoreRowResponse) {
        if let Some(state) = self.matches.get_mut(&row.match_id) {
            state.spirit_rows.insert(
                (row.team_id, row.submitted_by_team_id),
                SpiritSubmission {
                    total: row.total,
                    mvp_player_id: row.mvp_player_id,
                    msp_player_id: row.msp_player_id,
                },
            );
        }
    }

    pub(crate) fn record_spirit_submission(
        &mut self,
        match_id: i64,
        rated_team_id: i64,
        submitted_by_team_id: i64,
        total: i64,
        mvp_player_id: Option<i64>,
        msp_player_id: Option<i64>,
    ) {
        if let Some(state) = self.matches.get_mut(&match_id) {
            state.spirit_rows.insert(
                (rated_team_id, submitted_by_team_id),
                SpiritSubmission {
                    total,
                    mvp_player_id,
                    msp_player_id,
                },
            );
        }
    }

    pub(crate) fn record_score_confirmation(&mut self, row: &ScoreConfirmRowResponse) {
        if let Some(state) = self.matches.get_mut(&row.match_id) {
            state
                .score_confirmations
                .insert(row.team_id, (row.t1_score, row.t2_score));
        }
    }

    pub(crate) fn set_score_confirmation(
        &mut self,
        match_id: i64,
        team_id: i64,
        t1_score: i64,
        t2_score: i64,
    ) {
        if let Some(state) = self.matches.get_mut(&match_id) {
            state
                .score_confirmations
                .insert(team_id, (t1_score, t2_score));
        }
    }

    pub(crate) fn match_state(&self, match_id: i64) -> &MatchState {
        self.matches
            .get(&match_id)
            .unwrap_or_else(|| panic!("missing tracked match {match_id}"))
    }

    pub(crate) fn team_name(&self, team_id: i64) -> &str {
        &self
            .teams
            .get(&team_id)
            .unwrap_or_else(|| panic!("missing tracked team {team_id}"))
            .name
    }

    pub(crate) fn init_rank(&self, team_id: i64) -> i64 {
        self.teams
            .get(&team_id)
            .map(|team| team.init_rank)
            .unwrap_or(i64::MAX)
    }

    pub(crate) fn completed_match_count(&self) -> i64 {
        self.matches
            .values()
            .filter(|state| state.possession.unwrap_or(0) >= 3)
            .count() as i64
    }

    pub(crate) fn total_points_scored(&self) -> i64 {
        self.matches
            .values()
            .map(|state| state.t1_score + state.t2_score)
            .sum()
    }

    pub(crate) fn match_ids_for_team(&self, team_id: i64) -> HashSet<i64> {
        self.matches
            .values()
            .filter(|state| state.t1_id == team_id || state.t2_id == team_id)
            .map(|state| state.id)
            .collect()
    }

    pub(crate) fn game_gap_minutes(&self, division: i64) -> Vec<i64> {
        let mut gaps = Vec::new();
        let canonical_starts = self.canonical_gap_start_times(division);

        for team_id in self
            .teams
            .values()
            .filter(|team| team.division == division)
            .map(|team| team.id)
        {
            let mut matches: Vec<_> = self
                .matches
                .values()
                .filter(|state| {
                    state.division == division
                        && state.possession.unwrap_or(0) >= 3
                        && (state.t1_id == team_id || state.t2_id == team_id)
                })
                .filter_map(|state| {
                    canonical_starts
                        .get(&state.id)
                        .copied()
                        .or_else(|| parse_match_time(&state.time))
                        .map(|start_time| (state, start_time))
                })
                .collect();
            matches.sort_by_key(|(_, start_time)| *start_time);

            for window in matches.windows(2) {
                let (current_match, current_start) = window[0];
                let (_, next_start) = window[1];
                if current_start.date() != next_start.date() {
                    continue;
                }
                let current_end = current_start
                    + Duration::minutes(match_duration_minutes(
                        current_match.division,
                        current_match.match_type,
                    ));
                gaps.push(next_start.signed_duration_since(current_end).num_minutes());
            }
        }

        gaps
    }

    pub(crate) fn ground_distribution_score(&self, division: Option<i64>) -> f64 {
        let team_scores: Vec<f64> = self
            .teams
            .values()
            .filter(|team| division.is_none_or(|value| team.division == value))
            .filter_map(|team| self.team_ground_distribution_msd(team.id))
            .collect();

        if team_scores.is_empty() {
            0.0
        } else {
            team_scores.iter().sum::<f64>() / team_scores.len() as f64
        }
    }

    fn canonical_gap_start_times(&self, division: i64) -> HashMap<i64, NaiveDateTime> {
        let mut grouped: BTreeMap<i64, Vec<&MatchState>> = BTreeMap::new();

        for state in self
            .matches
            .values()
            .filter(|state| state.division == division && state.possession.unwrap_or(0) >= 3)
        {
            grouped.entry(state.match_type).or_default().push(state);
        }

        let mut starts = HashMap::new();

        for (match_type, mut states) in grouped {
            states.sort_by_key(|state| (parse_match_time(&state.time), state.id));
            let canonical_times = canonical_gap_stage_start_times(division, match_type);

            for (index, state) in states.into_iter().enumerate() {
                let start_time = canonical_times
                    .as_ref()
                    .and_then(|times| times.get(index).copied())
                    .or_else(|| parse_match_time(&state.time));

                if let Some(start_time) = start_time {
                    starts.insert(state.id, start_time);
                }
            }
        }

        starts
    }

    pub(crate) fn swiss_summary(&self, division: i64) -> Vec<SortMetrics> {
        self.summary(division, false, None)
    }

    pub(crate) fn display_summary_for_order(
        &self,
        division: i64,
        order: &[i64],
    ) -> Vec<SortMetrics> {
        self.summary(division, true, Some(order))
    }

    fn summary(
        &self,
        division: i64,
        include_elimination: bool,
        order: Option<&[i64]>,
    ) -> Vec<SortMetrics> {
        let team_ids: Vec<i64> = self
            .teams
            .values()
            .filter(|team| team.division == division)
            .map(|team| team.id)
            .collect();

        let mut metrics = Vec::new();

        for team_id in team_ids {
            let mut wins = 0;
            let mut losses = 0;
            let mut draws = 0;
            let mut points = 0;
            let mut points_for = 0;
            let mut points_against = 0;
            let mut opponents = Vec::new();
            let mut h2h = HashMap::new();
            let mut per_round = BTreeMap::new();

            for state in self.matches.values().filter(|state| {
                state.division == division
                    && state.possession.unwrap_or(0) >= 3
                    && (include_elimination || state.match_type < 1000)
            }) {
                let (scored, conceded, opponent) = if state.t1_id == team_id {
                    (state.t1_score, state.t2_score, state.t2_id)
                } else if state.t2_id == team_id {
                    (state.t2_score, state.t1_score, state.t1_id)
                } else {
                    continue;
                };

                points_for += scored;
                points_against += conceded;
                opponents.push(opponent);

                let result = if scored > conceded {
                    wins += 1;
                    points += 2;
                    1
                } else if scored < conceded {
                    losses += 1;
                    -1
                } else {
                    draws += 1;
                    points += 1;
                    0
                };

                per_round.insert(state.match_type, result);
                h2h.insert(opponent, result);
            }

            let round_results = per_round.into_values().collect();
            let spirit_avg = self.team_spirit_average(team_id, !include_elimination);

            metrics.push(SortMetrics {
                team_id,
                init_rank: self
                    .teams
                    .get(&team_id)
                    .map(|team| team.init_rank)
                    .unwrap_or(i64::MAX),
                wins,
                losses,
                draws,
                points,
                points_for,
                points_against,
                spirit_avg,
                round_results,
                opponents,
                h2h,
            });
        }

        if let Some(order) = order {
            let mut metrics_by_team: HashMap<i64, SortMetrics> = metrics
                .into_iter()
                .map(|metric| (metric.team_id, metric))
                .collect();
            let mut ordered = Vec::with_capacity(metrics_by_team.len());

            for team_id in order {
                if let Some(metric) = metrics_by_team.remove(team_id) {
                    ordered.push(metric);
                }
            }

            let mut remaining: Vec<_> = metrics_by_team.into_values().collect();
            remaining.sort_by_key(|metric| (metric.init_rank, metric.team_id));
            ordered.extend(remaining);
            ordered
        } else {
            sort_metrics_with_seed(&mut metrics, self.current_stage_seed(division));
            metrics
        }
    }

    fn current_stage_seed(&self, division: i64) -> u64 {
        deterministic_stage_coin_toss_seed(
            self.stage_seed_base,
            division,
            self.current_stage_key(division),
        )
    }

    fn current_stage_key(&self, division: i64) -> i64 {
        let total_rounds = if division == 0 {
            api::OPEN_ROUNDS
        } else {
            api::WOMEN_ROUNDS
        };

        let highest_existing = self
            .matches
            .values()
            .filter(|state| state.division == division)
            .map(|state| state.match_type)
            .max()
            .unwrap_or(1);

        if self.stage_is_complete(division, highest_existing) {
            if highest_existing < total_rounds {
                return highest_existing + 1;
            }

            if highest_existing == total_rounds {
                return if division == 1 { 1002 } else { 1001 };
            }

            if highest_existing == 1001 {
                return 1002;
            }
        }

        highest_existing
    }

    fn stage_is_complete(&self, division: i64, stage_key: i64) -> bool {
        let mut total = 0usize;
        let mut completed = 0usize;

        for state in self
            .matches
            .values()
            .filter(|state| state.division == division && state.match_type == stage_key)
        {
            total += 1;
            if state.possession.unwrap_or(0) >= 3 {
                completed += 1;
            }
        }

        total > 0 && total == completed
    }

    pub(crate) fn standings_rank_map(&self, division: i64) -> HashMap<i64, i64> {
        self.swiss_summary(division)
            .iter()
            .enumerate()
            .map(|(index, metrics)| (metrics.team_id, index as i64 + 1))
            .collect()
    }

    pub(crate) fn expected_swiss_round(&self, division: i64) -> ExpectedSwissRound {
        let standings = self.swiss_summary(division);
        let history = self.match_history(division, true);
        let grouped = build_scoring_groups(&standings);
        let naive_pairings = naive_pairings(&grouped);
        let naive_had_rematch = naive_pairings.iter().any(|(team_a, team_b)| {
            history
                .get(team_a)
                .map(|opponents| opponents.contains(team_b))
                .unwrap_or(false)
        });

        let next_round = standings
            .first()
            .map(|metrics| metrics.round_results.len() as i64 + 1)
            .unwrap_or(1);
        let live_standings: Vec<TeamSortData> = standings
            .iter()
            .map(|metrics| self.as_live_sort_data(metrics))
            .collect();
        let pairing_result =
            live_generate_round_pairings_with_diagnostics(&live_standings, &history, next_round);

        ExpectedSwissRound {
            naive_had_rematch,
            diagnostics: pairing_result.diagnostics,
        }
    }

    pub(crate) fn expected_display_order_after_playoffs(&self, division: i64) -> Vec<i64> {
        let standings = self.live_swiss_sort_data(division);
        let playoff_results = self.completed_playoff_results(division, 1001);
        live_build_seed_order_after_elimination_results(&standings, &playoff_results, &[])
    }

    pub(crate) fn expected_display_order_after_elimination(&self, division: i64) -> Vec<i64> {
        let standings = self.live_swiss_sort_data(division);
        let playoff_results = self.completed_playoff_results(division, 1001);
        let final_results = self.completed_playoff_results(division, 1002);
        live_build_seed_order_after_elimination_results(
            &standings,
            &playoff_results,
            &final_results,
        )
    }

    fn team_spirit_average(&self, team_id: i64, swiss_only: bool) -> f64 {
        let mut totals = Vec::new();

        for state in self.matches.values() {
            if state.possession.unwrap_or(0) < 3 {
                continue;
            }
            if swiss_only && state.match_type >= 1000 {
                continue;
            }

            for ((rated_team_id, submitted_by_team_id), submission) in &state.spirit_rows {
                if *rated_team_id == team_id && *submitted_by_team_id != team_id {
                    totals.push(submission.total as f64);
                }
            }
        }

        if totals.is_empty() {
            0.0
        } else {
            totals.iter().sum::<f64>() / totals.len() as f64
        }
    }

    fn as_live_sort_data(&self, metrics: &SortMetrics) -> TeamSortData {
        TeamSortData {
            team_id: metrics.team_id,
            name: self.team_name(metrics.team_id).to_string(),
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

    fn live_swiss_sort_data(&self, division: i64) -> Vec<TeamSortData> {
        self.swiss_summary(division)
            .iter()
            .map(|metrics| self.as_live_sort_data(metrics))
            .collect()
    }

    fn completed_playoff_results(
        &self,
        division: i64,
        match_type: i64,
    ) -> Vec<LivePlayedMatchResult> {
        let mut states: Vec<_> = self
            .matches
            .values()
            .filter(|state| {
                state.division == division
                    && state.match_type == match_type
                    && state.possession.unwrap_or(0) >= 3
            })
            .collect();
        states.sort_by_key(|state| state.id);

        states
            .into_iter()
            .map(|state| LivePlayedMatchResult {
                t1: state.t1_id,
                t2: state.t2_id,
                winner: if state.t1_score >= state.t2_score {
                    state.t1_id
                } else {
                    state.t2_id
                },
            })
            .collect()
    }

    fn match_history(&self, division: i64, swiss_only: bool) -> HashMap<i64, HashSet<i64>> {
        let mut history: HashMap<i64, HashSet<i64>> = HashMap::new();

        for state in self.matches.values() {
            if state.division != division || state.possession.unwrap_or(0) < 3 {
                continue;
            }
            if swiss_only && state.match_type >= 1000 {
                continue;
            }

            history.entry(state.t1_id).or_default().insert(state.t2_id);
            history.entry(state.t2_id).or_default().insert(state.t1_id);
        }

        history
    }

    fn team_ground_distribution_msd(&self, team_id: i64) -> Option<f64> {
        let mut ground_counts = [0_i64; 4];

        for state in self.matches.values().filter(|state| {
            state.possession.unwrap_or(0) >= 3 && (state.t1_id == team_id || state.t2_id == team_id)
        }) {
            let Some(ground_index) = parse_ground_index(&state.field_name) else {
                continue;
            };
            ground_counts[ground_index] += 1;
        }

        let total_matches: i64 = ground_counts.iter().sum();
        if total_matches == 0 {
            return None;
        }

        let ideal = total_matches as f64 / ground_counts.len() as f64;
        Some(
            ground_counts
                .iter()
                .map(|count| {
                    let diff = *count as f64 - ideal;
                    diff * diff
                })
                .sum::<f64>()
                / ground_counts.len() as f64,
        )
    }

    fn apply_player_event(&mut self, match_id: i64, player_id: Option<i64>, event_type: i64) {
        let Some(player_id) = player_id else {
            return;
        };

        let totals = self.players.entry(player_id).or_default();
        totals.matches.insert(match_id);
        match event_type {
            0 => totals.goals += 1,
            1 => totals.assists += 1,
            2 => totals.blocks += 1,
            3 => totals.turnovers += 1,
            _ => {}
        }
    }
}

fn match_duration_minutes(_division: i64, match_type: i64) -> i64 {
    if match_type >= 1000 { 75 } else { 65 }
}

fn parse_match_time(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").ok()
}

fn parse_ground_index(value: &str) -> Option<usize> {
    match value
        .chars()
        .filter(|char| char.is_ascii_digit())
        .collect::<String>()
        .as_str()
    {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        _ => None,
    }
}

fn canonical_gap_stage_start_times(division: i64, match_type: i64) -> Option<Vec<NaiveDateTime>> {
    Some(match (division, match_type) {
        (0, 1) => expand_gap_times(
            "2026-05-22",
            &[("06:00:00", 4), ("07:20:00", 3), ("10:00:00", 4)],
        ),
        (0, 2) => expand_gap_times(
            "2026-05-22",
            &[("12:40:00", 3), ("14:00:00", 4), ("15:20:00", 4)],
        ),
        (0, 3) => expand_gap_times(
            "2026-05-22",
            &[("18:10:00", 3), ("19:30:00", 4), ("20:50:00", 4)],
        ),
        (0, 4) => expand_gap_times(
            "2026-05-23",
            &[("06:00:00", 4), ("07:20:00", 3), ("10:00:00", 4)],
        ),
        (0, 5) => expand_gap_times(
            "2026-05-23",
            &[("12:40:00", 3), ("14:00:00", 4), ("15:20:00", 4)],
        ),
        (0, 6) => expand_gap_times(
            "2026-05-23",
            &[("18:10:00", 3), ("19:30:00", 4), ("20:50:00", 4)],
        ),
        (0, 1001) => expand_gap_times("2026-05-24", &[("06:15:00", 4)]),
        (0, 1002) => expand_gap_times(
            "2026-05-24",
            &[
                ("07:50:00", 1),
                ("09:25:00", 4),
                ("11:00:00", 3),
                ("12:35:00", 2),
                ("15:40:00", 1),
            ],
        ),
        (1, 1) => expand_gap_times("2026-05-22", &[("07:20:00", 1), ("08:40:00", 4)]),
        (1, 2) => expand_gap_times("2026-05-22", &[("11:20:00", 4), ("12:40:00", 1)]),
        (1, 3) => expand_gap_times("2026-05-22", &[("16:50:00", 4), ("18:10:00", 1)]),
        (1, 4) => expand_gap_times("2026-05-23", &[("07:20:00", 1), ("08:40:00", 4)]),
        (1, 5) => expand_gap_times("2026-05-23", &[("11:20:00", 4), ("12:40:00", 1)]),
        (1, 6) => expand_gap_times("2026-05-23", &[("16:50:00", 4), ("18:10:00", 1)]),
        (1, 1001) => expand_gap_times("2026-05-24", &[("07:50:00", 2)]),
        (1, 1002) => expand_gap_times(
            "2026-05-24",
            &[
                ("07:50:00", 1),
                ("11:00:00", 1),
                ("12:35:00", 2),
                ("14:10:00", 1),
            ],
        ),
        _ => return None,
    })
}

fn expand_gap_times(date: &str, times: &[(&str, usize)]) -> Vec<NaiveDateTime> {
    let mut expanded = Vec::new();

    for (time, repeat_count) in times {
        let value = parse_match_time(&format!("{date} {time}")).expect("gap time should parse");
        for _ in 0..*repeat_count {
            expanded.push(value);
        }
    }

    expanded
}
