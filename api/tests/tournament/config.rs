use super::harness::TestResult;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct RunConfig {
    pub(crate) itr: u64,
    pub(crate) seed: u64,
    pub(crate) log_dir: PathBuf,
}

impl RunConfig {
    pub(crate) fn from_itr(itr: u64) -> TestResult<Self> {
        if itr == 0 {
            return Err("itr must be greater than zero".into());
        }

        Ok(Self::build(itr))
    }

    pub(crate) fn from_process_args<I>(args: I) -> TestResult<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut itr = env::var("ITR")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1);

        let mut args = args.into_iter().peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--itr" | "itr" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing value after itr".to_string())?;
                    itr = parse_itr(&value)?;
                }
                "--nocapture" | "--show-output" => {}
                "--color" => {
                    let _ = args.next();
                }
                _ if arg.starts_with("--itr=") => {
                    itr = parse_itr(arg.trim_start_matches("--itr="))?;
                }
                _ if arg.starts_with("itr=") => {
                    itr = parse_itr(arg.trim_start_matches("itr="))?;
                }
                _ => {}
            }
        }

        Ok(Self::build(itr))
    }

    pub(crate) fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("api crate should have a repo root parent")
            .to_path_buf()
    }

    pub(crate) fn scenario_label(&self, scenario: &str) -> String {
        format!("{scenario}-itr{}", self.itr)
    }

    pub(crate) fn scenario_seed(&self, scenario: &str) -> u64 {
        scenario.bytes().fold(self.seed, |seed, byte| {
            seed.rotate_left(5) ^ u64::from(byte)
        })
    }

    fn build(itr: u64) -> Self {
        let seed = 0x5A_CC_A7_u64 ^ itr.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let log_dir = Self::repo_root()
            .join("logs")
            .join(format!("test_tournament_itr{itr}"));

        Self { itr, seed, log_dir }
    }
}

fn parse_itr(value: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let itr = value.parse::<u64>()?;
    if itr == 0 {
        return Err("itr must be greater than zero".into());
    }
    Ok(itr)
}
