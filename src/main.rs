#![allow(clippy::needless_return)]

mod util;
mod command;

use crate::util::{get_env_files, run_op_command};

fn main() {
  let env_files = get_env_files();

  let env_file_arguments: Vec<String> = env_files
    .iter()
    .map(|s| format!("{}={}", "--env-file", s.path().to_string_lossy()))
    .collect();
  
  println!("Running npm start with secrets expanded from .env files");
  env_files.iter().for_each(|e| println!("{}", e.path().display()));
  let _result = run_op_command(env_file_arguments);
}
