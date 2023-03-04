#![allow(clippy::needless_return)]

use std::{env, process::Command};
use walkdir::{DirEntry, WalkDir};

fn main() {
  if let Ok(current_dir) = env::current_dir() {
    let directories = WalkDir::new(&current_dir)
      .into_iter()
      .filter_entry(is_skip_dir)
      .filter_map(|e| e.ok());

    let mut env_files: Vec<DirEntry> = vec![];

    for entry in directories {
      if is_env_file(&entry) {
        let cloned = entry.clone();
        env_files.push(cloned)
      }
    }

    let env_commands: Vec<String> = env_files
      .iter()
      .map(|s| format!("{}={}", "--env-file", s.path().to_string_lossy()))
      .collect();

    // let env_command_args = env_commands.join(" ");
    println!("Running npm start with secrets expanded from .env files");
    let _result = run_command_with_args(env_commands);
  }
}

fn is_env_file(entry: &DirEntry) -> bool {
  entry.file_type().is_file()
    && entry
      .file_name()
      .to_str()
      .map(|s| s.ends_with(".env"))
      .unwrap_or(false)
}

// TODO: dont hard code this list
fn is_skip_dir(entry: &DirEntry) -> bool {
  let binding = entry.file_name();
  let name = binding.to_string_lossy();
  if name.contains(".git") || name.contains("node_modules") {
    return false;
  }

  return true;
}

fn run_command_with_args(args: Vec<String>) -> Result<(), std::io::Error> {
  let status = Command::new("op")
    .arg("run")
    .args(args)
    .arg("--")
    .arg("yarn")
    .arg("start")
    .status()
    .expect("Failed to execute command");

  if !status.success() {
    eprintln!("Command failed: {}", status);
  }

  Ok(())
}
