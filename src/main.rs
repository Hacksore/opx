#![allow(clippy::needless_return)]

mod util;

use crate::util::{get_env_files, run_op_command};

fn main() {
  let env_files = get_env_files();
  run_op_command(env_files);
}
