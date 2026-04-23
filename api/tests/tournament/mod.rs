mod assertions;
mod config;
mod harness;
mod model;
mod reporting;
mod scenarios;
mod simulation;
mod support;

use self::config::RunConfig;
use self::harness::TestResult;
use self::reporting::ReportWriter;

pub fn cli_main() -> TestResult {
    let config = RunConfig::from_process_args(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let mut reporter = ReportWriter::new(&config)?;
        reporter.run_banner()?;
        scenarios::run_all(&config, &mut reporter).await?;
        reporter.finish()
    })
}