#![allow(clippy::needless_return)]

mod config;
mod util;

use crate::util::{ensure_not_nested_opx, get_env_files, parse_cli_args, run_op_command};
use anstyle::{AnsiColor, Color, Style};
use anyhow::{bail, Context, Result};
use config::OpxConfig;
use dirs::home_dir;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tracing::info;
use tracing_subscriber::EnvFilter;

const OPX_VERSION: &str = env!("CARGO_PKG_VERSION");
const STYLE_DIM: Style = Style::new().dimmed();
const STYLE_ERROR_SUMMARY: Style = Color::Ansi(AnsiColor::BrightRed).on_default().bold();
const STYLE_BRIGHT_RED: Style = Color::Ansi(AnsiColor::BrightRed).on_default();
const STYLE_ERROR_CODE: Style = Color::Ansi(AnsiColor::BrightBlack)
  .on_default()
  .bg_color(Some(Color::Ansi(AnsiColor::BrightRed)));
const STYLE_ERROR_CODE_BRACKET: Style = Color::Ansi(AnsiColor::BrightRed)
  .on_default()
  .bg_color(Some(Color::Ansi(AnsiColor::BrightRed)));
const STYLE_CYAN_BADGE: Style = AnsiColor::Black
  .on_default()
  .bg_color(Some(Color::Ansi(AnsiColor::Cyan)))
  .effects(anstyle::Effects::BOLD);
const STYLE_YELLOW_BADGE: Style = AnsiColor::Black
  .on_default()
  .bg_color(Some(Color::Ansi(AnsiColor::Yellow)))
  .effects(anstyle::Effects::BOLD);

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

fn styled(style: Style, message: impl AsRef<str>) -> String {
  format!("{style}{}{style:#}", message.as_ref())
}

fn non_empty_lines(message: impl AsRef<str>) -> Vec<String> {
  message
    .as_ref()
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(str::to_string)
    .collect()
}

fn known_error_label(line: &str) -> Option<(&str, &str)> {
  let (label, value) = line.split_once(':')?;

  match label {
    "command" | "hint" | "selected" | "status" | "value" | "where" | "why" => {
      Some((label, value.trim_start()))
    }
    _ => None,
  }
}

fn label_style(label: &str) -> Style {
  match label {
    "hint" => STYLE_YELLOW_BADGE,
    _ => STYLE_CYAN_BADGE,
  }
}

fn format_error_detail_line(line: &str, indent: &str) -> Option<(String, bool)> {
  if let Some((label, value)) = known_error_label(line) {
    if label == "where" {
      return None;
    }

    if label == "why" {
      return Some((format!("{indent}{value}"), false));
    }

    return Some((
      format!(
        "{indent}{} {value}",
        styled(label_style(label), format!(" {label} "))
      ),
      label == "hint",
    ));
  }

  Some((format!("{indent}{}", styled(STYLE_DIM, line)), false))
}

fn append_formatted_error_lines(message: impl AsRef<str>, output: &mut String, indent: &str) {
  let lines = non_empty_lines(message);
  let Some((summary, details)) = lines.split_first() else {
    return;
  };

  output.push_str(&styled(STYLE_ERROR_SUMMARY, summary));

  let detail_lines = details
    .iter()
    .filter_map(|line| format_error_detail_line(line, indent))
    .collect::<Vec<_>>();

  if detail_lines.is_empty() {
    return;
  }

  output.push_str("\n\n");

  for (index, (line, needs_gap_before)) in detail_lines.iter().enumerate() {
    if index > 0 {
      output.push('\n');

      if *needs_gap_before {
        output.push('\n');
      }
    }

    output.push_str(line);
  }
}

fn error_summary(error: &anyhow::Error) -> String {
  non_empty_lines(error.to_string())
    .into_iter()
    .next()
    .unwrap_or_else(|| "Unexpected opx error.".to_string())
}

