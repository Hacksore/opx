#![allow(clippy::needless_return)]

mod config;
mod util;

use anyhow::Result;
use config::OpxConfig;

use crate::util::{get_env_files, run_op_command};
use std::env;

fn main() -> Result<()> {
  let env_files = get_env_files();
  let cli_args = env::args().skip(1).collect::<Vec<String>>();

  // read config from the local director if possible
  let config = OpxConfig::new()?;
  let package_manager = config.get_package_manager();

  // default command for now is start but this should be configurable
  let op_args = match cli_args {
    args if args.is_empty() => vec!["start".to_string()],
    args => args,
  };

  run_op_command(env_files, op_args, package_manager);

  Ok(())
}
