#![allow(clippy::needless_return)]

mod util;

use crate::util::{get_env_files, run_op_command};
use std::env;

fn main() {
  let env_files = get_env_files();
  let cli_args = env::args().skip(1).collect::<Vec<String>>();

  // default command for now is start but this should be configurable 
  let op_args = match cli_args {
    args if args.is_empty() => vec!["start".to_string()],
    args => args,
  };

  run_op_command(env_files, op_args.into_iter())
}
