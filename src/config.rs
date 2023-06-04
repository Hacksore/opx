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
}

/// The filename for the package.json
const PACKAGE_JSON_FILE: &str = "package.json";

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
    println!("packageManager: {}", package_json["packageManager"]);

    if !package_json["packageManager"].is_string() {
      println!("[OPX] Can't find \"packageManager\" in the \"package.json\" file.");
    } else {
      println!("[OPX] Found \"packageManager\" in the \"package.json\" file.");
      // extrac the package manager from the before the @ symbol
      let raw_package_manager = package_json["packageManager"].as_str().unwrap();
      let parts = raw_package_manager.split("@").collect::<Vec<&str>>();
      package_manager = parts.get(0).unwrap().to_owned()
    }

    let instance = OpxConfig { package_manager };

    // Initialize default values for your properties
    Ok(instance)
  }

  pub fn get_package_manager(&self) -> &String {
    &self.package_manager
  }
}
