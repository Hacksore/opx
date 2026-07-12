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

  let args_clone = args.clone();
  let command_display = format!("{} {}", package_manager, args_clone.join(" "));

  let mut binding = Command::new("op");
  let command = binding
    .env(OPX_DEPTH, (current_opx_depth()? + 1).to_string())
    .arg("run")
    .args(op_env_flags)
    .arg("--")
    .arg(package_manager)
    .args(args)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

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
mod tests {
  use super::{
    ensure_not_nested_opx, env_file_sort_key, get_env_files_from_dir, is_valid_env_file,
    parse_cli_args, run_op_command, ParsedCliArgs, FORCE_COLOR, OPX_ALLOW_NESTED, OPX_DEPTH,
  };
  use std::env;
  use std::ffi::OsString;
  use std::fs;
  use std::path::{Path, PathBuf};
  use std::sync::{Mutex, OnceLock};

  static PROCESS_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

  struct ProcessStateGuard {
    current_dir: PathBuf,
    path: Option<OsString>,
    force_color: Option<OsString>,
    opx_allow_nested: Option<OsString>,
    opx_depth: Option<OsString>,
    mock_args: Option<OsString>,
    mock_force_color: Option<OsString>,
    mock_opx_depth: Option<OsString>,
    mock_exit: Option<OsString>,
  }

  impl ProcessStateGuard {
    fn capture() -> Self {
      Self {
        current_dir: env::current_dir().unwrap(),
        path: env::var_os("PATH"),
        force_color: env::var_os(FORCE_COLOR),
        opx_allow_nested: env::var_os(OPX_ALLOW_NESTED),
        opx_depth: env::var_os(OPX_DEPTH),
        mock_args: env::var_os("OPX_MOCK_ARGS"),
        mock_force_color: env::var_os("OPX_MOCK_FORCE_COLOR"),
        mock_opx_depth: env::var_os("OPX_MOCK_OPX_DEPTH"),
        mock_exit: env::var_os("OPX_MOCK_EXIT"),
      }
    }
  }

  impl Drop for ProcessStateGuard {
    fn drop(&mut self) {
      env::set_current_dir(&self.current_dir).unwrap();
      restore_env_var("PATH", self.path.as_ref());
      restore_env_var(FORCE_COLOR, self.force_color.as_ref());
      restore_env_var(OPX_ALLOW_NESTED, self.opx_allow_nested.as_ref());
      restore_env_var(OPX_DEPTH, self.opx_depth.as_ref());
      restore_env_var("OPX_MOCK_ARGS", self.mock_args.as_ref());
      restore_env_var("OPX_MOCK_FORCE_COLOR", self.mock_force_color.as_ref());
      restore_env_var("OPX_MOCK_OPX_DEPTH", self.mock_opx_depth.as_ref());
      restore_env_var("OPX_MOCK_EXIT", self.mock_exit.as_ref());
    }
  }

