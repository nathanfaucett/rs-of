#![recursion_limit = "256"]

use std::io;

use tokio::runtime::Builder;

use unified_server::cli;

fn main() -> io::Result<()> {
    // Owner bootstrap and retained-state verification need more than Tokio's default stack.
    Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(64 * 1024 * 1024)
        .build()?
        .block_on(cli::run())
}
