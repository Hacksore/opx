use anyhow::{Context, Result};
use serde_json::Value;
use std::env;
use std::fs;

#[derive(Debug)]
pub struct OpxConfig {
  pub package_manager: String,
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

/// Used to be a config file but now this is just a way to read the package.json
impl OpxConfig {
  pub fn new() -> Result<Self> {
    let mut package_json_path = env::current_dir().context("Failed to get current directory")?;
    // add the file config name
    package_json_path.push(PACKAGE_JSON_FILE);

    if !package_json_path.exists() {
      println!("[OPX] Can't find \"package.json\" in the current directory.")
    }

    let contents = fs::read_to_string(&package_json_path)
      .with_context(|| format!("Failed to read {}", package_json_path.display()))?;

    // read the packageManager field to see if it's npm or yarn
    let package_json: Value = serde_json::from_str(&contents)
      .with_context(|| format!("Failed to parse {}", package_json_path.display()))?;
    let mut package_manager: String = String::from("npm");

    if let Some(raw_package_manager) = package_json["packageManager"].as_str() {
      package_manager = raw_package_manager
        .split_once('@')
        .map_or(raw_package_manager, |(manager, _)| manager)
        .to_string();

      println!("[OPX] Using package manager {package_manager}");
    } else {
      println!("[OPX] Can't find \"packageManager\" in the \"package.json\" file.");
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
