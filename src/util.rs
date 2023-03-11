use std::env;
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

const FORCE_COLOR: &str = "FORCE_COLOR";

/// TODO: what do you do about dimensions .env.local vs .env.production
/// naive thought is you need a flag on the CLI for --env <env>
fn is_valid_env_file(name: &str) -> bool {
  if name == ".env" {
    return true;
  }

  return false;
}

/// Test if a given dir entry is an .env file
pub fn is_env_file(entry: &DirEntry) -> bool {
  entry.file_type().is_file()
    && entry
      .file_name()
      .to_str()
      .map(is_valid_env_file)
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
pub fn run_op_command(
  env_files: Vec<DirEntry>,
  args: impl Iterator<Item = String>,
  package_manager: &String,
) {
  let current_dir = env::current_dir();
  let mut current_dir_string = String::from("");
  match &current_dir {
    Ok(dir) => {
      current_dir_string = dir.to_string_lossy().to_string();
    }
    Err(_) => {
      // do nothing
    }
  }

  let force_color_str = env::var(FORCE_COLOR).unwrap_or_default();
  let force_color: bool = force_color_str.parse().unwrap_or(false);

  // set force color before running the shell command to make libs like chalk output colors
  if !force_color {
    println!("[OPX] Forcing terminal colors with {}=1", FORCE_COLOR);
    env::set_var(FORCE_COLOR, "1");
  }

  // print out a list of all the ENV files sourced
  env_files.iter().for_each(|e| {
    let env_file_path = e.path().display().to_string();
    let mut absolute_dir = env_file_path.replace(&current_dir_string, "");
    absolute_dir.remove(0);

    println!("[ENV] {}", absolute_dir)
  });

  let op_env_flags: Vec<String> = env_files
    .iter()
    .map(|s| format!("{}={}", "--env-file", s.path().to_string_lossy()))
    .collect();

  let mut binding = Command::new("op");
  let command = binding
    .arg("run")
    .args(op_env_flags)
    .arg("--")
    .arg(package_manager)
    .args(args);

  let command_args: Vec<_> = command.get_args().map(|s| s.to_string_lossy()).collect();
  println!("[OPX] {} {}", command.get_program().to_string_lossy(), command_args.join(" ")) ;

  let mut command_spawn = command.spawn().expect("Failed to execute command");
  let status = command_spawn.wait().expect("Failed to wait for child process");

  if force_color {
    env::remove_var(FORCE_COLOR)
  } else {
    env::set_var(FORCE_COLOR, "1");
  }

  if !status.success() {
    eprintln!("Command failed: {}", status);
  }
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
      env_files.push(cloned);
    }
  }

  return env_files;
}