fn error_code_for_summary(summary: &str) -> &'static str {
  if summary.starts_with("Invalid OPX_DEPTH value") {
    return "EOPX_INVALID_DEPTH";
  }

  if summary.starts_with("Invalid environment name") {
    return "EOPX_INVALID_ENV";
  }

  match summary {
    "Failed to determine the current working directory." => "EOPX_CURRENT_DIR",
    "Failed to determine the current working directory while scanning for .env files." => {
      "EOPX_ENV_SCAN_CURRENT_DIR"
    }
    "Failed to parse opx.defaultCommand." => "EOPX_DEFAULT_COMMAND_PARSE",
    "Invalid opx.defaultCommand." => "EOPX_DEFAULT_COMMAND_INVALID",
    "Refusing to run from your home directory." => "EOPX_HOME_DIRECTORY",
    "Failed to find package.json." => "EOPX_PACKAGE_JSON_NOT_FOUND",
    "Failed to read package.json." => "EOPX_PACKAGE_JSON_READ",
    "Failed to parse package.json as JSON." => "EOPX_PACKAGE_JSON_PARSE",
    "Refusing to run opx inside an opx-managed command." => "EOPX_NESTED_INVOCATION",
    "Missing environment name." => "EOPX_MISSING_ENV",
    "Multiple environments were selected." => "EOPX_MULTIPLE_ENVS",
    "Missing environment after --env." => "EOPX_MISSING_ENV",
    "Missing command to run." => "EOPX_MISSING_COMMAND",
    "Failed to start 1Password CLI `op`." => "EOPX_OP_NOT_FOUND",
    "Failed to start command through `op run`." => "EOPX_OP_START_FAILED",
    "Failed while waiting for the command launched by `op run`." => "EOPX_OP_WAIT_FAILED",
    "The command launched by opx exited unsuccessfully." => "EOPX_COMMAND_FAILED",
    _ => "EOPX_ERROR",
  }
}

fn format_error_code_footer(error_code: &str) -> String {
  format!(
    "{}{}{} {}",
    styled(STYLE_ERROR_CODE_BRACKET, "["),
    styled(STYLE_ERROR_CODE, error_code),
    styled(STYLE_ERROR_CODE_BRACKET, "]"),
    styled(STYLE_BRIGHT_RED, "Command failed with exit code 1."),
  )
}

fn format_error_message(error: &anyhow::Error) -> String {
  let mut message = String::new();
  let causes = error.chain().skip(1).collect::<Vec<_>>();
  let summary = error_summary(error);
  let error_code = error_code_for_summary(&summary);

  append_formatted_error_lines(error.to_string(), &mut message, "  ");

  if !causes.is_empty() {
    message.push_str("\n\n");
    message.push_str(&styled(STYLE_DIM, "caused by:"));

    for cause in causes {
      let mut cause_message = String::new();
      append_formatted_error_lines(cause.to_string(), &mut cause_message, "    ");

      if !cause_message.is_empty() {
        message.push_str("\n  - ");
        message.push_str(&cause_message);
      }
    }
  }

  message.push_str("\n\n");
  message.push_str(&format_error_code_footer(error_code));

  message
}

fn log_error(error: &anyhow::Error) {
  eprintln!("{}", format_error_message(error));
}

fn log_startup_banner() {
  info!("Starting opx v{OPX_VERSION}");
}

