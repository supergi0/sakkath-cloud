use serde::Serialize;
use tokio::sync::broadcast;

const LIVE_UPDATES_BUFFER: usize = 256;

#[derive(Clone)]
pub struct LiveUpdates {
    sender: broadcast::Sender<LiveUpdate>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveUpdate {
    MatchUpdated { match_id: i64 },
    ReportingRoundsUpdated,
}

impl LiveUpdate {
    pub fn matches_match_id(&self, match_id: i64) -> bool {
        match self {
            Self::MatchUpdated {
                match_id: updated_match_id,
            } => *updated_match_id == match_id,
            Self::ReportingRoundsUpdated => false,
        }
    }
}

impl LiveUpdates {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(LIVE_UPDATES_BUFFER);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveUpdate> {
        self.sender.subscribe()
    }

    pub fn publish_match_updated(&self, match_id: i64) {
        let _ = self.sender.send(LiveUpdate::MatchUpdated { match_id });
    }

    pub fn publish_reporting_rounds_updated(&self) {
        let _ = self.sender.send(LiveUpdate::ReportingRoundsUpdated);
    }
}

impl Default for LiveUpdates {
    fn default() -> Self {
        Self::new()
    }
}
