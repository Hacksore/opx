#![allow(clippy::needless_return)]

mod config;
mod util;

use crate::util::{get_env_files, run_op_command};
use anyhow::{Context, Result};
use config::OpxConfig;
use dirs::home_dir;
use std::env;

fn main() -> Result<()> {
  let current_dir = env::current_dir().context("Failed to get current directory")?;

  // if they are in their home dir then tell them to go to a project
  if Some(current_dir.as_path()) == home_dir().as_deref() {
    println!("[OPX] You are in your home directory. Please go to a project directory.");
    return Ok(());
  }

  // NOTE: this is expensive
  let env_files = get_env_files();
  let cli_args = env::args().skip(1).collect::<Vec<String>>();

  // read config from the local director if possible
  let config = OpxConfig::new()?;
  let package_manager = config.get_package_manager();

  // TODO: default command for now is start but this should be configurable
  let op_args = match cli_args {
    args if args.is_empty() => vec!["start".to_string()],
    args => args,
  };

  run_op_command(env_files, op_args, package_manager)?;

  Ok(())
}
