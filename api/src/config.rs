pub mod swiss_pairing {
    pub const DEFAULT_BAND_RADIUS: usize = 0;
    pub const DEFAULT_BAND_WEIGHT: i64 = 0;
    pub const DISABLED_LOOKAHEAD_SCENARIOS: usize = 0;
    pub const SINGLE_CANDIDATE_LIMIT: usize = 1;
    pub const SINGLE_EXPLORATION_LIMIT: usize = 1;

    pub const ROUND4_BAND_RADIUS: usize = 3;
    pub const ROUND4_BAND_WEIGHT: i64 = 1;

    pub const LATE_ROUND_BAND_RADIUS: usize = 3;
    pub const LATE_ROUND_BAND_WEIGHT: i64 = 10;

    pub const ROUND5_LOOKAHEAD_SCENARIOS: usize = 36;
    pub const ROUND6_LOOKAHEAD_SCENARIOS: usize = 36;

    pub const LOOKAHEAD_CANDIDATE_LIMIT: usize = 192;
    pub const LOOKAHEAD_EXPLORATION_LIMIT: usize = 384;

    pub const LOOKAHEAD_BOUNDARY_TARGETS: [(usize, usize); 4] =
        [(4, 5), (3, 6), (8, 9), (7, 10)];
}