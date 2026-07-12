use super::OpxConfig;
use serde_json::json;
use std::path::Path;

fn parse(package_json: serde_json::Value) -> OpxConfig {
  OpxConfig::from_package_json(&package_json, Path::new("package.json")).unwrap()
}

#[test]
fn defaults_to_npm_and_dev_without_package_overrides() {
  let config = parse(json!({}));

  assert_eq!(config.get_package_manager(), "npm");
  assert_eq!(config.get_default_script(), "dev");
  assert_eq!(config.get_default_command(), None);
}

#[test]
fn reads_package_manager_without_version_suffix() {
  let config = parse(json!({
    "packageManager": "pnpm@10.0.0"
  }));

  assert_eq!(config.get_package_manager(), "pnpm");
}

#[test]
fn reads_default_script_from_opx_config() {
  let config = parse(json!({
    "packageManager": "yarn@4.0.0",
    "opx": {
      "defaultScript": "start"
    }
  }));

  assert_eq!(config.get_package_manager(), "yarn");
  assert_eq!(config.get_default_script(), "start");
  assert_eq!(config.get_default_command(), None);
}

#[test]
fn reads_default_command_string_from_opx_config() {
  let config = parse(json!({
    "packageManager": "pnpm@10.0.0",
    "opx": {
      "defaultCommand": "next dev --hostname \"0.0.0.0\""
    }
  }));

  assert_eq!(
    config.get_default_command(),
    Some(
      vec![
        "next".to_string(),
        "dev".to_string(),
        "--hostname".to_string(),
        "0.0.0.0".to_string(),
      ]
      .as_slice()
    )
  );
}

#[test]
fn reads_default_command_array_from_opx_config() {
  let config = parse(json!({
    "opx": {
      "defaultCommand": ["node", "-e", "console.log('hello world')"]
    }
  }));

  assert_eq!(
    config.get_default_command(),
    Some(
      vec![
        "node".to_string(),
        "-e".to_string(),
        "console.log('hello world')".to_string(),
      ]
      .as_slice()
    )
  );
}

#[test]
fn default_command_takes_precedence_over_default_script() {
  let config = parse(json!({
    "opx": {
      "defaultCommand": "next dev",
      "defaultScript": "server"
    }
  }));

  assert_eq!(config.get_default_script(), "dev");
  assert_eq!(
    config.get_default_command(),
    Some(vec!["next".to_string(), "dev".to_string()].as_slice())
  );
}

#[test]
fn rejects_invalid_default_command() {
  let error = OpxConfig::from_package_json(
    &json!({
      "opx": {
        "defaultCommand": "next dev \"unterminated"
      }
    }),
    Path::new("package.json"),
  )
  .unwrap_err();

  assert!(error
    .to_string()
    .contains("Failed to parse opx.defaultCommand"));
}
