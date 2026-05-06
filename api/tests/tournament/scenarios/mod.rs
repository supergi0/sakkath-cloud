mod full_tournament;
mod match_flow;

use super::config::RunConfig;
use super::harness::TestResult;
use super::reporting::ReportWriter;

pub(crate) use self::full_tournament::{FullTournamentSummary, run_with_summary as run_full_tournament_with_summary};

pub(crate) async fn run_all(config: &RunConfig, reporter: &mut ReportWriter) -> TestResult {
    match_flow::run(config, reporter).await?;
    full_tournament::run(config, reporter).await
}
