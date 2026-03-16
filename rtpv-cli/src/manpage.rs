//! Man page generation using clap_mangen.
//!
//! `rtpv man` prints a roff-formatted man page to stdout.
//!
//!   rtpv man > /usr/local/share/man/man1/rtpv.1
//!   rtpv man | gzip > /usr/local/share/man/man1/rtpv.1.gz

use anyhow::Result;
use clap::CommandFactory;
use clap_mangen::Man;

use crate::Cli;

pub fn print_man() -> Result<()> {
    let cmd = Cli::command();
    let man = Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    print!("{}", String::from_utf8_lossy(&buf));
    Ok(())
}
