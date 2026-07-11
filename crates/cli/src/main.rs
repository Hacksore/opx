#![allow(clippy::needless_return)]

mod config;
mod util;

use anyhow::Result;
use clap::{Parser};
use config::OpxConfig;
use log::info;

use crate::util::{get_env_files, run_op_command};
use std::env;

use dirs::home_dir;

/// Enhance 1password secret expansion with the opx CLI
#[derive(Parser)]
#[command(name = "opx")]
#[command(about = "Enhance 1password secret expansion with the opx CLI")]
#[command(version)]
struct Cli {
  /// Arguments to pass to the underlying package manager command
  #[arg(trailing_var_arg = true)]
  args: Vec<String>,
}

fn main() -> Result<()> {
  // Initialize logger with [opx] prefix and colored output
  env_logger::Builder::from_default_env()
    .format(|buf, record| {
      use std::io::Write;
      let level_color = match record.level() {
        log::Level::Error => "\x1b[31m", // Red
        log::Level::Warn => "\x1b[33m",  // Yellow
        log::Level::Info => "\x1b[32m",  // Green
        log::Level::Debug => "\x1b[36m", // Cyan
        log::Level::Trace => "\x1b[35m", // Magenta
      };
      let reset_color = "\x1b[0m";
      writeln!(
        buf,
        "{}[opx]{} {}",
        level_color,
        reset_color,
        record.args()
      )
    })
    .filter_level(log::LevelFilter::Info)
    .init();

  let cli = Cli::parse();

  let current_dir = env::current_dir().expect("Failed to get current directory");

  // if they are in their home dir then tell them to go to a project
  if current_dir == home_dir().unwrap() {
    info!("You are in your home directory. Please go to a project directory.");
    return Ok(());
  }

  // NOTE: this is expensive
  let config = OpxConfig::new()?;
  let ignored_directories = config.get_ignored_directories();
  let env_files = get_env_files(ignored_directories);

  // read config from the local directory if possible
  let package_manager = config.get_package_manager();

  // Handle args: if no args provided, use configured default command, otherwise forward all args
  let op_args = if cli.args.is_empty() {
    vec![config.get_default_start_command().clone()]
  } else {
    cli.args
  };

  run_op_command(env_files, op_args, package_manager);

  Ok(())
}
