use super::{confirm_production, format_error_message, production_banner_lines};
use anyhow::anyhow;
use std::io::Cursor;
use unicode_width::UnicodeWidthStr;

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

#[test]
fn production_confirmation_requires_exact_lowercase_yes() {
  for rejected in ["y\n", "Y\n", "YES\n", "yes \n", "\n"] {
    let mut output = Vec::new();
    let error = confirm_production(&mut Cursor::new(rejected), &mut output).unwrap_err();

    assert!(error
      .to_string()
      .contains("Production secrets were not confirmed."));
    assert!(String::from_utf8(output)
      .unwrap()
      .contains("YOU ARE LOADING PRODUCTION SECRETS 🚨"));
  }

  let mut output = Vec::new();
  confirm_production(&mut Cursor::new("yes\n"), &mut output).unwrap();
  assert!(String::from_utf8(output)
    .unwrap()
    .contains("Type the exact word `yes` to continue:"));
}

#[test]
fn production_banner_rows_have_equal_terminal_widths() {
  let [headline, detail] = production_banner_lines();

  assert_eq!(headline.width(), detail.width());
}
