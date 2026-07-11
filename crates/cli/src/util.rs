use anyhow::{bail, Context, Result};
use std::env;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::process::Command;
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

const FORCE_COLOR: &str = "FORCE_COLOR";

fn restore_force_color(original_force_color: Option<OsString>) {
  match original_force_color {
    Some(value) => env::set_var(FORCE_COLOR, value),
    None => env::remove_var(FORCE_COLOR),
  }
}

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
  let current_dir = env::current_dir().context(
    "Failed to determine the current working directory.\n\nhint: Run opx from a project directory that still exists on disk.",
  )?;

  let original_force_color = env::var_os(FORCE_COLOR);
  let force_color_str = env::var(FORCE_COLOR).unwrap_or_default();
  let force_color: bool = force_color_str.parse().unwrap_or(false);

  // set force color before running the shell command to make libs like chalk output colors
  if !force_color {
    debug!(env_var = FORCE_COLOR, value = 1, "Forcing terminal colors");
    env::set_var(FORCE_COLOR, "1");
  }

  let env_file_paths = env_files
    .iter()
    .filter_map(|e| e.path().strip_prefix(&current_dir).ok())
    .map(|path| path.display().to_string())
    .collect::<Vec<String>>();

  debug!(env_files = ?env_file_paths, "Resolved env files");

  if env_files.is_empty() {
    warn!(
      directory = %current_dir.display(),
      "No .env files found.\n\nhint: Add a .env file with 1Password references, for example FOO=\"op://vault/item/field\"."
    );
  }

  let op_env_flags: Vec<String> = env_files
    .iter()
    .map(|s| format!("{}={}", "--env-file", s.path().to_string_lossy()))
    .collect();

  let op_env_flags_display: Vec<String> = env_file_paths
    .iter()
    .enumerate()
    .map(|(index, path)| {
      let mut display_flag = format!("--env-file={path}");
      if index + 1 != env_file_paths.len() {
        display_flag.push_str(" \\");
      }

      format!("\t{display_flag}")
    })
    .collect();

  let args_clone = args.clone();
  let command_display = format!("{} {}", package_manager, args_clone.join(" "));

  let mut binding = Command::new("op");
  let command = binding
    .arg("run")
    .args(op_env_flags)
    .arg("--")
    .arg(package_manager)
    .args(args);

  let flags = op_env_flags_display.join("\n");
  let fmt_string = if flags.is_empty() {
    format!("op run -- {} {}", package_manager, args_clone.join(" "))
  } else {
    format!(
      "op run \\\n{} -- {} {}",
      flags,
      package_manager,
      args_clone.join(" ")
    )
  };

  info!(
    command = %command_display,
    env_file_count = env_file_paths.len(),
    "Running command through 1Password"
  );
  debug!("{fmt_string}");

  let mut command_spawn = match command.spawn() {
    Ok(child) => child,
    Err(error) if error.kind() == ErrorKind::NotFound => {
      restore_force_color(original_force_color);
      bail!(
        "Failed to start 1Password CLI `op`.\n\nwhere: {}\nwhy: opx runs your command through `op run`, but no executable named `op` was found on PATH.\nhint: Install the 1Password CLI and make sure `op --version` works in this terminal.",
        current_dir.display()
      );
    }
    Err(error) => {
      restore_force_color(original_force_color);
      bail!(
        "Failed to start command through `op run`.\n\nwhere: {}\ncommand: op run ... -- {} {}\nwhy: {}\nhint: Check that the 1Password CLI is installed and that `{}` is available on PATH.",
        current_dir.display(),
        package_manager,
        args_clone.join(" "),
        error,
        package_manager
      );
    }
  };
  let status = match command_spawn.wait() {
    Ok(status) => status,
    Err(error) => {
      restore_force_color(original_force_color);
      bail!(
        "Failed while waiting for the command launched by `op run`.\n\ncommand: {} {}\nwhy: {}\nhint: Try running the printed `op run` command directly to see whether the child process is being interrupted.",
        package_manager,
        args_clone.join(" "),
        error
      );
    }
  };

  restore_force_color(original_force_color);

  if !status.success() {
    bail!(
      "The command launched by opx exited unsuccessfully.\n\ncommand: {} {}\nstatus: {}\nhint: opx successfully started `op run`; inspect the output above from `{}` to fix the failing script.",
      package_manager,
      args_clone.join(" "),
      status,
      package_manager
    );
  }

  Ok(())
}

/// Get all `DirEntry` for every `.env` file from the current directory
pub fn get_env_files() -> Result<Vec<DirEntry>> {
  let current_dir = env::current_dir().context(
    "Failed to determine the current working directory while scanning for .env files.\n\nhint: Run opx from a project directory that still exists on disk.",
  )?;

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

  Ok(env_files)
}
