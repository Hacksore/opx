use anyhow::{Result, Error};
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::prelude::*;
use std::env;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpxConfig {
  #[serde(skip)]
  pub config_path: Option<PathBuf>,
  pub package_manager: String
}

/// The filename for our config
const CONFIG_FILE_NAME: &str = "opx.json";

impl OpxConfig {
  pub fn new() -> Result<Self, Error> {
    
    let mut config_path = env::current_dir().unwrap();
    // add the file config name
    config_path.push(CONFIG_FILE_NAME);

    let mut instance = OpxConfig {
      package_manager: String::from("npm"),
      config_path: Some(config_path.clone())
    };

    if !config_path.exists() {      
      println!("[OPX] Can't find config file \"opx.json\", using default values");
    } else {
      // load internally and spread into?
      let config = instance.load()?;
      instance = config;
    }

    // Initialize default values for your properties
    Ok(instance)
  }

  // Save the config as a JSON file
  // pub fn save(&self) -> Result<()> {
  //   // TODO: don't clone
  //   let mut file = File::create(self.config_path.clone().unwrap())?;
  //   let json_data = serde_json::to_string_pretty(self)?;
  //   file.write_all(json_data.as_bytes())?;
  //   Ok(())
  // }

  // Load the config from a JSON file
  pub fn load(&self) -> Result<OpxConfig> {
    // TODO: don't clone
    let mut file = File::open(self.config_path.clone().unwrap())?;
    let mut contents = String::new();

    // write the file contents to the string 
    file.read_to_string(&mut contents)?;

    // attempt to deserialize this into the struct
    let config: OpxConfig = serde_json::from_str(&contents)?;

    Ok(config)

  }

  // Getters for your properties
  pub fn get_package_manager(&self) -> &String {
    &self.package_manager
  }

  // Getters for your properties
  // pub fn get_config(&self) -> &OpxConfig {
  //   self
  // }
}
