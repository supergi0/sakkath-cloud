use super::model::{ScheduleMatchResponse, TournamentTracker};
use rand::{Rng, RngCore, SeedableRng, rngs::StdRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutcomeKind {
    T1Win,
    T2Win,
    Draw,
}

impl OutcomeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::T1Win => "t1 win",
            Self::T2Win => "t2 win",
            Self::Draw => "draw",
        }
    }
}

pub(crate) struct TournamentSimulation {
    rng: StdRng,
}

impl TournamentSimulation {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub(crate) fn swiss_outcome(
        &mut self,
        round: i64,
        division: i64,
        schedule_match: &ScheduleMatchResponse,
        tracker: &TournamentTracker,
    ) -> OutcomeKind {
        if self.percent() < 5 {
            return OutcomeKind::Draw;
        }

        let seed_map = tracker.standings_rank_map(division);
        let t1_seed = seed_map
            .get(&schedule_match.t1_id)
            .copied()
            .unwrap_or_else(|| tracker.init_rank(schedule_match.t1_id));
        let t2_seed = seed_map
            .get(&schedule_match.t2_id)
            .copied()
            .unwrap_or_else(|| tracker.init_rank(schedule_match.t2_id));

        let favorite_is_t1 = t1_seed <= t2_seed;
        let gap = t1_seed.abs_diff(t2_seed).min(4) as u8;
        let round_bias = if round >= 4 { 4 } else { 0 };
        let division_bias = if division == 0 { 2 } else { 0 };
        self.biased_winner(favorite_is_t1, 56 + gap * 7 + round_bias + division_bias)
    }

    pub(crate) fn playoff_round_two_outcome(
        &mut self,
        tracker: &TournamentTracker,
        schedule_match: &ScheduleMatchResponse,
    ) -> OutcomeKind {
        let t1_rank = tracker.init_rank(schedule_match.t1_id);
        let t2_rank = tracker.init_rank(schedule_match.t2_id);
        self.playoff_outcome_from_seeds(t1_rank, t2_rank, 60, 6)
    }

    pub(crate) fn playoff_outcome_from_seeds(
        &mut self,
        seed_a: i64,
        seed_b: i64,
        favorite_base: u8,
        gap_factor: u8,
    ) -> OutcomeKind {
        let favorite_is_t1 = seed_a <= seed_b;
        let gap = seed_a.abs_diff(seed_b).min(4) as u8;
        self.biased_winner(favorite_is_t1, favorite_base + gap * gap_factor)
    }

    pub(crate) fn target_scores(
        &mut self,
        match_type: i64,
        current_t1: i64,
        current_t2: i64,
        outcome: OutcomeKind,
    ) -> (i64, i64) {
        match outcome {
            OutcomeKind::T1Win => {
                let winner_score = self.winner_score(match_type, current_t1.max(current_t2));
                let loser_score = self.loser_score(current_t2, winner_score);
                (winner_score, loser_score)
            }
            OutcomeKind::T2Win => {
                let winner_score = self.winner_score(match_type, current_t1.max(current_t2));
                let loser_score = self.loser_score(current_t1, winner_score);
                (loser_score, winner_score)
            }
            OutcomeKind::Draw => {
                let draw_floor = if match_type >= 1000 { 10 } else { 8 };
                let draw_ceiling = if match_type >= 1000 { 13 } else { 11 };
                let target = self
                    .rng
                    .random_range(draw_floor..=draw_ceiling)
                    .max(current_t1.max(current_t2));
                (target, target)
            }
        }
    }

    fn biased_winner(&mut self, favorite_is_t1: bool, favorite_percent: u8) -> OutcomeKind {
        let roll = self.percent();
        if favorite_is_t1 {
            if roll < favorite_percent {
                OutcomeKind::T1Win
            } else {
                OutcomeKind::T2Win
            }
        } else if roll < favorite_percent {
            OutcomeKind::T2Win
        } else {
            OutcomeKind::T1Win
        }
    }

    fn percent(&mut self) -> u8 {
        (self.rng.next_u64() % 100) as u8
    }

    fn winner_score(&mut self, match_type: i64, current_max: i64) -> i64 {
        let winner_floor = if match_type >= 1000 { 10 } else { 8 };
        let winner_ceiling = if match_type >= 1000 { 13 } else { 12 };

        self.rng
            .random_range(winner_floor..=winner_ceiling)
            .max(current_max + 1)
    }

    fn loser_score(&mut self, current_score: i64, winner_score: i64) -> i64 {
        let desired_margin = self.rng.random_range(1..=4) as i64;
        let preferred_score = winner_score.saturating_sub(desired_margin);
        preferred_score.clamp(current_score, winner_score - 1)
    }
}
