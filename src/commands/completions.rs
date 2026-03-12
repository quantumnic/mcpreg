use crate::error::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

/// Generate shell completions for mcpreg.
pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "mcpreg", &mut std::io::stdout());
    Ok(())
}
