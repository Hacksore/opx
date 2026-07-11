use anyhow::{bail, Context, Result};
use std::env;
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

const FORCE_COLOR: &str = "FORCE_COLOR";

/// TODO: what do you do about dimensions .env.local vs .env.production
/// naive thought is you need a flag on the CLI for --env <env>
fn is_valid_env_file(name: &str) -> bool {
  name == ".env"
}

/// Test if a given dir entry is an .env file
pub fn is_real_env_file(entry: &DirEntry) -> bool {
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
  !name.contains(".git") && !name.contains("node_modules")
}

/// Run the `op` command with all the `.env` vars files found in the current directory
pub fn run_op_command(
  env_files: Vec<DirEntry>,
  args: Vec<String>,
  package_manager: &str,
) -> Result<()> {
  let current_dir = env::current_dir().context("Failed to get current directory")?;

  let original_force_color = env::var_os(FORCE_COLOR);
  let force_color_str = env::var(FORCE_COLOR).unwrap_or_default();
  let force_color: bool = force_color_str.parse().unwrap_or(false);

  // set force color before running the shell command to make libs like chalk output colors
  if !force_color {
    println!("[OPX] Forcing terminal colors with {}=1", FORCE_COLOR);
    env::set_var(FORCE_COLOR, "1");
  }

  // print out a list of all the ENV files sourced
  env_files
    .iter()
    .filter_map(|e| e.path().strip_prefix(&current_dir).ok())
    .for_each(|path| println!("[ENV] {}", path.display()));

  let op_env_flags: Vec<String> = env_files
    .iter()
    .map(|s| format!("{}={}", "--env-file", s.path().to_string_lossy()))
    .collect();

  let op_env_flags_display: Vec<String> = op_env_flags
    .clone()
    .iter()
    .enumerate()
    .map(|(index, flag)| {
      let mut display_flag = flag.clone();
      if index + 1 != op_env_flags.len() {
        display_flag.push_str(" \\");
      }

      format!(
        "\t{}",
        display_flag.replace(&current_dir.to_string_lossy().to_string(), "")
      )
    })
    .collect();

  let args_clone = args.clone();

  let mut binding = Command::new("op");
  let command = binding
    .arg("run")
    .args(op_env_flags)
    .arg("--")
    .arg(package_manager)
    .args(args);

  let flags = op_env_flags_display.join("\n");
  let fmt_string = format!(
    "[OPX] op run \\\n{} -- {} {}",
    flags,
    package_manager,
    args_clone.join(" ")
  );

  println!("{fmt_string}");

  let mut command_spawn = command.spawn().context("Failed to execute command")?;
  let status = command_spawn
    .wait()
    .context("Failed to wait for child process")?;

  match original_force_color {
    Some(value) => env::set_var(FORCE_COLOR, value),
    None => env::remove_var(FORCE_COLOR),
  }

  if !status.success() {
    bail!("Command failed: {}", status);
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
    if is_real_env_file(&entry) {
      let cloned = entry.clone();
      env_files.push(cloned);
    }
  }

  env_files
}