  fn process_state_lock() -> &'static Mutex<()> {
    PROCESS_STATE_LOCK.get_or_init(|| Mutex::new(()))
  }

  fn restore_env_var(key: &str, value: Option<&OsString>) {
    match value {
      Some(value) => env::set_var(key, value),
      None => env::remove_var(key),
    }
  }

  fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
  }

  fn relative_paths(paths: &[walkdir::DirEntry], root: &Path) -> Vec<String> {
    paths
      .iter()
      .map(|entry| {
        entry
          .path()
          .strip_prefix(root)
          .unwrap()
          .to_string_lossy()
          .replace('\\', "/")
      })
      .collect()
  }

  fn prepend_path(path: &Path) -> OsString {
    let mut paths = vec![path.to_path_buf()];

    if let Some(existing_path) = env::var_os("PATH") {
      paths.extend(env::split_paths(&existing_path));
    }

    env::join_paths(paths).unwrap()
  }

  #[cfg(unix)]
  fn create_mock_op(bin_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(bin_dir).unwrap();
    let op_path = bin_dir.join("op");
    fs::write(
      &op_path,
      r#"#!/bin/sh
printf '%s\n' "$@" > "$OPX_MOCK_ARGS"
printf '%s\n' "${FORCE_COLOR-}" > "$OPX_MOCK_FORCE_COLOR"
if [ -n "${OPX_MOCK_OPX_DEPTH-}" ]; then
  printf '%s\n' "${OPX_DEPTH-}" > "$OPX_MOCK_OPX_DEPTH"
fi
exit "${OPX_MOCK_EXIT:-0}"
"#,
    )
    .unwrap();
    fs::set_permissions(&op_path, fs::Permissions::from_mode(0o755)).unwrap();
  }

  #[cfg(windows)]
  fn create_mock_op(bin_dir: &Path) {
    fs::create_dir_all(bin_dir).unwrap();
    fs::write(
      bin_dir.join("op.cmd"),
      r#"@echo off
for %%a in (%*) do echo %%~a>> "%OPX_MOCK_ARGS%"
echo %FORCE_COLOR%> "%OPX_MOCK_FORCE_COLOR%"
if not "%OPX_MOCK_OPX_DEPTH%"=="" echo %OPX_DEPTH%> "%OPX_MOCK_OPX_DEPTH%"
exit /B %OPX_MOCK_EXIT%
"#,
    )
    .unwrap();
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
  fn rejects_empty_equals_env_value() {
    let error = parse_cli_args(args(&["--env="])).unwrap_err();

    assert!(error.to_string().contains("Missing environment name"));
  }

  #[test]
  fn rejects_invalid_env_value() {
    let error = parse_cli_args(args(&["--env", "../prod"])).unwrap_err();

    assert!(error.to_string().contains("Invalid environment name"));
  }

  #[test]
  fn default_env_selection_only_includes_dot_env() {
    assert!(is_valid_env_file(".env", None));
    assert!(!is_valid_env_file(".env.prod", None));
    assert!(!is_valid_env_file(".env.staging", None));
  }

  #[test]
  fn selected_env_only_includes_matching_stage() {
    assert!(!is_valid_env_file(".env", Some("prod")));
    assert!(is_valid_env_file(".env.prod", Some("prod")));
    assert!(!is_valid_env_file(".env.dev", Some("prod")));
    assert!(!is_valid_env_file(".env.production", Some("prod")));
  }

  #[test]
  fn env_files_sort_selected_stage_by_shallowest_path() {
    let root = PathBuf::from("/repo");
    let mut paths = vec![
      PathBuf::from("/repo/apps/web/.env.prod"),
      PathBuf::from("/repo/.env.prod"),
    ];

    paths.sort_by_key(|path| env_file_sort_key(path, &root, Some("prod")));

    assert_eq!(
      paths,
      vec![
        PathBuf::from("/repo/.env.prod"),
        PathBuf::from("/repo/apps/web/.env.prod"),
      ]
    );
  }

  #[test]
  fn env_file_scan_walks_project_and_skips_ignored_directories() {
    let temp_dir = tempfile::tempdir().unwrap();
    fs::write(temp_dir.path().join(".env"), "ROOT=1").unwrap();
    fs::write(temp_dir.path().join(".env.prod"), "ROOT_PROD=1").unwrap();
    fs::create_dir_all(temp_dir.path().join("apps/web")).unwrap();
    fs::write(temp_dir.path().join("apps/web/.env"), "APP=1").unwrap();
    fs::write(temp_dir.path().join("apps/web/.env.prod"), "APP_PROD=1").unwrap();
    fs::create_dir_all(temp_dir.path().join(".git")).unwrap();
    fs::write(temp_dir.path().join(".git/.env"), "IGNORED=1").unwrap();
    fs::create_dir_all(temp_dir.path().join("node_modules/package")).unwrap();
    fs::write(
      temp_dir.path().join("node_modules/package/.env"),
      "IGNORED=1",
    )
    .unwrap();

    let default_env_files = get_env_files_from_dir(temp_dir.path(), None).unwrap();
    let prod_env_files = get_env_files_from_dir(temp_dir.path(), Some("prod")).unwrap();

    assert_eq!(
      relative_paths(&default_env_files, temp_dir.path()),
      vec![".env", "apps/web/.env"]
    );
    assert_eq!(
      relative_paths(&prod_env_files, temp_dir.path()),
      vec![".env.prod", "apps/web/.env.prod"]
    );
  }

  #[test]
  fn nested_opx_detection_allows_first_invocation() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    env::remove_var(OPX_ALLOW_NESTED);
    env::remove_var(OPX_DEPTH);

    ensure_not_nested_opx().unwrap();
  }

  #[test]
  fn nested_opx_detection_rejects_recursive_invocation() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    env::remove_var(OPX_ALLOW_NESTED);
    env::set_var(OPX_DEPTH, "1");

    let error = ensure_not_nested_opx().unwrap_err();

    assert!(error
      .to_string()
      .contains("Refusing to run opx inside an opx-managed command"));
  }

  #[test]
  fn nested_opx_detection_allows_explicit_override() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    env::set_var(OPX_ALLOW_NESTED, "1");
    env::set_var(OPX_DEPTH, "1");

    ensure_not_nested_opx().unwrap();
  }

  #[test]
  fn run_op_command_constructs_op_run_and_restores_force_color() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    let temp_dir = tempfile::tempdir().unwrap();
    let bin_dir = temp_dir.path().join("bin");
    let args_file = temp_dir.path().join("args.txt");
    let force_color_file = temp_dir.path().join("force_color.txt");
    let opx_depth_file = temp_dir.path().join("opx_depth.txt");

    create_mock_op(&bin_dir);
    fs::write(temp_dir.path().join(".env"), "ROOT=1").unwrap();
    fs::create_dir_all(temp_dir.path().join("apps/web")).unwrap();
    fs::write(temp_dir.path().join("apps/web/.env"), "APP=1").unwrap();

    let env_files = get_env_files_from_dir(temp_dir.path(), None).unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();
    env::set_var("PATH", prepend_path(&bin_dir));
    env::remove_var(FORCE_COLOR);
    env::remove_var(OPX_DEPTH);
    env::set_var("OPX_MOCK_ARGS", &args_file);
    env::set_var("OPX_MOCK_FORCE_COLOR", &force_color_file);
    env::set_var("OPX_MOCK_OPX_DEPTH", &opx_depth_file);
    env::set_var("OPX_MOCK_EXIT", "0");

    run_op_command(env_files, args(&["run", "dev"]), "npm", None).unwrap();

    let recorded_args = fs::read_to_string(args_file).unwrap();
    assert_eq!(
      recorded_args
        .lines()
        .map(str::to_string)
        .collect::<Vec<String>>(),
      vec![
        "run".to_string(),
        format!("--env-file={}", temp_dir.path().join(".env").display()),
        format!(
          "--env-file={}",
          temp_dir.path().join("apps/web/.env").display()
        ),
        "--".to_string(),
        "npm".to_string(),
        "run".to_string(),
        "dev".to_string(),
      ]
    );
    assert_eq!(fs::read_to_string(force_color_file).unwrap().trim(), "1");
    assert_eq!(fs::read_to_string(opx_depth_file).unwrap().trim(), "1");
    assert!(env::var_os(FORCE_COLOR).is_none());
  }

  #[test]
  fn run_op_command_restores_force_color_when_op_is_missing() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    let temp_dir = tempfile::tempdir().unwrap();
    let empty_bin_dir = temp_dir.path().join("empty-bin");
    fs::create_dir_all(&empty_bin_dir).unwrap();
    env::set_current_dir(temp_dir.path()).unwrap();
    env::set_var("PATH", &empty_bin_dir);
    env::set_var(FORCE_COLOR, "false");

    let error = run_op_command(vec![], args(&["dev"]), "pnpm", None).unwrap_err();

    assert!(error
      .to_string()
      .contains("Failed to start 1Password CLI `op`"));
    assert_eq!(env::var(FORCE_COLOR).unwrap(), "false");
  }

  #[test]
  fn run_op_command_reports_child_failure_and_restores_force_color() {
    let _lock = process_state_lock().lock().unwrap();
    let _guard = ProcessStateGuard::capture();
    let temp_dir = tempfile::tempdir().unwrap();
    let bin_dir = temp_dir.path().join("bin");
    let args_file = temp_dir.path().join("args.txt");
    let force_color_file = temp_dir.path().join("force_color.txt");

    create_mock_op(&bin_dir);
    env::set_current_dir(temp_dir.path()).unwrap();
    env::set_var("PATH", prepend_path(&bin_dir));
    env::set_var(FORCE_COLOR, "true");
    env::set_var("OPX_MOCK_ARGS", &args_file);
    env::set_var("OPX_MOCK_FORCE_COLOR", &force_color_file);
    env::set_var("OPX_MOCK_EXIT", "7");

    let error = run_op_command(vec![], args(&["dev"]), "pnpm", None).unwrap_err();

    assert!(error
      .to_string()
      .contains("The command launched by opx exited unsuccessfully"));
    assert_eq!(fs::read_to_string(force_color_file).unwrap().trim(), "true");
    assert_eq!(env::var(FORCE_COLOR).unwrap(), "true");
  }
}
