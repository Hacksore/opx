use serde::Deserialize;

use std::fs;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpxConfig {
  pub package_manager: String
}

/// The filename for our config
const CONFIG_FILE_NAME: &str = "opx.json";

impl OpxConfig {
  pub fn new() -> OpxConfig {
    // add in the file that should be in the directory
    let mut file_path = std::env::current_dir().unwrap();
    file_path.push(CONFIG_FILE_NAME);

    println!("Looking for: {}", file_path.to_string_lossy());
    
    let contents = fs::read_to_string(file_path).expect("Could not read file");
    serde_json::from_str(&contents).expect("Could not parse configuration file")
  }

  pub fn get_package_manager(&self) -> String {
    // TODO: how do I not clone this?
    return self.package_manager.clone();
  }

  pub fn save(&self) {
    // write config to a file
    todo!()
  }
}
