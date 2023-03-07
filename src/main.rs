#![allow(clippy::needless_return)]

mod util;

use std::env::{args as dank_args};
use crate::util::{get_env_files, run_op_command};

fn main() {
  let env_files = get_env_files();
  let cli_args = dank_args().skip(1).collect::<Vec<String>>();

  let op_args = match cli_args {
    args if args.is_empty() => vec!["start".to_string()],
    args => args,
  };

  run_op_command(env_files, op_args.into_iter())
}
