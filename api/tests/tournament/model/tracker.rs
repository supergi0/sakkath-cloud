use super::api_types::{
    MatchDetailResponse, ScheduleMatchResponse, ScoreConfirmRowResponse, SpiritScoreRowResponse,
    TeamResponse,
};
use super::pairings::{
    ExpectedPlayoffMatch, ExpectedSwissRound, SortMetrics, build_playoff_round_one,
    build_playoff_round_two, build_scoring_groups, generate_round_pairings, naive_pairings,
    sort_metrics,
};
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
}

impl TournamentTracker {
    pub(crate) fn new(teams: &[TeamResponse]) -> Self {
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

    pub(crate) fn match_ids_for_team_and_type(
        &self,
        team_id: i64,
        match_type: i64,
    ) -> HashSet<i64> {
        self.matches
            .values()
            .filter(|state| {
                (state.t1_id == team_id || state.t2_id == team_id) && state.match_type == match_type
            })
            .map(|state| state.id)
            .collect()
    }

    pub(crate) fn swiss_summary(&self, division: i64) -> Vec<SortMetrics> {
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
                    && state.match_type < 1000
                    && state.possession.unwrap_or(0) >= 3
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
            let spirit_avg = self.team_spirit_average(team_id, true);

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

        sort_metrics(&mut metrics);
        metrics
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

        let pairings = generate_round_pairings(&standings, &history);

        ExpectedSwissRound {
            pairings,
            naive_had_rematch,
        }
    }

    pub(crate) fn expected_playoff_round_one(&self, division: i64) -> Vec<ExpectedPlayoffMatch> {
        build_playoff_round_one(&self.swiss_summary(division))
    }

    pub(crate) fn expected_playoff_round_two(&self, division: i64) -> Vec<ExpectedPlayoffMatch> {
        let round_one = self.expected_playoff_round_one(division);
        build_playoff_round_two(&round_one, self)
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
