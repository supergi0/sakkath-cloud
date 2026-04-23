mod full_tournament;
mod match_flow;

use super::config::RunConfig;
use super::harness::TestResult;
use super::reporting::ReportWriter;

pub(crate) async fn run_all(config: &RunConfig, reporter: &mut ReportWriter) -> TestResult {
    match_flow::run(config, reporter).await?;
    full_tournament::run(config, reporter).await
}