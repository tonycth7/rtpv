//! `rtpv env` / `rtpv dotenv` — export entry fields as shell env vars.
//!
//!   eval $(rtpv env aws/prod)          # export to current shell
//!   rtpv dotenv db/prod > .env         # write .env file

use anyhow::Result;
use crate::config::Config;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvFormat { Shell, DotEnv }

pub struct EnvOutput {
    pub lines: Vec<(String, String)>,   // (VAR_NAME, value)
    pub format: EnvFormat,
}

impl EnvOutput {
    pub fn to_string(&self) -> String {
        self.lines.iter().map(|(k, v)| {
            match self.format {
                EnvFormat::Shell  => format!("export {}={}", k, shell_quote(v)),
                EnvFormat::DotEnv => format!("{}={}", k, v),
            }
        }).collect::<Vec<_>>().join("\n")
    }
}

pub fn run(cfg: &Config, path: &str, format: EnvFormat) -> Result<EnvOutput> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;

    let mut lines = Vec::new();
    let prefix = "RTPV";

    // Always export password
    lines.push((format!("{}_PASSWORD", prefix), entry.password.clone()));

    // Standard field mappings
    let mappings: &[(&str, &str)] = &[
        ("username", "USERNAME"),
        ("user",     "USERNAME"),
        ("email",    "EMAIL"),
        ("host",     "HOST"),
        ("port",     "PORT"),
        ("database", "DATABASE"),
        ("token",    "TOKEN"),
    ];

    for (field, var_suffix) in mappings {
        if let Some(val) = entry.get_field(field) {
            let var = format!("{}_{}", prefix, var_suffix);
            // Don't duplicate
            if !lines.iter().any(|(k, _)| k == &var) {
                lines.push((var, val.to_string()));
            }
        }
    }

    // Also export any remaining custom fields as RTPV_<FIELD>
    for (k, v) in &entry.fields {
        let var = format!("{}_{}", prefix, k.to_uppercase().replace('-', "_"));
        if !lines.iter().any(|(key, _)| key == &var) {
            lines.push((var, v.clone()));
        }
    }

    Ok(EnvOutput { lines, format })
}

/// Quote a value for shell export — wraps in single quotes, escaping any ' inside.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ─────────────────────────────────────────────────────────────────────────────
// `rtpv run` — inject credentials as env vars into a subprocess
// ─────────────────────────────────────────────────────────────────────────────
pub fn run_with_env(cfg: &Config, path: &str, command: &[String]) -> Result<std::process::ExitStatus> {
    if command.is_empty() {
        anyhow::bail!("No command specified");
    }

    let env_out = run(cfg, path, EnvFormat::Shell)?;

    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..]);

    for (k, v) in &env_out.lines {
        cmd.env(k, v);
    }

    // Smart mappings for common tools
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    let tool = command[0].as_str();

    match tool {
        "psql" => {
            if let Some(pw) = entry.get_field("password").or(Some(&entry.password)) {
                cmd.env("PGPASSWORD", pw);
            }
            if let Some(u) = entry.username()             { cmd.env("PGUSER",     u); }
            if let Some(h) = entry.get_field("host")      { cmd.env("PGHOST",     h); }
            if let Some(p) = entry.get_field("port")      { cmd.env("PGPORT",     p); }
            if let Some(d) = entry.get_field("database")  { cmd.env("PGDATABASE", d); }
        }
        "mysql" | "mysqldump" => {
            cmd.env("MYSQL_PWD", &entry.password);
        }
        "aws" => {
            if let Some(tok) = entry.token() {
                // Split "AKID:SECRET" format
                if let Some((key, secret)) = tok.split_once(':') {
                    cmd.env("AWS_ACCESS_KEY_ID",     key);
                    cmd.env("AWS_SECRET_ACCESS_KEY", secret);
                }
            }
        }
        _ => {}
    }

    Ok(cmd.status()?)
}
