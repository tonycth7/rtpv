// build.rs — generate shell completions at build time
// Completions land in OUT_DIR and are installed by install.sh

use std::io::Error;

fn main() -> Result<(), Error> {
    // OUT_DIR is set by cargo
    // We just signal that build.rs exists; the actual generation
    // happens via `rtpv completions` at runtime (simpler, no macro magic needed)
    Ok(())
}
