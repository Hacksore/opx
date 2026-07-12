#![allow(clippy::needless_return)]

mod config;
mod util;

use crate::util::{ensure_not_nested_opx, get_env_files, parse_cli_args, run_op_command};
use anstyle::{AnsiColor, Color, Style};
use anyhow::{Context, Result};
use config::OpxConfig;
use dirs::home_dir;
use std::env;
use std::process::ExitCode;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

const OPX_VERSION: &str = env!("CARGO_PKG_VERSION");
const STYLE_BOLD: Style = Style::new().bold();
const STYLE_DIM: Style = Style::new().dimmed();
const STYLE_RED: Style = Color::Ansi(AnsiColor::Red).on_default();
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

  output.push_str(&styled(STYLE_BOLD, summary));

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

fn format_error_message(error: &anyhow::Error) -> String {
  let mut message = String::new();
  let causes = error.chain().skip(1).collect::<Vec<_>>();

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

  message
}

fn log_error(error: &anyhow::Error) {
  eprintln!(
    "{} {}",
    styled(STYLE_RED, "ERROR"),
    format_error_message(error)
  );
}

fn log_startup_banner() {
  info!("Starting opx v{OPX_VERSION}");
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

    assert!(message.contains("\x1b[1mCommand failed.\x1b[0m"));
    assert!(message.contains("\x1b[2mcaused by:\x1b[0m"));
    assert!(message.contains("\x1b[1mFailed to run.\x1b[0m"));
    assert!(message.contains("\x1b[1m\x1b[30m\x1b[43m hint"));
    assert!(!message.contains("where"));
    assert!(!message.contains("where:"));
    assert!(!message.contains("hint:"));
    assert!(message.contains("\n\n"));
  }
}
