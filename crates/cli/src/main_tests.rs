use super::format_error_message;
use anyhow::anyhow;

#[test]
fn formatted_errors_hide_where_and_include_footer_code() {
  let error = anyhow!(
    "Failed to find package.json.\n\nwhere: /repo\nwhy: opx reads package.json to choose which package manager to run.\nhint: Run opx from the root of a JavaScript project."
  );

  let message = format_error_message(&error);

  assert!(message.contains("Failed to find package.json."));
  assert!(message.contains("opx reads package.json to choose which package manager to run."));
  assert!(message.contains("Run opx from the root of a JavaScript project."));
  assert!(message.contains("EOPX_PACKAGE_JSON_NOT_FOUND"));
  assert!(message.contains("Command failed with exit code 1."));
  assert!(!message.contains("/repo"));
  assert!(!message.contains("where:"));
}
