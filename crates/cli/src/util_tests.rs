use super::{
  ensure_not_nested_opx, get_env_files_from_dir, parse_cli_args, run_op_command, ParsedCliArgs,
  FORCE_COLOR, OPX_ALLOW_NESTED, OPX_DEPTH,
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
fn parse_cli_args_selects_environment_and_preserves_separator_args() {
  let parsed = parse_cli_args(args(&["--prod", "--", "db:push", "--prod"])).unwrap();

  assert_eq!(
    parsed,
    ParsedCliArgs {
      selected_env: Some("prod".to_string()),
      command_args: args(&["db:push", "--prod"]),
    }
  );
}

#[test]
fn parse_cli_args_rejects_ambiguous_or_invalid_environment_selection() {
  assert!(parse_cli_args(args(&["--prod", "--dev"]))
    .unwrap_err()
    .to_string()
    .contains("Multiple environments"));
  assert!(parse_cli_args(args(&["--env"]))
    .unwrap_err()
    .to_string()
    .contains("Missing environment"));
  assert!(parse_cli_args(args(&["--env", "../prod"]))
    .unwrap_err()
    .to_string()
    .contains("Invalid environment name"));
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
fn nested_opx_detection_rejects_recursive_invocation_unless_allowed() {
  let _lock = process_state_lock().lock().unwrap();
  let _guard = ProcessStateGuard::capture();
  env::remove_var(OPX_ALLOW_NESTED);
  env::set_var(OPX_DEPTH, "1");

  let error = ensure_not_nested_opx().unwrap_err();
  assert!(error
    .to_string()
    .contains("Refusing to run opx inside an opx-managed command"));

  env::set_var(OPX_ALLOW_NESTED, "1");
  ensure_not_nested_opx().unwrap();
}

#[test]
fn run_op_command_constructs_op_run_and_restores_process_state() {
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

  run_op_command(env_files, args(&["npm", "run", "dev"]), None).unwrap();

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
fn run_op_command_restores_force_color_when_op_is_missing_or_child_fails() {
  let _lock = process_state_lock().lock().unwrap();
  let _guard = ProcessStateGuard::capture();
  let temp_dir = tempfile::tempdir().unwrap();
  let empty_bin_dir = temp_dir.path().join("empty-bin");
  fs::create_dir_all(&empty_bin_dir).unwrap();
  env::set_current_dir(temp_dir.path()).unwrap();
  env::set_var("PATH", &empty_bin_dir);
  env::set_var(FORCE_COLOR, "false");

  let error = run_op_command(vec![], args(&["pnpm", "dev"]), None).unwrap_err();

  assert!(error
    .to_string()
    .contains("Failed to start 1Password CLI `op`"));
  assert_eq!(env::var(FORCE_COLOR).unwrap(), "false");

  let bin_dir = temp_dir.path().join("bin");
  let args_file = temp_dir.path().join("args.txt");
  let force_color_file = temp_dir.path().join("force_color.txt");
  create_mock_op(&bin_dir);
  env::set_var("PATH", prepend_path(&bin_dir));
  env::set_var(FORCE_COLOR, "true");
  env::set_var("OPX_MOCK_ARGS", &args_file);
  env::set_var("OPX_MOCK_FORCE_COLOR", &force_color_file);
  env::set_var("OPX_MOCK_EXIT", "7");

  let error = run_op_command(vec![], args(&["pnpm", "dev"]), None).unwrap_err();

  assert!(error
    .to_string()
    .contains("The command launched by opx exited unsuccessfully"));
  assert_eq!(fs::read_to_string(force_color_file).unwrap().trim(), "true");
  assert_eq!(env::var(FORCE_COLOR).unwrap(), "true");
}
