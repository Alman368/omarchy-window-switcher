use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::thread;
use std::time::Duration;

const SOCKET_NAME: &str = "mogtab.sock";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    Alt,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IpcCommand {
    Open {
        profile: Profile,
        direction: Direction,
    },
    Close,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedArgs {
    Command(IpcCommand),
    Help,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("mogtabctl: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let ParsedArgs::Command(command) = parse_args(env::args().skip(1))? else {
        println!("{}", help_text());
        return Ok(ExitCode::SUCCESS);
    };

    match send_ipc(command) {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(err) if matches!(command, IpcCommand::Open { .. }) => {
            send_after_start(command, err).map(|()| ExitCode::SUCCESS)
        }
        Err(_) if matches!(command, IpcCommand::Close | IpcCommand::Cancel) => {
            Ok(ExitCode::SUCCESS)
        }
        Err(err) => Err(err),
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<ParsedArgs, String> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Ok(ParsedArgs::Help);
    };

    match command.as_str() {
        "-h" | "--help" | "help" => Ok(ParsedArgs::Help),
        "close" => Ok(ParsedArgs::Command(IpcCommand::Close)),
        "cancel" => Ok(ParsedArgs::Command(IpcCommand::Cancel)),
        "open" => parse_open_args(args),
        _ => Err(format!("unknown command: {command}\n\n{}", help_text())),
    }
}

fn parse_open_args(args: impl IntoIterator<Item = String>) -> Result<ParsedArgs, String> {
    let mut profile = Profile::Alt;
    let mut direction = Direction::Next;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                let Some(value) = args.next() else {
                    return Err("--profile requires alt or super".to_string());
                };
                profile = parse_profile(&value)?;
            }
            "alt" | "super" => profile = parse_profile(&arg)?,
            "next" | "prev" => direction = parse_direction(&arg)?,
            "-h" | "--help" => return Ok(ParsedArgs::Help),
            _ => return Err(format!("unknown open argument: {arg}\n\n{}", help_text())),
        }
    }

    Ok(ParsedArgs::Command(IpcCommand::Open { profile, direction }))
}

fn parse_profile(value: &str) -> Result<Profile, String> {
    match value {
        "alt" => Ok(Profile::Alt),
        "super" => Ok(Profile::Super),
        _ => Err(format!("invalid profile: {value}")),
    }
}

fn parse_direction(value: &str) -> Result<Direction, String> {
    match value {
        "next" => Ok(Direction::Next),
        "prev" => Ok(Direction::Prev),
        _ => Err(format!("invalid direction: {value}")),
    }
}

fn send_after_start(command: IpcCommand, first_err: String) -> Result<(), String> {
    start_daemon()?;

    for _ in 0..20 {
        thread::sleep(Duration::from_millis(25));
        if send_ipc(command).is_ok() {
            return Ok(());
        }
    }

    Err(format!(
        "daemon was not reachable after auto-start: {first_err}"
    ))
}

fn start_daemon() -> Result<(), String> {
    let daemon = daemon_path();
    Command::new(&daemon)
        .arg("run")
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("failed to auto-start {daemon}: {err}"))
}

fn daemon_path() -> String {
    let Ok(current_exe) = env::current_exe() else {
        return "mogtab".to_string();
    };

    let Some(parent) = current_exe.parent() else {
        return "mogtab".to_string();
    };

    let sibling = parent.join("mogtab");
    if fs::metadata(&sibling).is_ok() {
        sibling.to_string_lossy().into_owned()
    } else {
        "mogtab".to_string()
    }
}

fn send_ipc(command: IpcCommand) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket_path())
        .map_err(|err| format!("switcher daemon is not running: {err}"))?;
    stream
        .write_all(ipc_payload(command).as_bytes())
        .map_err(|err| format!("failed to send IPC command: {err}"))
}

fn ipc_payload(command: IpcCommand) -> &'static str {
    match command {
        IpcCommand::Open {
            profile: Profile::Alt,
            direction: Direction::Next,
        } => "open alt next\n",
        IpcCommand::Open {
            profile: Profile::Alt,
            direction: Direction::Prev,
        } => "open alt prev\n",
        IpcCommand::Open {
            profile: Profile::Super,
            direction: Direction::Next,
        } => "open super next\n",
        IpcCommand::Open {
            profile: Profile::Super,
            direction: Direction::Prev,
        } => "open super prev\n",
        IpcCommand::Close => "close\n",
        IpcCommand::Cancel => "cancel\n",
    }
}

fn socket_path() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join(SOCKET_NAME)
}

fn help_text() -> String {
    "Usage:
  mogtabctl open [--profile alt|super] [next|prev]
  mogtabctl close
  mogtabctl cancel"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_alt_next() {
        assert_eq!(
            parse_args(["open"].into_iter().map(String::from)),
            Ok(ParsedArgs::Command(IpcCommand::Open {
                profile: Profile::Alt,
                direction: Direction::Next,
            }))
        );
    }

    #[test]
    fn parses_existing_hotkey_shape() {
        assert_eq!(
            parse_args(
                ["open", "--profile", "super", "prev"]
                    .into_iter()
                    .map(String::from),
            ),
            Ok(ParsedArgs::Command(IpcCommand::Open {
                profile: Profile::Super,
                direction: Direction::Prev,
            }))
        );
    }

    #[test]
    fn help_is_not_a_parse_error() {
        assert_eq!(
            parse_args(["--help"].into_iter().map(String::from)),
            Ok(ParsedArgs::Help)
        );
    }
}
