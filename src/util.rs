use std::process::Command;
use walkdir::{DirEntry, WalkDir};
use std::{env};

pub fn is_env_file(entry: &DirEntry) -> bool {
  entry.file_type().is_file()
    && entry
      .file_name()
      .to_str()
      .map(|s| s.ends_with(".env"))
      .unwrap_or(false)
}

/// TODO: Do not hard code this list and maybe add yet another dotfile?
pub fn is_skip_dir(entry: &DirEntry) -> bool {
  let binding = entry.file_name();
  let name = binding.to_string_lossy();
  if name.contains(".git") || name.contains("node_modules") {
    return false;
  }

  return true;
}

/// Run the `op` command with all the `.env` vars files found in the current directory
pub fn run_op_command(args: Vec<String>) -> Result<(), std::io::Error> {
  let status = Command::new("op")
    .arg("run")
    .args(args)
    .arg("--")
    // TODO: allow custom pkg manager
    .arg("yarn")
    // TODO: allow custom command
    .arg("start")
    .arg("--color")
    .arg("always")
    .status()
    .expect("Failed to execute command");

  if !status.success() {
    eprintln!("Command failed: {}", status);
  }

  Ok(())
}

/// Get all `DirEntry` for every `.env` file from the current directory
pub fn get_env_files() -> Vec<DirEntry> {
  let current_dir = env::current_dir().expect("Failed to get current directory");
   
  // All the dirs with .env files excluding certain skipped folders
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

  return env_files;
}
