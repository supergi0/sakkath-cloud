use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LoginResponse {
    pub(crate) token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MutationResponse {
    pub(crate) success: bool,
    pub(crate) t1_score: Option<i64>,
    pub(crate) t2_score: Option<i64>,
    pub(crate) possession: Option<i64>,
    pub(crate) finalized: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TeamResponse {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) division: i64,
    pub(crate) init_rank: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TeamDetailResponse {
    pub(crate) id: i64,
    pub(crate) games_played: i64,
    pub(crate) wins: i64,
    pub(crate) losses: i64,
    pub(crate) draws: i64,
    pub(crate) spirit_avg: f64,
    pub(crate) current_rank: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MatchPlayerResponse {
    pub(crate) id: i64,
    pub(crate) team_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MatchEventResponse {
    pub(crate) id: i64,
    pub(crate) player_id: Option<i64>,
    pub(crate) event_type: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MatchDetailResponse {
    pub(crate) id: i64,
    pub(crate) t1_id: i64,
    pub(crate) t2_id: i64,
    pub(crate) t1_score: i64,
    pub(crate) t2_score: i64,
    pub(crate) t1_spirit: Option<i64>,
    pub(crate) t2_spirit: Option<i64>,
    pub(crate) possession: Option<i64>,
    pub(crate) match_type: i64,
    pub(crate) reporting_enabled: bool,
    pub(crate) players: Vec<MatchPlayerResponse>,
    pub(crate) events: Vec<MatchEventResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TeamMatchResponse {
    pub(crate) id: i64,
    pub(crate) t1_score: i64,
    pub(crate) t2_score: i64,
    pub(crate) t1_spirit: Option<i64>,
    pub(crate) t2_spirit: Option<i64>,
    pub(crate) match_type: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UpcomingMatchResponse {
    pub(crate) id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReportingRoundSettingResponse {
    pub(crate) round_key: i64,
    pub(crate) label: String,
    pub(crate) is_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SpiritScoreRowResponse {
    pub(crate) match_id: i64,
    pub(crate) team_id: i64,
    pub(crate) total: i64,
    pub(crate) mvp_player_id: Option<i64>,
    pub(crate) msp_player_id: Option<i64>,
    pub(crate) submitted_by_team_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ScoreConfirmRowResponse {
    pub(crate) match_id: i64,
    pub(crate) team_id: i64,
    pub(crate) t1_score: i64,
    pub(crate) t2_score: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StandingRowResponse {
    pub(crate) id: i64,
    pub(crate) wins: i64,
    pub(crate) losses: i64,
    pub(crate) draws: i64,
    pub(crate) points_for: i64,
    pub(crate) points_against: i64,
    pub(crate) spirit_avg: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ScheduleMatchResponse {
    pub(crate) id: i64,
    pub(crate) t1_id: i64,
    pub(crate) t2_id: i64,
    pub(crate) t1_score: i64,
    pub(crate) t2_score: i64,
    pub(crate) t1_spirit: Option<i64>,
    pub(crate) t2_spirit: Option<i64>,
    pub(crate) possession: Option<i64>,
    pub(crate) match_type: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ScheduleGridResponse {
    pub(crate) rows: Vec<ScheduleGridRowResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ScheduleGridRowResponse {
    pub(crate) cells: Vec<ScheduleGridCellResponse>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScheduleGridCellResponse {
    pub(crate) match_id: Option<i64>,
    pub(crate) t1_seed_rank: Option<i64>,
    pub(crate) t2_seed_rank: Option<i64>,
    pub(crate) t1_score: Option<i64>,
    pub(crate) t2_score: Option<i64>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ScheduleGridCellWire {
    match_id: Option<i64>,
    #[serde(default)]
    t1_seed_rank: Option<i64>,
    #[serde(default)]
    t2_seed_rank: Option<i64>,
    #[serde(default)]
    t1_score: Option<i64>,
    #[serde(default)]
    t2_score: Option<i64>,
    #[serde(default)]
    data: Option<[i64; 5]>,
    #[serde(default)]
    seed_ranks: Option<[i64; 2]>,
    status: String,
}

impl<'de> Deserialize<'de> for ScheduleGridCellResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ScheduleGridCellWire::deserialize(deserializer)?;
        let compact_data = wire.data;
        let compact_seeds = wire.seed_ranks;

        Ok(Self {
            match_id: wire.match_id,
            t1_seed_rank: wire
                .t1_seed_rank
                .or_else(|| compact_seeds.map(|seed_ranks| seed_ranks[0])),
            t2_seed_rank: wire
                .t2_seed_rank
                .or_else(|| compact_seeds.map(|seed_ranks| seed_ranks[1])),
            t1_score: wire.t1_score.or_else(|| compact_data.map(|data| data[3])),
            t2_score: wire.t2_score.or_else(|| compact_data.map(|data| data[4])),
            status: wire.status,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StatsResponse {
    pub(crate) teams: i64,
    pub(crate) players: i64,
    pub(crate) points: i64,
    pub(crate) games: i64,
    pub(crate) fields: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PlayerStatResponse {
    pub(crate) id: i64,
    pub(crate) goals: i64,
    pub(crate) assists: i64,
    pub(crate) blocks: i64,
    pub(crate) turnovers: i64,
    pub(crate) matches: i64,
}
