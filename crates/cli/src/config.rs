use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

const DEFAULT_PACKAGE_MANAGER: &str = "npm";
const DEFAULT_SCRIPT: &str = "dev";

#[derive(Debug)]
pub struct OpxConfig {
  pub package_manager: String,
  pub default_script: String,
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

/// Used to be a config file but now this is just a way to read the package.json
impl OpxConfig {
  fn from_package_json(package_json: &Value, package_json_path: &Path) -> Self {
    let mut package_manager: String = String::from(DEFAULT_PACKAGE_MANAGER);
    let mut default_script: String = String::from(DEFAULT_SCRIPT);

    if let Some(raw_package_manager) = package_json["packageManager"].as_str() {
      package_manager = raw_package_manager
        .split_once('@')
        .map_or(raw_package_manager, |(manager, _)| manager)
        .to_string();

      debug!(package_manager, "Resolved package manager");
    } else {
      warn!(
        package_json = %package_json_path.display(),
        package_manager,
        "packageManager not found in package.json; defaulting to npm. hint: Add packageManager to package.json to make this explicit."
      );
    }

    if let Some(opx_config) = package_json.get("opx") {
      if let Some(raw_default_script) = opx_config["defaultScript"].as_str() {
        let configured_default_script = raw_default_script.trim();

        if configured_default_script.is_empty() {
          warn!(
            package_json = %package_json_path.display(),
            default_script,
            "opx.defaultScript in package.json is empty; defaulting to dev. hint: Set opx.defaultScript to a package script name like `start` or `server`."
          );
        } else {
          default_script = configured_default_script.to_string();
          debug!(default_script, "Resolved default script");
        }
      } else if !opx_config.is_object() {
        warn!(
          package_json = %package_json_path.display(),
          default_script,
          "opx in package.json must be an object; defaulting to dev. hint: Configure it as `\"opx\": {{ \"defaultScript\": \"start\" }}`."
        );
      }
    }

    OpxConfig {
      package_manager,
      default_script,
    }
  }

  fn from_directory(current_dir: &Path) -> Result<Self> {
    let mut package_json_path = current_dir.to_path_buf();
    // add the file config name
    package_json_path.push(PACKAGE_JSON_FILE);

    if !package_json_path.exists() {
      bail!(
        "Failed to find package.json.\n\nwhere: {}\nwhy: opx reads package.json to choose which package manager to run.\nhint: Run opx from the root of a JavaScript project, or add a package.json file with a packageManager field.",
        current_dir.display()
      );
    }

    let contents = fs::read_to_string(&package_json_path).with_context(|| {
      format!(
        "Failed to read package.json.\n\nwhere: {}\nhint: Check that the file exists and that your user has permission to read it.",
        package_json_path.display()
      )
    })?;

    // read the packageManager field to see if it's npm or yarn
    let package_json: Value = serde_json::from_str(&contents).with_context(|| {
      format!(
        "Failed to parse package.json as JSON.\n\nwhere: {}\nhint: Fix the JSON syntax, then run opx again.",
        package_json_path.display()
      )
    })?;
    let instance = OpxConfig::from_package_json(&package_json, &package_json_path);

    // Initialize default values for your properties
    Ok(instance)
  }

  pub fn new() -> Result<Self> {
    let current_dir = env::current_dir().context(
      "Failed to determine the current working directory.\n\nhint: Run opx from a project directory that still exists on disk.",
    )?;

    Self::from_directory(&current_dir)
  }

  /// Get the package manager from the package.json
  pub fn get_package_manager(&self) -> &str {
    &self.package_manager
  }

  /// Get the default script from package.json opx config
  pub fn get_default_script(&self) -> &str {
    &self.default_script
  }
}

#[cfg(test)]
mod tests {
  use super::OpxConfig;
  use serde_json::json;
  use std::fs;
  use std::path::Path;

  fn parse(package_json: serde_json::Value) -> OpxConfig {
    OpxConfig::from_package_json(&package_json, Path::new("package.json"))
  }

  #[test]
  fn defaults_to_npm_and_dev_without_package_overrides() {
    let config = parse(json!({}));

    assert_eq!(config.get_package_manager(), "npm");
    assert_eq!(config.get_default_script(), "dev");
  }

  #[test]
  fn reads_package_manager_without_version_suffix() {
    let config = parse(json!({
      "packageManager": "pnpm@10.0.0"
    }));

    assert_eq!(config.get_package_manager(), "pnpm");
    assert_eq!(config.get_default_script(), "dev");
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
  }

  #[test]
  fn ignores_empty_default_script() {
    let config = parse(json!({
      "opx": {
        "defaultScript": " "
      }
    }));

    assert_eq!(config.get_default_script(), "dev");
  }

  #[test]
  fn new_errors_when_package_json_is_missing() {
    let temp_dir = tempfile::tempdir().unwrap();

    let error = OpxConfig::from_directory(temp_dir.path()).unwrap_err();

    assert!(error.to_string().contains("Failed to find package.json"));
    assert!(error
      .to_string()
      .contains(temp_dir.path().to_string_lossy().as_ref()));
  }

  #[test]
  fn new_errors_when_package_json_is_invalid_json() {
    let temp_dir = tempfile::tempdir().unwrap();
    fs::write(temp_dir.path().join("package.json"), "{").unwrap();

    let error = OpxConfig::from_directory(temp_dir.path()).unwrap_err();

    assert!(error
      .to_string()
      .contains("Failed to parse package.json as JSON"));
  }

  #[cfg(unix)]
  #[test]
  fn new_errors_when_package_json_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let package_json_path = temp_dir.path().join("package.json");
    fs::write(&package_json_path, "{}").unwrap();
    fs::set_permissions(&package_json_path, fs::Permissions::from_mode(0o000)).unwrap();

    let error = OpxConfig::from_directory(temp_dir.path()).unwrap_err();

    fs::set_permissions(&package_json_path, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(error.to_string().contains("Failed to read package.json"));
  }
}
