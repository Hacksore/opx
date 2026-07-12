use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;

const DEFAULT_PACKAGE_MANAGER: &str = "npm";
const DEFAULT_SCRIPT: &str = "dev";

#[derive(Debug)]
pub struct OpxConfig {
  pub package_manager: String,
  pub default_script: String,
  pub default_command: Option<Vec<String>>,
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

/// Used to be a config file but now this is just a way to read the package.json
impl OpxConfig {
  fn parse_default_command(
    raw_default_command: &Value,
    package_json_path: &Path,
  ) -> Result<Option<Vec<String>>> {
    if let Some(command) = raw_default_command.as_str() {
      let command = command.trim();

      if command.is_empty() {
        warn!(
          package_json = %package_json_path.display(),
          default_script = DEFAULT_SCRIPT,
          "opx.defaultCommand in package.json is empty; defaulting to dev. hint: Set opx.defaultCommand to a raw command like `next dev`."
        );
        return Ok(None);
      }

      return shell_words::split(command)
        .with_context(|| {
          format!(
            "Failed to parse opx.defaultCommand.\n\nwhere: {}\nvalue: {}\nhint: Use shell-style quoting, for example `next dev --hostname \"0.0.0.0\"`, or use an array like [\"next\", \"dev\"].",
            package_json_path.display(),
            command
          )
        })
        .map(Some);
    }

    if let Some(command_parts) = raw_default_command.as_array() {
      let mut command = Vec::with_capacity(command_parts.len());

      for (index, command_part) in command_parts.iter().enumerate() {
        let Some(command_part) = command_part.as_str() else {
          bail!(
            "Invalid opx.defaultCommand.\n\nwhere: {}\nwhy: defaultCommand array item {} is not a string.\nhint: Use an array of command arguments, for example [\"next\", \"dev\"].",
            package_json_path.display(),
            index
          );
        };

        if command_part.is_empty() {
          bail!(
            "Invalid opx.defaultCommand.\n\nwhere: {}\nwhy: defaultCommand array item {} is empty.\nhint: Remove empty arguments or use a non-empty string argument.",
            package_json_path.display(),
            index
          );
        }

        command.push(command_part.to_string());
      }

      if command.is_empty() {
        warn!(
          package_json = %package_json_path.display(),
          default_script = DEFAULT_SCRIPT,
          "opx.defaultCommand in package.json is empty; defaulting to dev. hint: Set opx.defaultCommand to a raw command like `next dev`."
        );
        return Ok(None);
      }

      return Ok(Some(command));
    }

    bail!(
      "Invalid opx.defaultCommand.\n\nwhere: {}\nwhy: defaultCommand must be a string or an array of strings.\nhint: Use `\"defaultCommand\": \"next dev\"` or `\"defaultCommand\": [\"next\", \"dev\"]`.",
      package_json_path.display()
    );
  }

  fn from_package_json(package_json: &Value, package_json_path: &Path) -> Result<Self> {
    let mut package_manager: String = String::from(DEFAULT_PACKAGE_MANAGER);
    let mut default_script: String = String::from(DEFAULT_SCRIPT);
    let mut default_command: Option<Vec<String>> = None;

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
      if let Some(raw_default_command) = opx_config.get("defaultCommand") {
        default_command = Self::parse_default_command(raw_default_command, package_json_path)?;

        if let Some(default_command) = &default_command {
          debug!(default_command = %default_command.join(" "), "Resolved default command");
        }
      } else if let Some(raw_default_script) = opx_config["defaultScript"].as_str() {
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
          "opx in package.json must be an object; defaulting to dev. hint: Configure it as `\"opx\": {{ \"defaultScript\": \"start\" }}` or `\"opx\": {{ \"defaultCommand\": \"next dev\" }}`."
        );
      }
    }

    Ok(OpxConfig {
      package_manager,
      default_script,
      default_command,
    })
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
    let instance = OpxConfig::from_package_json(&package_json, &package_json_path)?;

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

  /// Get the raw default command from package.json opx config
  pub fn get_default_command(&self) -> Option<&[String]> {
    self.default_command.as_deref()
  }
}
