#![allow(clippy::needless_return)]

mod config;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::OpxConfig;

use crate::util::{get_env_files, run_op_command};
use std::env;

use dirs::home_dir;

/// Enhance 1password secret expansion with the opx CLI
#[derive(Parser)]
#[command(name = "opx")]
#[command(about = "Enhance 1password secret expansion with the opx CLI")]
#[command(version)]
struct Cli {
  #[command(subcommand)]
  command: Option<Commands>,

  /// Arguments to pass to the underlying package manager command
  #[arg(trailing_var_arg = true)]
  args: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
  /// Start the development server (default command)
  Start {
    /// Arguments to pass to the start command
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
  },
  /// Run a package manager command with 1password secret expansion
  Run {
    /// The command to run
    command: String,
    /// Arguments to pass to the command
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
  },
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  let current_dir = env::current_dir().unwrap();

  // if they are in their home dir then tell them to go to a project
  if current_dir == home_dir().unwrap() {
    println!("[OPX] You are in your home directory. Please go to a project directory.");
    return Ok(());
  }

  // NOTE: this is expensive
  let env_files = get_env_files();

  // read config from the local directory if possible
  let config = OpxConfig::new()?;
  let package_manager = config.get_package_manager();

  // Handle different commands
  let op_args = match cli.command {
    Some(Commands::Start { args }) => {
      let mut start_args = vec!["start".to_string()];
      start_args.extend(args);
      start_args
    }
    Some(Commands::Run { command, args }) => {
      let mut run_args = vec![command];
      run_args.extend(args);
      run_args
    }
    None => {
      // Default behavior: if no command specified, use 'start'
      // If args were provided without a command, treat them as start args
      if cli.args.is_empty() {
        vec!["start".to_string()]
      } else {
        let mut start_args = vec!["start".to_string()];
        start_args.extend(cli.args);
        start_args
      }
    }
  };

  run_op_command(env_files, op_args, package_manager);

  Ok(())
}
