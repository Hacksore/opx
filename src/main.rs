#![allow(clippy::needless_return)]

mod util;


use crate::util::{get_env_files, run_op_command};

fn main() {
  let env_files = get_env_files();
  let mut args = argv::iter();

  let verb = args.find(|s| s.to_ascii_lowercase() != "opx");
  if let Some(thing) = verb.and_then(|f| f.to_str()) {
    println!("Got the {:#?}", thing);
    run_op_command(env_files, thing);
  }
  
}
