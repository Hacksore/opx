#![allow(clippy::needless_return)]

mod config;
mod util;

use crate::util::{get_env_files, parse_cli_args, run_op_command};
use anyhow::{Context, Result};
use config::OpxConfig;
use dirs::home_dir;
use std::env;
use std::process::ExitCode;
use tracing::{error, warn};
use tracing_subscriber::EnvFilter;

fn init_logging() {
  let filter = EnvFilter::try_from_env("OPX_LOG").unwrap_or_else(|_| EnvFilter::new("opx=info"));

  let _ = tracing_subscriber::fmt()
    .with_env_filter(filter)
    .with_ansi(true)
    .with_target(false)
    .without_time()
    .compact()
    .try_init();
}

fn format_error(error: &anyhow::Error) -> String {
  let mut message = error.to_string();
  let causes = error.chain().skip(1).collect::<Vec<_>>();

  if !causes.is_empty() {
    message.push_str("\n\ncaused by:");

    for cause in causes {
      message.push_str(&format!("\n  - {cause}"));
    }
  }

  message
}

fn main() -> ExitCode {
  init_logging();

  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      error!("{}", format_error(&error));
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<()> {
  let current_dir = env::current_dir().context(
    "Failed to determine the current working directory.\n\nhint: Run opx from a project directory that still exists on disk.",
  )?;

  // if they are in their home dir then tell them to go to a project
  if Some(current_dir.as_path()) == home_dir().as_deref() {
    warn!("You are in your home directory. Please go to a project directory.");
    return Ok(());
  }

  let cli_args = env::args().skip(1).collect::<Vec<String>>();
  let parsed_args = parse_cli_args(cli_args)?;

  // NOTE: this is expensive
  let env_files = get_env_files(parsed_args.selected_env.as_deref())?;

  // read config from the local director if possible
  let config = OpxConfig::new()?;
  let package_manager = config.get_package_manager();

  // TODO: default command for now is start but this should be configurable
  let op_args = match parsed_args.command_args {
    args if args.is_empty() => vec!["start".to_string()],
    args => args,
  };

  run_op_command(
    env_files,
    op_args,
    package_manager,
    parsed_args.selected_env.as_deref(),
  )?;

  Ok(())
}
