use anyhow::{bail, Context, Result};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

pub const PTY_PROXY_ARG: &str = "__opx_pty_proxy";

struct RawModeGuard;

impl RawModeGuard {
  fn enable() -> Result<Self> {
    enable_raw_mode().context("Failed to enable raw terminal input for the PTY proxy.")?;
    Ok(Self)
  }
}

impl Drop for RawModeGuard {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
  }
}

pub fn run(command_args: Vec<OsString>) -> Result<u32> {
  let (program, args) = command_args
    .split_first()
    .ok_or_else(|| anyhow::anyhow!("Missing command for the internal opx PTY proxy."))?;

  let pty_system = native_pty_system();
  let pair = pty_system
    .openpty(PtySize::default())
    .context("Failed to open a pseudo-terminal.")?;

  let mut command = CommandBuilder::new(program);
  command.args(args);
  command.cwd(
    std::env::current_dir()
      .context("Failed to determine the pseudo-terminal working directory.")?,
  );

  let mut child = pair
    .slave
    .spawn_command(command)
    .context("Failed to start command in a pseudo-terminal.")?;
  drop(pair.slave);

  let mut reader = pair
    .master
    .try_clone_reader()
    .context("Failed to open the pseudo-terminal output stream.")?;
  let writer = pair
    .master
    .take_writer()
    .context("Failed to open the pseudo-terminal input stream.")?;
  let writer = Arc::new(Mutex::new(Some(writer)));
  let _raw_mode = RawModeGuard::enable()?;

  let output_thread = thread::spawn(move || -> io::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut buffer = [0_u8; 4096];

    loop {
      let count = match reader.read(&mut buffer) {
        Ok(0) => break,
        Ok(count) => count,
        Err(error) if error.raw_os_error() == Some(5) => break,
        Err(error) => return Err(error),
      };
      stdout.write_all(&buffer[..count])?;
      stdout.flush()?;
    }

    Ok(())
  });

  let input_writer = Arc::clone(&writer);
  thread::spawn(move || {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut buffer = [0_u8; 1024];

    loop {
      let count = match stdin.read(&mut buffer) {
        Ok(0) | Err(_) => break,
        Ok(count) => count,
      };
      let mut writer = input_writer.lock().unwrap();
      let Some(writer) = writer.as_mut() else {
        break;
      };

      if writer.write_all(&buffer[..count]).is_err() || writer.flush().is_err() {
        break;
      }
    }
  });

  let status = child
    .wait()
    .context("Failed while waiting for the pseudo-terminal command.")?;
  drop(writer.lock().unwrap().take());

  match output_thread.join() {
    Ok(result) => result.context("Failed to forward pseudo-terminal output.")?,
    Err(_) => bail!("The pseudo-terminal output thread panicked."),
  }

  Ok(status.exit_code())
}
