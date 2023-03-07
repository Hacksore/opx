#![allow(clippy::needless_return)]

mod util;

use crate::util::{get_env_files, run_op_command};

fn main() {
  let env_files = get_env_files();
  let args = argv::iter();

  let verb = args.skip(1);
  
  // println!("Got the {:#?}", verb);
  run_op_command(env_files, verb);
    
}
