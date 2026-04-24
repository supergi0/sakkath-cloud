use super::model::{ExpectedPlayoffMatch, ScheduleMatchResponse, TournamentTracker};
use rand::{RngCore, SeedableRng, rngs::StdRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutcomeKind {
    T1Win,
    T2Win,
}

impl OutcomeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::T1Win => "t1 win",
            Self::T2Win => "t2 win",
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

    pub(crate) fn playoff_round_one_outcome(
        &mut self,
        expected_match: &ExpectedPlayoffMatch,
    ) -> OutcomeKind {
        let favorite_is_t1 = expected_match.seed_a <= expected_match.seed_b;
        let gap = expected_match.seed_a.abs_diff(expected_match.seed_b).min(4) as u8;
        self.biased_winner(favorite_is_t1, 62 + gap * 8)
    }

    pub(crate) fn playoff_round_two_outcome(
        &mut self,
        tracker: &TournamentTracker,
        schedule_match: &ScheduleMatchResponse,
    ) -> OutcomeKind {
        let t1_rank = tracker.init_rank(schedule_match.t1_id);
        let t2_rank = tracker.init_rank(schedule_match.t2_id);
        let favorite_is_t1 = t1_rank <= t2_rank;
        let gap = t1_rank.abs_diff(t2_rank).min(4) as u8;
        self.biased_winner(favorite_is_t1, 60 + gap * 6)
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
}
