mod live;
mod navigation;
mod terminal;

use std::env;
use std::process::ExitCode;

use navigation::Directory;

fn main() -> ExitCode {
    let result = env::current_dir()
        .and_then(|path| Directory::open(&path))
        .and_then(terminal::run);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FolderWalk: {error}");
            ExitCode::FAILURE
        }
    }
}
