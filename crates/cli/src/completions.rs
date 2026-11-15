use std::io;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::args::CliServerCommand;

pub async fn run_completions(shell: Shell) -> io::Result<()> {
    generate(
        shell,
        &mut CliServerCommand::command(),
        env!("CARGO_PKG_NAME"),
        &mut io::stdout(),
    );
    Ok(())
}
