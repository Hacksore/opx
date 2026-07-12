use anyhow::{bail, Context, Result};
use std::cmp::Ordering;
use std::env;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

const FORCE_COLOR: &str = "FORCE_COLOR";
pub const OPX_ALLOW_NESTED: &str = "OPX_ALLOW_NESTED";
pub const OPX_DEPTH: &str = "OPX_DEPTH";
const PROD_ENV: &str = "prod";
const DEV_ENV: &str = "dev";
const STAGING_ENV: &str = "staging";

fn log_debug_lines(message: &str) {
  for line in message.lines().filter(|line| !line.trim().is_empty()) {
    debug!("{line}");
  }
}

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

fn current_opx_depth() -> Result<u32> {
  match env::var(OPX_DEPTH) {
    Ok(value) => value.parse::<u32>().with_context(|| {
      format!(
        "Invalid {OPX_DEPTH} value `{value}`.\n\nwhy: opx uses {OPX_DEPTH} to detect recursive invocations.\nhint: Unset {OPX_DEPTH}, or set it to a non-negative integer."
      )
    }),
    Err(env::VarError::NotPresent) => Ok(0),
    Err(env::VarError::NotUnicode(_)) => bail!(
      "Invalid {OPX_DEPTH} value.\n\nwhy: opx uses {OPX_DEPTH} to detect recursive invocations, but the value is not valid UTF-8.\nhint: Unset {OPX_DEPTH}, or set it to a non-negative integer."
    ),
  }
}

fn allows_nested_opx() -> bool {
  env::var(OPX_ALLOW_NESTED).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

pub fn ensure_not_nested_opx() -> Result<()> {
  let depth = current_opx_depth()?;

  if depth == 0 || allows_nested_opx() {
    return Ok(());
  }

  bail!(
    "Refusing to run opx inside an opx-managed command.\n\nwhy: {OPX_DEPTH} is already set to {depth}, which means this process was started by opx. Running opx again usually recurses, for example `opx -> pnpm dev -> opx`.\nhint: Remove `opx` from the package script and let opx run the raw command, for example `\"dev\": \"next dev\"`, then start the app with `opx`.\n\nIf you intentionally need nested opx, set {OPX_ALLOW_NESTED}=1."
  );
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
  match selected_env {
    Some(environment) => name == format!(".env.{environment}"),
    None => name == ".env",
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
    Some(".env") if selected_env.is_none() => 0,
    Some(name) if selected_env.is_some_and(|environment| name == format!(".env.{environment}")) => {
      0
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
  command_args: Vec<String>,
  selected_env: Option<&str>,
) -> Result<()> {
  if command_args.is_empty() {
    bail!("Missing command to run.\n\nhint: Pass a command to opx, or configure `opx.defaultScript` / `opx.defaultCommand` in package.json.");
  }

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
    match selected_env {
      Some(environment) => {
        warn!(
          directory = %current_dir.display(),
          environment,
          "No .env.{environment} files found. hint: Add a .env.{environment} file with 1Password references, or run without an environment flag to load .env files."
        );
      }
      None => {
        warn!(
          directory = %current_dir.display(),
          "No .env files found. hint: Add a .env file with 1Password references, for example FOO=\"op://vault/item/field\"."
        );
      }
    }
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

  let command_display = command_args.join(" ");

  let mut binding = Command::new("op");
  let command = binding
    .env(OPX_DEPTH, (current_opx_depth()? + 1).to_string())
    .arg("run")
    .args(op_env_flags)
    .arg("--")
    .args(&command_args)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

  let flags = op_env_flags_display.join("\n");
  let fmt_string = if flags.is_empty() {
    format!("op run -- {command_display}")
  } else {
    format!("op run \\\n{} -- {command_display}", flags)
  };

  info!(
    command = %command_display,
    environment = selected_env.unwrap_or("default"),
    env_file_count = env_file_paths.len(),
    "Running command through 1Password"
  );
  log_debug_lines(&fmt_string);

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
        "Failed to start command through `op run`.\n\nwhere: {}\ncommand: op run ... -- {}\nwhy: {}\nhint: Check that the 1Password CLI is installed and that the command is available on PATH.",
        current_dir.display(),
        command_display,
        error,
      );
    }
  };
  let status = match command_spawn.wait() {
    Ok(status) => status,
    Err(error) => {
      restore_force_color(original_force_color);
      bail!(
        "Failed while waiting for the command launched by `op run`.\n\ncommand: {}\nwhy: {}\nhint: Try running the printed `op run` command directly to see whether the child process is being interrupted.",
        command_display,
        error
      );
    }
  };

  restore_force_color(original_force_color);

  if !status.success() {
    bail!(
      "The command launched by opx exited unsuccessfully.\n\ncommand: {}\nstatus: {}\nhint: opx successfully started `op run`; inspect the output above from `{}` to fix the failing script.",
      command_display,
      status,
      command_args[0]
    );
  }

  Ok(())
}

/// Get all `DirEntry` for every `.env` file from the current directory
pub fn get_env_files(selected_env: Option<&str>) -> Result<Vec<DirEntry>> {
  let current_dir = env::current_dir().context(
    "Failed to determine the current working directory while scanning for .env files.\n\nhint: Run opx from a project directory that still exists on disk.",
  )?;

  get_env_files_from_dir(&current_dir, selected_env)
}

fn get_env_files_from_dir(current_dir: &Path, selected_env: Option<&str>) -> Result<Vec<DirEntry>> {
  // All the dirs with .env files excluding certain skipped folders
  let directories = WalkDir::new(current_dir)
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

  env_files.sort_by(|left, right| compare_env_files(left, right, current_dir, selected_env));

  Ok(env_files)
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod util_tests;