fn normalized_path(path: &Path) -> PathBuf {
  path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_home_directory(path: &Path) -> bool {
  let normalized_current_dir = normalized_path(path);

  home_dir()
    .as_deref()
    .is_some_and(|home_path| normalized_path(home_path) == normalized_current_dir)
    || ["HOME", "USERPROFILE"]
      .into_iter()
      .filter_map(env::var_os)
      .map(PathBuf::from)
      .any(|home_path| normalized_path(&home_path) == normalized_current_dir)
}

fn default_script_args(package_manager: &str, default_script: &str) -> Vec<String> {
  if package_manager == "npm" {
    return vec!["run".to_string(), default_script.to_string()];
  }

  vec![default_script.to_string()]
}

fn package_manager_command(package_manager: &str, args: Vec<String>) -> Vec<String> {
  let mut command = vec![package_manager.to_string()];
  command.extend(args);
  command
}

fn default_command_args(config: &OpxConfig) -> Vec<String> {
  if let Some(default_command) = config.get_default_command() {
    return default_command.to_vec();
  }

  package_manager_command(
    config.get_package_manager(),
    default_script_args(config.get_package_manager(), config.get_default_script()),
  )
}

fn main() -> ExitCode {
  init_logging();
  log_startup_banner();

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

  // Avoid scanning a user's whole home directory when opx was run outside a project.
  if is_home_directory(&current_dir) {
    bail!(
      "Refusing to run from your home directory.\n\nwhere: {}\nwhy: opx must run from a JavaScript project directory.\nhint: Change into your project directory, then run opx again.",
      current_dir.display()
    );
  }

  let cli_args = env::args().skip(1).collect::<Vec<String>>();
  let parsed_args = parse_cli_args(cli_args)?;
  ensure_not_nested_opx()?;

  // NOTE: this is expensive
  let env_files = get_env_files(parsed_args.selected_env.as_deref())?;

  // read config from the local director if possible
  let config = OpxConfig::new()?;
  let op_command = match parsed_args.command_args {
    args if args.is_empty() => default_command_args(&config),
    args => package_manager_command(config.get_package_manager(), args),
  };

  run_op_command(env_files, op_command, parsed_args.selected_env.as_deref())?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::{default_command_args, default_script_args, package_manager_command};
  use crate::config::OpxConfig;
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
  fn package_manager_command_prepends_package_manager() {
    assert_eq!(
      package_manager_command("pnpm", vec!["db:push".to_string()]),
      vec!["pnpm", "db:push"]
    );
  }

  #[test]
  fn default_command_uses_raw_default_command_when_configured() {
    let config = OpxConfig {
      package_manager: "pnpm".to_string(),
      default_script: "dev".to_string(),
      default_command: Some(vec!["next".to_string(), "dev".to_string()]),
    };

    assert_eq!(default_command_args(&config), vec!["next", "dev"]);
  }

  #[test]
  fn default_command_falls_back_to_package_script() {
    let config = OpxConfig {
      package_manager: "npm".to_string(),
      default_script: "start".to_string(),
      default_command: None,
    };

    assert_eq!(default_command_args(&config), vec!["npm", "run", "start"]);
  }

  #[test]
  fn error_formatting_preserves_multiline_messages_for_single_logger_output() {
    let error =
      anyhow!("Failed to run.\n\nhint: Try again.").context("Command failed.\n\nwhere: tests");

    let message = super::format_error_message(&error);

    assert!(message.contains("\x1b[1m\x1b[91mCommand failed.\x1b[0m"));
    assert!(message.contains("\x1b[2mcaused by:\x1b[0m"));
    assert!(message.contains("\x1b[1m\x1b[91mFailed to run.\x1b[0m"));
    assert!(message.contains("\x1b[1m\x1b[30m\x1b[43m hint"));
    assert!(message.contains("\x1b[91m\x1b[101m[\x1b[0m"));
    assert!(message.contains("\x1b[90m\x1b[101mEOPX_ERROR\x1b[0m"));
    assert!(message.contains("\x1b[91m\x1b[101m]\x1b[0m"));
    assert!(message.contains("\x1b[91mCommand failed with exit code 1.\x1b[0m"));
    assert!(!message.contains("where"));
    assert!(!message.contains("where:"));
    assert!(!message.contains("hint:"));
    assert!(message.contains("\n\n"));
  }

  #[test]
  fn error_codes_cover_known_error_summaries() {
    let cases = [
      (
        "Failed to determine the current working directory.",
        "EOPX_CURRENT_DIR",
      ),
      (
        "Failed to determine the current working directory while scanning for .env files.",
        "EOPX_ENV_SCAN_CURRENT_DIR",
      ),
      (
        "Failed to parse opx.defaultCommand.",
        "EOPX_DEFAULT_COMMAND_PARSE",
      ),
      (
        "Invalid opx.defaultCommand.",
        "EOPX_DEFAULT_COMMAND_INVALID",
      ),
      (
        "Refusing to run from your home directory.",
        "EOPX_HOME_DIRECTORY",
      ),
      (
        "Failed to find package.json.",
        "EOPX_PACKAGE_JSON_NOT_FOUND",
      ),
      ("Failed to read package.json.", "EOPX_PACKAGE_JSON_READ"),
      (
        "Failed to parse package.json as JSON.",
        "EOPX_PACKAGE_JSON_PARSE",
      ),
      ("Invalid OPX_DEPTH value `abc`.", "EOPX_INVALID_DEPTH"),
      (
        "Refusing to run opx inside an opx-managed command.",
        "EOPX_NESTED_INVOCATION",
      ),
      ("Missing environment name.", "EOPX_MISSING_ENV"),
      ("Invalid environment name `prod!`.", "EOPX_INVALID_ENV"),
      ("Multiple environments were selected.", "EOPX_MULTIPLE_ENVS"),
      ("Missing environment after --env.", "EOPX_MISSING_ENV"),
      ("Missing command to run.", "EOPX_MISSING_COMMAND"),
      ("Failed to start 1Password CLI `op`.", "EOPX_OP_NOT_FOUND"),
      (
        "Failed to start command through `op run`.",
        "EOPX_OP_START_FAILED",
      ),
      (
        "Failed while waiting for the command launched by `op run`.",
        "EOPX_OP_WAIT_FAILED",
      ),
      (
        "The command launched by opx exited unsuccessfully.",
        "EOPX_COMMAND_FAILED",
      ),
    ];

    for (summary, code) in cases {
      assert_eq!(super::error_code_for_summary(summary), code);
    }
  }
}
