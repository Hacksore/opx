use anyhow::{bail, Context, Result};
use std::cmp::Ordering;
use std::env;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

const FORCE_COLOR: &str = "FORCE_COLOR";
const PROD_ENV: &str = "prod";
const DEV_ENV: &str = "dev";
const STAGING_ENV: &str = "staging";

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedCliArgs {
  pub selected_env: Option<String>,
  pub command_args: Vec<String>,
}

fn restore_force_color(original_force_color: Option<OsString>) {
  match original_force_color {
    Some(value) => env::set_var(FORCE_COLOR, value),
    None => env::remove_var(FORCE_COLOR),
  }
}

fn shorthand_env_arg(arg: &str) -> Option<&'static str> {
  match arg {
    "--prod" => Some(PROD_ENV),
    "--dev" => Some(DEV_ENV),
    "--staging" => Some(STAGING_ENV),
    _ => None,
  }
}

fn validate_environment_name(environment: &str) -> Result<()> {
  if environment.is_empty() {
    bail!(
      "Missing environment name.\n\nhint: Use --env <env>, for example `opx --env prod db:push`."
    );
  }

  if !environment
    .chars()
    .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
  {
    bail!(
      "Invalid environment name `{}`.\n\nwhy: opx maps environments to file names like .env.<env>.\nhint: Use only letters, numbers, hyphens, and underscores.",
      environment
    );
  }

  Ok(())
}

fn select_environment(selected_env: &mut Option<String>, environment: &str) -> Result<()> {
  validate_environment_name(environment)?;

  if let Some(existing_environment) = selected_env {
    bail!(
      "Multiple environments were selected.\n\nselected: {}, {}\nhint: Choose one environment with --env <env> or one shorthand like --prod.",
      existing_environment,
      environment
    );
  }

  *selected_env = Some(environment.to_string());

  Ok(())
}

pub fn parse_cli_args(args: Vec<String>) -> Result<ParsedCliArgs> {
  let mut selected_env: Option<String> = None;
  let mut command_args: Vec<String> = vec![];
  let mut index = 0;

  while index < args.len() {
    let arg = &args[index];

    if arg == "--" {
      command_args.extend(args.iter().skip(index + 1).cloned());
      break;
    }

    if arg == "--env" {
      let environment = args.get(index + 1).ok_or_else(|| {
        anyhow::anyhow!(
          "Missing environment after --env.\n\nhint: Use --env <env>, for example `opx --env prod db:push`."
        )
      })?;

      if environment == "--" {
        bail!(
          "Missing environment after --env.\n\nhint: Put the environment before --, for example `opx --env prod -- db:push --prod`."
        );
      }

      select_environment(&mut selected_env, environment)?;
      index += 2;
      continue;
    }

    if let Some(environment) = arg.strip_prefix("--env=") {
      select_environment(&mut selected_env, environment)?;
      index += 1;
      continue;
    }

    if let Some(environment) = shorthand_env_arg(arg) {
      select_environment(&mut selected_env, environment)?;
      index += 1;
      continue;
    }

    command_args.push(arg.clone());
    index += 1;
  }

  Ok(ParsedCliArgs {
    selected_env,
    command_args,
  })
}

fn is_valid_env_file(name: &str, selected_env: Option<&str>) -> bool {
  if name == ".env" {
    return true;
  }

  match selected_env {
    Some(environment) => name == format!(".env.{environment}"),
    None => false,
  }
}

/// Test if a given dir entry is an .env file
pub fn is_real_env_file(entry: &DirEntry, selected_env: Option<&str>) -> bool {
  entry.file_type().is_file()
    && entry
      .file_name()
      .to_str()
      .map(|name| is_valid_env_file(name, selected_env))
      .unwrap_or(false)
}

fn env_file_group(path: &Path, selected_env: Option<&str>) -> usize {
  match path.file_name().and_then(|name| name.to_str()) {
    Some(".env") => 0,
    Some(name) if selected_env.is_some_and(|environment| name == format!(".env.{environment}")) => {
      1
    }
    _ => 2,
  }
}

