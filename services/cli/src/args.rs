use std::net::IpAddr;

use clap::{FromArgMatches, Parser, Subcommand};
#[cfg(feature = "completions")]
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[clap(version, about, author)]
pub struct CliArgs<T = CliServerCommand>
where
    T: Subcommand + FromArgMatches,
{
    #[arg(long, short = 'c', default_value = "./config.yaml")]
    pub config: String,
    #[clap(subcommand)]
    pub command: Option<T>,
}

#[derive(Parser, Debug)]
pub enum CliServerCommand {
    Serve {
        #[clap(flatten)]
        serve: CliServerServe,
    },
    #[cfg(feature = "completions")]
    Completions { shell: Shell },
}

#[derive(Parser, Debug, Default)]
pub struct CliServerServe {
    #[arg(long, short = 'p')]
    pub port: Option<u16>,
    #[arg(long, short = 'h')]
    pub host: Option<IpAddr>,
}
