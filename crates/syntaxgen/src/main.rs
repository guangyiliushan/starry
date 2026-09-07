use std::process::ExitCode;

use syntaxgen::{check, workspace_root, write_all};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--check") {
        match check(workspace_root()) {
            Ok(()) => {
                println!("generated vocabulary is current");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("syntaxgen: {error}");
                ExitCode::FAILURE
            }
        }
    } else {
        match write_all(workspace_root()) {
            Ok(()) => {
                println!("generated vocabulary");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("syntaxgen: {error}");
                ExitCode::FAILURE
            }
        }
    }
}
