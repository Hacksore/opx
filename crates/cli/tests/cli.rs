use assert_cmd::Command;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

fn opx_command(project_dir: &Path) -> Command {
  let mut command = Command::cargo_bin("opx").unwrap();
  command.current_dir(project_dir);
  command
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

fn read_lines(path: impl Into<PathBuf>) -> Vec<String> {
  fs::read_to_string(path.into())
    .unwrap()
    .lines()
    .map(str::to_string)
    .collect()
}

#[test]
fn runs_default_script_through_mocked_op() {
  let temp_dir = tempfile::tempdir().unwrap();
  let project_dir = temp_dir.path();
  let bin_dir = project_dir.join("bin");
  let args_file = project_dir.join("args.txt");
  let force_color_file = project_dir.join("force_color.txt");

  create_mock_op(&bin_dir);
  fs::write(
    project_dir.join("package.json"),
    r#"{
      "packageManager": "pnpm@10.0.0",
      "opx": {
        "defaultScript": "serve"
      }
    }"#,
  )
  .unwrap();
  fs::write(project_dir.join(".env"), "ROOT=1").unwrap();
  fs::create_dir_all(project_dir.join("apps/web")).unwrap();
  fs::write(project_dir.join("apps/web/.env"), "APP=1").unwrap();
  let canonical_project_dir = fs::canonicalize(project_dir).unwrap();

  opx_command(project_dir)
    .env("PATH", prepend_path(&bin_dir))
    .env("OPX_MOCK_ARGS", &args_file)
    .env("OPX_MOCK_FORCE_COLOR", &force_color_file)
    .env("OPX_MOCK_EXIT", "0")
    .assert()
    .success();

  assert_eq!(
    read_lines(args_file),
    vec![
      "run".to_string(),
      format!(
        "--env-file={}",
        canonical_project_dir.join(".env").display()
      ),
      format!(
        "--env-file={}",
        canonical_project_dir.join("apps/web/.env").display()
      ),
      "--".to_string(),
      "pnpm".to_string(),
      "serve".to_string(),
    ]
  );
  assert_eq!(fs::read_to_string(force_color_file).unwrap().trim(), "1");
}

#[test]
fn runs_raw_default_command_through_mocked_op() {
  let temp_dir = tempfile::tempdir().unwrap();
  let project_dir = temp_dir.path();
  let bin_dir = project_dir.join("bin");
  let args_file = project_dir.join("args.txt");
  let force_color_file = project_dir.join("force_color.txt");

  create_mock_op(&bin_dir);
  fs::write(
    project_dir.join("package.json"),
    r#"{
      "packageManager": "pnpm@10.0.0",
      "scripts": {
        "dev": "opx"
      },
      "opx": {
        "defaultCommand": "next dev"
      }
    }"#,
  )
  .unwrap();
  fs::write(project_dir.join(".env"), "ROOT=1").unwrap();
  let canonical_project_dir = fs::canonicalize(project_dir).unwrap();

  opx_command(project_dir)
    .env("PATH", prepend_path(&bin_dir))
    .env("OPX_MOCK_ARGS", &args_file)
    .env("OPX_MOCK_FORCE_COLOR", &force_color_file)
    .env("OPX_MOCK_EXIT", "0")
    .assert()
    .success();

  assert_eq!(
    read_lines(args_file),
    vec![
      "run".to_string(),
      format!(
        "--env-file={}",
        canonical_project_dir.join(".env").display()
      ),
      "--".to_string(),
      "next".to_string(),
      "dev".to_string(),
    ]
  );
  assert_eq!(fs::read_to_string(force_color_file).unwrap().trim(), "1");
}

#[test]
fn runs_selected_environment_and_preserves_command_args_after_separator() {
  let temp_dir = tempfile::tempdir().unwrap();
  let project_dir = temp_dir.path();
  let bin_dir = project_dir.join("bin");
  let args_file = project_dir.join("args.txt");
  let force_color_file = project_dir.join("force_color.txt");

  create_mock_op(&bin_dir);
  fs::write(
    project_dir.join("package.json"),
    r#"{"packageManager":"npm@10.0.0"}"#,
  )
  .unwrap();
  fs::write(project_dir.join(".env"), "ROOT=1").unwrap();
  fs::write(project_dir.join(".env.prod"), "PROD=1").unwrap();
  let canonical_project_dir = fs::canonicalize(project_dir).unwrap();

  opx_command(project_dir)
    .args(["--prod", "--", "db:push", "--prod"])
    .env("PATH", prepend_path(&bin_dir))
    .env("OPX_MOCK_ARGS", &args_file)
    .env("OPX_MOCK_FORCE_COLOR", &force_color_file)
    .env("OPX_MOCK_EXIT", "0")
    .assert()
    .success();

  assert_eq!(
    read_lines(args_file),
    vec![
      "run".to_string(),
      format!(
        "--env-file={}",
        canonical_project_dir.join(".env.prod").display()
      ),
      "--".to_string(),
      "npm".to_string(),
      "db:push".to_string(),
      "--prod".to_string(),
    ]
  );
}

#[test]
fn fails_when_package_json_is_missing() {
  let temp_dir = tempfile::tempdir().unwrap();

  opx_command(temp_dir.path())
    .assert()
    .failure()
    .stderr(predicates::str::contains("Failed to find package.json"));
}

#[test]
fn fails_when_package_json_is_invalid() {
  let temp_dir = tempfile::tempdir().unwrap();
  fs::write(temp_dir.path().join("package.json"), "{").unwrap();

  opx_command(temp_dir.path())
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Failed to parse package.json as JSON",
    ));
}

#[test]
fn fails_fast_when_invoked_inside_opx_managed_command() {
  let temp_dir = tempfile::tempdir().unwrap();
  fs::write(
    temp_dir.path().join("package.json"),
    r#"{"packageManager":"pnpm@10.0.0"}"#,
  )
  .unwrap();

  opx_command(temp_dir.path())
    .env("OPX_DEPTH", "1")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Refusing to run opx inside an opx-managed command",
    ));
}

#[test]
fn fails_when_op_is_not_on_path() {
  let temp_dir = tempfile::tempdir().unwrap();
  let empty_bin_dir = temp_dir.path().join("empty-bin");
  fs::create_dir_all(&empty_bin_dir).unwrap();
  fs::write(
    temp_dir.path().join("package.json"),
    r#"{"packageManager":"pnpm@10.0.0"}"#,
  )
  .unwrap();

  opx_command(temp_dir.path())
    .env("PATH", empty_bin_dir)
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "Failed to start 1Password CLI `op`",
    ));
}

#[test]
fn reports_failure_from_op_child_process() {
  let temp_dir = tempfile::tempdir().unwrap();
  let project_dir = temp_dir.path();
  let bin_dir = project_dir.join("bin");
  let args_file = project_dir.join("args.txt");
  let force_color_file = project_dir.join("force_color.txt");

  create_mock_op(&bin_dir);
  fs::write(
    project_dir.join("package.json"),
    r#"{"packageManager":"pnpm@10.0.0"}"#,
  )
  .unwrap();

  opx_command(project_dir)
    .env("PATH", prepend_path(&bin_dir))
    .env("OPX_MOCK_ARGS", args_file)
    .env("OPX_MOCK_FORCE_COLOR", force_color_file)
    .env("OPX_MOCK_EXIT", "9")
    .assert()
    .failure()
    .stderr(predicates::str::contains(
      "The command launched by opx exited unsuccessfully",
    ));
}
