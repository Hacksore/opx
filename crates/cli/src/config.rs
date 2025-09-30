use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::env;
use std::fs::File;
use std::io::prelude::*;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpxConfig {
  pub package_manager: String,
  pub ignored_directories: Vec<String>,
  pub default_start_command: String,
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

/// Used to be a config file but now this is just a way to read the package.json
impl OpxConfig {
  pub fn new() -> Result<Self, Error> {
    let mut package_json_path = env::current_dir().unwrap();
    // add the file config name
    package_json_path.push(PACKAGE_JSON_FILE);

    if !package_json_path.exists() {
      println!("[OPX] Can't find \"package.json\" in the current directory.")
    }

    let mut file = File::open(package_json_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // read the packageManager field to see if it's npm or yarn
    let package_json: Value = serde_json::from_str(&contents)?;
    let mut package_manager: String = String::from("npm");

    if !package_json["packageManager"].is_string() {
      println!("[OPX] Can't find \"packageManager\" in the \"package.json\" file.");
    } else {
      // extract the package manager from the before the @ symbol
      let raw_package_manager = package_json["packageManager"].as_str().unwrap();
      let parts = raw_package_manager.split("@").collect::<Vec<&str>>();
      package_manager = parts[0].to_string();

      println!("[OPX] Using package manager {package_manager}");
    }

    // Parse opx configuration section
    let mut ignored_directories = vec![".git".to_string(), "node_modules".to_string()];
    let mut default_start_command = "start".to_string();

    if let Some(opx_config) = package_json.get("opx") {
      println!("[OPX] Found opx configuration in package.json");
      
      if let Some(ignored_dirs) = opx_config.get("ignoredDirectories") {
        if let Some(dirs_array) = ignored_dirs.as_array() {
          ignored_directories = dirs_array
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect();
          println!("[OPX] Using custom ignored directories: {:?}", ignored_directories);
        }
      } else {
        println!("[OPX] Using default ignored directories: {:?}", ignored_directories);
      }

      if let Some(default_cmd) = opx_config.get("defaultStartCommand") {
        if let Some(cmd_str) = default_cmd.as_str() {
          default_start_command = cmd_str.to_string();
          println!("[OPX] Using custom default start command: {}", default_start_command);
        }
      } else {
        println!("[OPX] Using default start command: {}", default_start_command);
      }
    } else {
      println!("[OPX] No opx configuration found, using defaults");
      println!("[OPX] Using default ignored directories: {:?}", ignored_directories);
      println!("[OPX] Using default start command: {}", default_start_command);
    }

    let instance = OpxConfig { 
      package_manager,
      ignored_directories,
      default_start_command,
    };

    // Initialize default values for your properties
    Ok(instance)
  }

  /// Get the package manager from the package.json
  pub fn get_package_manager(&self) -> &String {
    &self.package_manager
  }

  /// Get the ignored directories from the opx config
  pub fn get_ignored_directories(&self) -> &Vec<String> {
    &self.ignored_directories
  }

  /// Get the default start command from the opx config
  pub fn get_default_start_command(&self) -> &String {
    &self.default_start_command
  }
}
