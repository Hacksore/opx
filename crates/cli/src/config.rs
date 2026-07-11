use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::fs;
use tracing::{debug, warn};

#[derive(Debug)]
pub struct OpxConfig {
  pub package_manager: String,
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

/// Used to be a config file but now this is just a way to read the package.json
impl OpxConfig {
  pub fn new() -> Result<Self> {
    let current_dir = env::current_dir().context(
      "Failed to determine the current working directory.\n\nhint: Run opx from a project directory that still exists on disk.",
    )?;

    let mut package_json_path = current_dir.clone();
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
    let mut package_manager: String = String::from("npm");

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
        "packageManager not found in package.json; defaulting to npm.\n\nhint: Add packageManager to package.json to make this explicit."
      );
    }

    let instance = OpxConfig { package_manager };

    // Initialize default values for your properties
    Ok(instance)
  }

  /// Get the package manager from the package.json
  pub fn get_package_manager(&self) -> &str {
    &self.package_manager
  }
}
