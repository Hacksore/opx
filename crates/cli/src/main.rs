#![allow(clippy::needless_return)]

mod config;
mod util;

use crate::util::{ensure_not_nested_opx, get_env_files, parse_cli_args, run_op_command};
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
    .with_writer(std::io::stderr)
    .try_init();
}

fn append_non_empty_lines(lines: &mut Vec<String>, message: impl AsRef<str>) {
  lines.extend(
    message
      .as_ref()
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(str::to_string),
  );
}

fn append_prefixed_non_empty_lines(
  lines: &mut Vec<String>,
  prefix: &str,
  message: impl AsRef<str>,
) {
  lines.extend(
    message
      .as_ref()
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(|line| format!("{prefix}{line}")),
  );
}

fn format_error_lines(error: &anyhow::Error) -> Vec<String> {
  let mut lines = vec![];
  let causes = error.chain().skip(1).collect::<Vec<_>>();

  append_non_empty_lines(&mut lines, error.to_string());

  if !causes.is_empty() {
    lines.push("caused by:".to_string());

    for cause in causes {
      append_prefixed_non_empty_lines(&mut lines, "  - ", cause.to_string());
    }
  }

  lines
}

fn log_error(error: &anyhow::Error) {
  for line in format_error_lines(error) {
    error!("{line}");
  }
}

fn default_script_args(package_manager: &str, default_script: &str) -> Vec<String> {
  if package_manager == "npm" {
    return vec!["run".to_string(), default_script.to_string()];
  }

  vec![default_script.to_string()]
}

fn main() -> ExitCode {
  init_logging();

  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      log_error(&error);
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
  ensure_not_nested_opx()?;

  // NOTE: this is expensive
  let env_files = get_env_files(parsed_args.selected_env.as_deref())?;

  // read config from the local director if possible
  let config = OpxConfig::new()?;
  let package_manager = config.get_package_manager();

  let op_args = match parsed_args.command_args {
    args if args.is_empty() => default_script_args(package_manager, config.get_default_script()),
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

#[cfg(test)]
mod tests {
  use super::default_script_args;
  use anyhow::anyhow;

  #[test]
  fn default_script_uses_npm_run_for_npm() {
    assert_eq!(default_script_args("npm", "dev"), vec!["run", "dev"]);
  }

  #[test]
  fn default_script_uses_direct_script_for_other_package_managers() {
    assert_eq!(default_script_args("pnpm", "dev"), vec!["dev"]);
    assert_eq!(default_script_args("yarn", "server"), vec!["server"]);
  }

  #[test]
  fn error_formatting_splits_multiline_messages_for_logger_output() {
    let error =
      anyhow!("Failed to run.\n\nhint: Try again.").context("Command failed.\n\nwhere: tests");

    let lines = super::format_error_lines(&error);

    assert_eq!(
      lines,
      vec![
        "Command failed.",
        "where: tests",
        "caused by:",
        "  - Failed to run.",
        "  - hint: Try again.",
      ]
    );
    assert!(lines.iter().all(|line| !line.contains('\n')));
  }
}