fn relative_path(path: &Path, current_dir: &Path) -> PathBuf {
  path
    .strip_prefix(current_dir)
    .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

fn env_file_sort_key(
  path: &Path,
  current_dir: &Path,
  selected_env: Option<&str>,
) -> (usize, usize, PathBuf) {
  let relative_path = relative_path(path, current_dir);

  (
    env_file_group(&relative_path, selected_env),
    relative_path.components().count(),
    relative_path,
  )
}

fn compare_env_files(
  left: &DirEntry,
  right: &DirEntry,
  current_dir: &Path,
  selected_env: Option<&str>,
) -> Ordering {
  env_file_sort_key(left.path(), current_dir, selected_env).cmp(&env_file_sort_key(
    right.path(),
    current_dir,
    selected_env,
  ))
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
  selected_env: Option<&str>,
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
    environment = selected_env.unwrap_or("default"),
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
pub fn get_env_files(selected_env: Option<&str>) -> Result<Vec<DirEntry>> {
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
    if is_real_env_file(&entry, selected_env) {
      let cloned = entry.clone();
      env_files.push(cloned);
    }
  }

  env_files.sort_by(|left, right| compare_env_files(left, right, &current_dir, selected_env));

  Ok(env_files)
}

#[cfg(test)]
mod tests {
  use super::{env_file_sort_key, is_valid_env_file, parse_cli_args, ParsedCliArgs};
  use std::path::PathBuf;

  fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
  }

  #[test]
  fn parses_prod_shorthand_before_command() {
    let parsed = parse_cli_args(args(&["--prod", "db:push"])).unwrap();

    assert_eq!(
      parsed,
      ParsedCliArgs {
        selected_env: Some("prod".to_string()),
        command_args: args(&["db:push"]),
      }
    );
  }

  #[test]
  fn parses_env_after_command() {
    let parsed = parse_cli_args(args(&["db:push", "--env", "staging"])).unwrap();

    assert_eq!(
      parsed,
      ParsedCliArgs {
        selected_env: Some("staging".to_string()),
        command_args: args(&["db:push"]),
      }
    );
  }

  #[test]
  fn parses_equals_env_before_command() {
    let parsed = parse_cli_args(args(&["--env=dev", "dev"])).unwrap();

    assert_eq!(
      parsed,
      ParsedCliArgs {
        selected_env: Some("dev".to_string()),
        command_args: args(&["dev"]),
      }
    );
  }

  #[test]
  fn preserves_args_after_separator() {
    let parsed = parse_cli_args(args(&["--env", "prod", "--", "db:push", "--prod"])).unwrap();

    assert_eq!(
      parsed,
      ParsedCliArgs {
        selected_env: Some("prod".to_string()),
        command_args: args(&["db:push", "--prod"]),
      }
    );
  }

  #[test]
  fn rejects_multiple_environment_selectors() {
    let error = parse_cli_args(args(&["--prod", "--dev"])).unwrap_err();

    assert!(error.to_string().contains("Multiple environments"));
  }

  #[test]
  fn rejects_missing_env_value() {
    let error = parse_cli_args(args(&["--env"])).unwrap_err();

    assert!(error.to_string().contains("Missing environment"));
  }

  #[test]
  fn default_env_selection_only_includes_dot_env() {
    assert!(is_valid_env_file(".env", None));
    assert!(!is_valid_env_file(".env.prod", None));
    assert!(!is_valid_env_file(".env.staging", None));
  }

  #[test]
  fn selected_env_includes_baseline_and_matching_stage() {
    assert!(is_valid_env_file(".env", Some("prod")));
    assert!(is_valid_env_file(".env.prod", Some("prod")));
    assert!(!is_valid_env_file(".env.dev", Some("prod")));
    assert!(!is_valid_env_file(".env.production", Some("prod")));
  }

  #[test]
  fn env_files_sort_baseline_before_selected_stage() {
    let root = PathBuf::from("/repo");
    let mut paths = vec![
      PathBuf::from("/repo/apps/web/.env.prod"),
      PathBuf::from("/repo/.env.prod"),
      PathBuf::from("/repo/apps/web/.env"),
      PathBuf::from("/repo/.env"),
    ];

    paths.sort_by_key(|path| env_file_sort_key(path, &root, Some("prod")));

    assert_eq!(
      paths,
      vec![
        PathBuf::from("/repo/.env"),
        PathBuf::from("/repo/apps/web/.env"),
        PathBuf::from("/repo/.env.prod"),
        PathBuf::from("/repo/apps/web/.env.prod"),
      ]
    );
  }
}
