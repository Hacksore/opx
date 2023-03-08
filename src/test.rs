// TODO: make this more useful?
#[test]
fn test() {
  let env_files = get_env_files();
  let vec = vec!["start".to_string()];
  run_op_command(env_files, vec.into_iter());
}