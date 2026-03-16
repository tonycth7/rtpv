// cli/commands.rs — implementation of every CLI command

use crate::audit;
use crate::clipboard;
use crate::config::Config;
use crate::gen::{self, GenMode};
use crate::git::GitSync;
use crate::otp;
use crate::ui::{self, Printer};
use crate::vault::{Entry, EntryKind, Vault};
use anyhow::{bail, Result};
use totp_rs::TOTP;

// ── init ─────────────────────────────────────────────────────────────────────

pub fn cmd_init(cfg: &Config, printer: &Printer, force: bool) -> Result<()> {
    if force {
        let pubkey = Vault::reinit(&cfg.vault_dir)?;
        printer.ok(&format!(
            "Re-initialized vault at {}",
            cfg.vault_dir.display()
        ));
        printer.info(&format!("Public key: {}", pubkey));
        printer.warn("All existing entries need re-encryption: rtpv recipients reencrypt");
    } else {
        let pubkey = Vault::init(&cfg.vault_dir)?;
        printer.ok(&format!("Vault initialized at {}", cfg.vault_dir.display()));
        printer.info(&format!("Public key: {}", pubkey));
        printer.dim("Add a git remote: git -C ~/.password-vault remote add origin git@github.com:user/vault.git");
        printer.dim("Generate config:  rtpv gen-conf");
    }
    Ok(())
}

// ── add ──────────────────────────────────────────────────────────────────────

pub struct AddArgs {
    pub path: String,
    pub kind: EntryKind,
    pub email: Option<String>,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub password: bool,
    pub words: Option<usize>,
    pub pin: Option<usize>,
    pub length: Option<usize>,
    pub no_print: bool,
}

pub fn cmd_add(vault: &Vault, cfg: &Config, printer: &Printer, args: AddArgs) -> Result<()> {
    if vault.exists(&args.path) {
        bail!(
            "Entry already exists: {}  (use: rtpv edit {})",
            args.path,
            args.path
        );
    }

    let mut entry = Entry::new(args.kind);

    // Resolve password
    let pw = if args.password {
        ui::prompt_password_confirm("Password")?
    } else if let Some(n) = args.words {
        gen::passphrase(n, '-')?
    } else if let Some(n) = args.pin {
        gen::pin(n)?
    } else {
        gen::random(args.length.unwrap_or(cfg.gen_length), &cfg.gen_chars)?
    };

    entry.fields.password = Some(pw.clone());
    if let Some(e) = args.email {
        entry.fields.email = Some(e);
    }
    if let Some(u) = args.username {
        entry.fields.username = Some(u);
    }
    if let Some(u) = args.url {
        entry.fields.url = Some(u);
    }
    if let Some(n) = args.notes {
        entry.fields.notes = Some(n);
    }

    vault.write(&args.path, &entry)?;

    printer.ok(&format!("Added: {}", args.path));
    if !args.no_print {
        printer.dim(&format!("Password: {}", pw));
    }

    if clipboard::available() {
        clipboard::copy_with_clear(&pw, cfg.clip_timeout)?;
        printer.dim(&format!(
            "Copied to clipboard (clears in {}s)",
            cfg.clip_timeout
        ));
    }

    autosync(vault, cfg, &format!("Add {}", args.path));
    Ok(())
}

// ── show ─────────────────────────────────────────────────────────────────────

pub fn cmd_show(vault: &Vault, printer: &Printer, path: &str, reveal: bool) -> Result<()> {
    let entry = vault.read(path)?;
    printer.banner(path);

    printer.info(&format!("type: {}", entry.meta.kind.display()));
    println!();

    macro_rules! show {
        ($label:expr, $val:expr) => {
            if let Some(v) = $val {
                printer.field($label, v);
            }
        };
    }

    if reveal {
        show!("password", entry.fields.password.as_deref());
    } else {
        if entry.fields.password.is_some() {
            printer.secret_field("password");
        }
    }

    show!("username", entry.fields.username.as_deref());
    show!("email", entry.fields.email.as_deref());
    show!("url", entry.fields.url.as_deref());
    show!("host", entry.fields.host.as_deref());
    show!("port", entry.fields.port.as_deref());
    show!("database", entry.fields.database.as_deref());
    show!("ssid", entry.fields.ssid.as_deref());
    show!("security", entry.fields.security.as_deref());
    show!("product", entry.fields.product.as_deref());

    if entry.fields.token.is_some() {
        printer.secret_field("token");
    }
    if entry.fields.number.is_some() {
        printer.secret_field("number");
    }
    if entry.fields.cvv.is_some() {
        printer.secret_field("cvv");
    }
    if entry.fields.pub_key.is_some() {
        printer.field("pub_key", "…(stored)");
    }
    if entry.fields.pvt_key.is_some() {
        printer.secret_field("pvt_key");
    }

    if entry.fields.has_otp() {
        printer.field("otp", "● configured");
    }

    if let Some(n) = &entry.fields.notes {
        println!();
        printer.section("Notes");
        for line in n.lines() {
            println!("  {}", line);
        }
    }

    for (k, v) in &entry.fields.extra {
        printer.field(k, v);
    }

    println!();
    printer.dim(&format!(
        "created:  {}",
        entry.meta.created.format("%Y-%m-%d")
    ));
    printer.dim(&format!(
        "modified: {}",
        entry.meta.modified.format("%Y-%m-%d")
    ));
    if entry.meta.starred {
        printer.dim("★ starred");
    }
    if !entry.meta.tags.is_empty() {
        printer.dim(&format!("tags: {}", entry.meta.tags.join(", ")));
    }
    println!();
    Ok(())
}

// ── copy ─────────────────────────────────────────────────────────────────────

pub fn cmd_copy(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
    username: bool,
    email: bool,
    token: bool,
    otp_flag: bool,
    notes: bool,
    field: Option<&str>,
) -> Result<()> {
    let entry = vault.read(path)?;

    let (field_name, value) = if let Some(f) = field {
        let v = entry
            .fields
            .get(f)
            .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in '{}'", f, path))?
            .to_string();
        (f.to_string(), v)
    } else if username {
        let v = entry
            .fields
            .username
            .as_deref()
            .or(entry.fields.email.as_deref())
            .ok_or_else(|| anyhow::anyhow!("No username/email in '{}'", path))?
            .to_string();
        ("username".to_string(), v)
    } else if email {
        let v = entry
            .fields
            .email
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No email in '{}'", path))?
            .to_string();
        ("email".to_string(), v)
    } else if token {
        let v = entry
            .fields
            .token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No token in '{}'", path))?
            .to_string();
        ("token".to_string(), v)
    } else if otp_flag {
        let uri = entry
            .fields
            .otp
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No OTP configured for '{}'", path))?;
        let state = otp::current_state(uri)?;
        printer.ok(&format!(
            "OTP {} ({}s remaining)",
            state.code, state.remaining
        ));
        clipboard::copy_with_clear(&state.code, state.remaining)?;
        return Ok(());
    } else if notes {
        let v = entry
            .fields
            .notes
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No notes in '{}'", path))?
            .to_string();
        ("notes".to_string(), v)
    } else {
        // Default: copy password (or primary field for type)
        let v = entry
            .fields
            .primary(&entry.meta.kind)
            .ok_or_else(|| anyhow::anyhow!("No primary secret in '{}'", path))?
            .to_string();
        ("password".to_string(), v)
    };

    clipboard::copy_with_clear(&value, cfg.clip_timeout)?;
    printer.ok(&format!(
        "Copied {} (clears in {}s)",
        field_name, cfg.clip_timeout
    ));
    Ok(())
}

// ── rm ───────────────────────────────────────────────────────────────────────

pub fn cmd_remove(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
    force: bool,
) -> Result<()> {
    if !vault.exists(path) {
        bail!("Entry not found: {}", path);
    }
    if !force && !ui::confirm(&format!("Delete '{}'?", path))? {
        printer.dim("Aborted");
        return Ok(());
    }
    vault.remove(path)?;
    printer.ok(&format!("Deleted: {}", path));
    autosync(vault, cfg, &format!("Delete {}", path));
    Ok(())
}

// ── edit ─────────────────────────────────────────────────────────────────────

pub fn cmd_edit(vault: &Vault, cfg: &Config, printer: &Printer, path: &str) -> Result<()> {
    if !vault.exists(path) {
        bail!("Entry not found: {}", path);
    }

    let entry = vault.read(path)?;
    let plaintext = entry.to_toml()?;

    let mut tmp = tempfile::NamedTempFile::new()?;
    use std::io::Write;
    tmp.write_all(plaintext.as_bytes())?;
    tmp.flush()?;
    let tmp_path = tmp.path().to_path_buf();

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()?;
    if !status.success() {
        bail!("Editor exited with error");
    }

    let edited = std::fs::read_to_string(&tmp_path)?;
    if edited.trim() == plaintext.trim() {
        printer.dim("No changes");
        return Ok(());
    }

    let mut updated = Entry::from_toml(&edited)?;
    updated.touch();
    vault.write(path, &updated)?;
    printer.ok(&format!("Saved: {}", path));
    autosync(vault, cfg, &format!("Edit {}", path));
    Ok(())
}

// ── rename ───────────────────────────────────────────────────────────────────

pub fn cmd_rename(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    src: &str,
    dst: &str,
) -> Result<()> {
    vault.rename(src, dst)?;
    printer.ok(&format!("{} → {}", src, dst));
    autosync(vault, cfg, &format!("Rename {} → {}", src, dst));
    Ok(())
}

// ── clone ────────────────────────────────────────────────────────────────────

pub fn cmd_clone(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    src: &str,
    dst: &str,
) -> Result<()> {
    vault.clone_entry(src, dst)?;
    printer.ok(&format!("Cloned {} → {}", src, dst));
    autosync(vault, cfg, &format!("Clone {} → {}", src, dst));
    Ok(())
}

// ── set ──────────────────────────────────────────────────────────────────────

pub fn cmd_set(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
    field: &str,
    value: Option<String>,
) -> Result<()> {
    let mut entry = vault.read(path)?;
    let value = match value {
        Some(v) => v,
        None => {
            let is_secret = matches!(
                field,
                "password" | "token" | "cvv" | "number" | "pvt_key" | "key"
            );
            if is_secret {
                ui::prompt_password(field)?
            } else {
                ui::prompt_text(field)?
            }
        }
    };
    entry.fields.set(field, value);
    entry.touch();
    vault.write(path, &entry)?;
    printer.ok(&format!("Set {} in {}", field, path));
    autosync(vault, cfg, &format!("Set {} in {}", field, path));
    Ok(())
}

// ── rotate ───────────────────────────────────────────────────────────────────

pub fn cmd_rotate(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
    length: Option<usize>,
) -> Result<()> {
    let mut entry = vault.read(path)?;
    let pw = gen::random(length.unwrap_or(cfg.gen_length), &cfg.gen_chars)?;
    entry.fields.password = Some(pw.clone());
    entry.touch();
    vault.write(path, &entry)?;
    printer.ok(&format!("Rotated password for {}", path));
    if clipboard::available() {
        clipboard::copy_with_clear(&pw, cfg.clip_timeout)?;
        printer.dim(&format!(
            "New password copied (clears in {}s)",
            cfg.clip_timeout
        ));
    }
    autosync(vault, cfg, &format!("Rotate {}", path));
    Ok(())
}

// ── list ─────────────────────────────────────────────────────────────────────

pub fn cmd_list(vault: &Vault, printer: &Printer, filter: Option<&str>) -> Result<()> {
    let entries = vault.list()?;
    let mut count = 0;
    for e in &entries {
        if let Some(f) = filter {
            if !e.contains(f) {
                continue;
            }
        }
        println!("{}", e);
        count += 1;
    }
    printer.dim(&format!("{} entries", count));
    Ok(())
}

// ── gen ──────────────────────────────────────────────────────────────────────

pub fn cmd_gen(
    cfg: &Config,
    printer: &Printer,
    words: Option<usize>,
    pin: Option<usize>,
    pronounceable: bool,
    length: Option<usize>,
    no_copy: bool,
) -> Result<()> {
    let mode = if let Some(n) = words {
        GenMode::Words { count: n, sep: '-' }
    } else if let Some(n) = pin {
        GenMode::Pin { length: n }
    } else if pronounceable {
        GenMode::Pronounceable {
            length: length.unwrap_or(16),
        }
    } else {
        GenMode::Random {
            length: length.unwrap_or(cfg.gen_length),
            chars: cfg.gen_chars.clone(),
        }
    };

    let pw = gen::generate(&mode)?;
    println!("{}", pw);

    if !no_copy && clipboard::available() {
        clipboard::copy_with_clear(&pw, cfg.clip_timeout)?;
        printer.dim(&format!("Copied (clears in {}s)", cfg.clip_timeout));
    }
    Ok(())
}

// ── otp ──────────────────────────────────────────────────────────────────────

pub fn cmd_otp_copy(vault: &Vault, printer: &Printer, path: &str) -> Result<()> {
    let entry = vault.read(path)?;
    let uri = entry
        .fields
        .otp
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No OTP configured for '{}'", path))?;
    let state = otp::current_state(uri)?;
    clipboard::copy_with_clear(&state.code, state.remaining)?;
    printer.ok(&format!(
        "OTP {} ({}s remaining)",
        state.code, state.remaining
    ));
    Ok(())
}

pub fn cmd_otp_show(vault: &Vault, printer: &Printer, path: &str) -> Result<()> {
    let entry = vault.read(path)?;
    let uri = entry
        .fields
        .otp
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No OTP configured for '{}'", path))?;
    let state = otp::current_state(uri)?;
    printer.bold(&state.code);
    println!("  Remaining: {}s", state.remaining);
    let bar_len = 30usize;
    let filled = (bar_len as f32 * (1.0 - state.progress)) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_len - filled);
    println!("  [{}]", bar);
    if let Some(i) = &state.issuer {
        println!("  Issuer:  {}", i);
    }
    if let Some(a) = &state.account {
        println!("  Account: {}", a);
    }
    Ok(())
}

pub fn cmd_otp_list(vault: &Vault, printer: &Printer) -> Result<()> {
    let entries = vault.list()?;
    let mut found = false;
    for path in &entries {
        if let Ok(entry) = vault.read(path) {
            if let Some(uri) = entry.fields.otp.as_deref() {
                if let Ok(state) = otp::current_state(uri) {
                    println!("  {:40} {} ({}s)", path, state.code, state.remaining);
                    found = true;
                }
            }
        }
    }
    if !found {
        printer.info("No OTP entries found");
    }
    Ok(())
}

pub fn cmd_otp_add(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
    uri: &str,
) -> Result<()> {
    let mut entry = vault.read(path)?;

    if !uri.starts_with("otpauth://") {
        anyhow::bail!("Invalid OTP URI");
    }

    entry.fields.otp = Some(uri.to_string());
    vault.write(path, &entry)?;

    println!("OTP added");

    Ok(())
}
// ── audit ────────────────────────────────────────────────────────────────────

pub fn cmd_audit(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    hibp: bool,
    fix: bool,
) -> Result<()> {
    printer.banner("Security Audit");
    let report = audit::run_audit(vault, cfg.max_age_days);
    printer.dim(&format!("Scanned {} entries", report.scanned));

    if report.issues.is_empty() {
        printer.ok("No issues found");
    } else {
        for issue in &report.issues {
            printer.warn(&format!(
                "[{}] {} — {}",
                issue.label(),
                issue.path,
                issue.detail
            ));
        }
    }

    if fix {
        printer.section("Auto-fixing weak passwords");
        for issue in &report.issues {
            if issue.kind == crate::audit::IssueKind::Weak {
                if let Ok(_) = cmd_rotate(vault, cfg, printer, &issue.path, None) {
                    printer.ok(&format!("Rotated: {}", issue.path));
                }
            }
        }
    }

    if hibp {
        printer.section("HIBP Check");
        let paths = vault.list()?;
        for path in &paths {
            if let Ok(entry) = vault.read(path) {
                if let Some(pw) = entry.fields.password.as_deref() {
                    print!("  {}... ", path);
                    match audit::hibp_check(pw, cfg.hibp_timeout) {
                        Ok(0) => println!("✔ safe"),
                        Ok(n) => println!("⚠ BREACHED ({} times)", n),
                        Err(e) => println!("✖ {}", e),
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
            }
        }
    }
    Ok(())
}

// ── hibp ─────────────────────────────────────────────────────────────────────

pub fn cmd_hibp(vault: &Vault, cfg: &Config, printer: &Printer, path: &str) -> Result<()> {
    let entry = vault.read(path)?;
    let pw = entry
        .fields
        .password
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No password in '{}'", path))?;
    printer.step("Checking HIBP...");
    let count = audit::hibp_check(pw, cfg.hibp_timeout)?;
    if count > 0 {
        printer.warn(&format!(
            "BREACHED — found {} times in HIBP database",
            count
        ));
    } else {
        printer.ok("Not found in HIBP");
    }
    Ok(())
}

// ── strength ─────────────────────────────────────────────────────────────────

pub fn cmd_strength(vault: &Vault, printer: &Printer, path: &str) -> Result<()> {
    let entry = vault.read(path)?;
    let pw = entry
        .fields
        .password
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("No password in '{}'", path))?;
    let s = gen::check_strength(pw);
    printer.banner(&format!("Strength: {}", path));
    println!("  Score:      {}/4  ({})", s.score, s.label);
    println!("  Entropy:    {:.1} bits", s.bits);
    println!("  Crack time: {}", s.crack_time);
    println!("  Length:     {}", pw.len());
    if !s.suggestions.is_empty() {
        println!();
        for sug in &s.suggestions {
            printer.dim(sug);
        }
    }
    Ok(())
}

// ── sync ─────────────────────────────────────────────────────────────────────

pub fn cmd_sync(vault: &Vault, cfg: &Config, printer: &Printer, remote: &str) -> Result<()> {
    let git = GitSync::open(&cfg.vault_dir)?;
    printer.step("Pulling...");
    git.pull(remote)?;
    printer.step("Pushing...");
    git.push(remote)?;
    printer.ok("Synced");
    Ok(())
}

// ── log ──────────────────────────────────────────────────────────────────────

pub fn cmd_log(cfg: &Config, printer: &Printer, count: usize) -> Result<()> {
    let git = GitSync::open(&cfg.vault_dir)?;
    printer.banner(&format!("History — last {} commits", count));
    for c in git.log(count)? {
        let ts = chrono::DateTime::from_timestamp(c.time, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        println!("  {}  {}  {}  ({})", c.hash, ts, c.message, c.author);
    }
    Ok(())
}

// ── fill (autotype) ───────────────────────────────────────────────────────────

pub fn cmd_fill(
    vault: &Vault,
    printer: &Printer,
    path: &str,
    delay_ms: u64,
    press_enter: bool,
) -> Result<()> {
    let entry = vault.read(path)?;
    let username = entry.fields.username.as_deref().unwrap_or("").to_string();
    let password = entry.fields.password.as_deref().unwrap_or("").to_string();

    let tool = if which("xdotool") {
        "xdotool"
    } else if which("ydotool") {
        "ydotool"
    } else {
        bail!("Install xdotool or ydotool for autofill");
    };

    std::thread::sleep(std::time::Duration::from_millis(delay_ms));

    if !username.is_empty() {
        xtype(tool, &username)?;
        xkey(tool, "Tab")?;
    }
    if !password.is_empty() {
        xtype(tool, &password)?;
        if press_enter {
            xkey(tool, "Return")?;
        }
    }
    printer.ok(&format!("Autofilled {}", path));
    Ok(())
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn xtype(tool: &str, text: &str) -> Result<()> {
    let args = if tool == "xdotool" {
        vec!["type", "--clearmodifiers", "--", text]
    } else {
        vec!["type", "--", text]
    };
    let s = std::process::Command::new(tool).args(&args).status()?;
    if !s.success() {
        bail!("{} type failed", tool);
    }
    Ok(())
}

fn xkey(tool: &str, key: &str) -> Result<()> {
    let s = std::process::Command::new(tool)
        .args(["key", key])
        .status()?;
    if !s.success() {
        bail!("{} key {} failed", tool, key);
    }
    Ok(())
}

// ── run (env inject) ─────────────────────────────────────────────────────────

pub fn cmd_run(vault: &Vault, path: &str, command: &[String]) -> Result<()> {
    if command.is_empty() {
        bail!("No command specified");
    }
    let entry = vault.read(path)?;
    let mut cmd = std::process::Command::new(&command[0]);
    cmd.args(&command[1..]);

    // Standard exports
    if let Some(pw) = &entry.fields.password {
        cmd.env("RTPV_PASSWORD", pw);
    }
    if let Some(u) = &entry.fields.username {
        cmd.env("RTPV_USERNAME", u);
        cmd.env("RTPV_USER", u);
    }
    if let Some(e) = &entry.fields.email {
        cmd.env("RTPV_EMAIL", e);
    }
    if let Some(h) = &entry.fields.host {
        cmd.env("RTPV_HOST", h);
    }
    if let Some(p) = &entry.fields.port {
        cmd.env("RTPV_PORT", p);
    }
    if let Some(d) = &entry.fields.database {
        cmd.env("RTPV_DATABASE", d);
    }
    if let Some(t) = &entry.fields.token {
        cmd.env("RTPV_TOKEN", t);
    }

    // Smart mappings
    let tool = command[0].split('/').last().unwrap_or(&command[0]);
    match tool {
        "psql" => {
            if let Some(pw) = &entry.fields.password {
                cmd.env("PGPASSWORD", pw);
            }
            if let Some(u) = &entry.fields.username {
                cmd.env("PGUSER", u);
            }
            if let Some(h) = &entry.fields.host {
                cmd.env("PGHOST", h);
            }
            if let Some(p) = &entry.fields.port {
                cmd.env("PGPORT", p);
            }
            if let Some(d) = &entry.fields.database {
                cmd.env("PGDATABASE", d);
            }
        }
        "mysql" | "mysqldump" => {
            if let Some(pw) = &entry.fields.password {
                cmd.env("MYSQL_PWD", pw);
            }
        }
        "aws" => {
            if let Some(tok) = &entry.fields.token {
                if let Some((k, s)) = tok.split_once(':') {
                    cmd.env("AWS_ACCESS_KEY_ID", k);
                    cmd.env("AWS_SECRET_ACCESS_KEY", s);
                }
            }
        }
        _ => {}
    }

    let status = cmd.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

// ── SSH ───────────────────────────────────────────────────────────────────────

pub fn cmd_ssh_add(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    name: &str,
    private_key_path: &str,
    public_key_path: Option<&str>,
) -> Result<()> {
    let path = format!("ssh/{}", name);
    if vault.exists(&path) {
        bail!("SSH key already stored at '{}'", path);
    }

    let pvt = std::fs::read_to_string(private_key_path)
        .map_err(|_| anyhow::anyhow!("Cannot read private key: {}", private_key_path))?;

    let mut entry = Entry::new(EntryKind::Ssh);
    entry.fields.pvt_key = Some(pvt.trim().to_string());

    // Try to find public key
    let pub_path = public_key_path
        .map(String::from)
        .unwrap_or_else(|| format!("{}.pub", private_key_path));
    if let Ok(pub_key) = std::fs::read_to_string(&pub_path) {
        entry.fields.pub_key = Some(pub_key.trim().to_string());
    }

    vault.write(&path, &entry)?;
    printer.ok(&format!("SSH key stored: {}", path));
    autosync(vault, cfg, &format!("Store SSH key {}", name));
    Ok(())
}

pub fn cmd_ssh_restore(vault: &Vault, printer: &Printer, name: &str) -> Result<()> {
    let path = if name.starts_with("ssh/") {
        name.to_string()
    } else {
        format!("ssh/{}", name)
    };
    let entry = vault.read(&path)?;

    let ssh_dir = dirs::home_dir().unwrap_or_default().join(".ssh");
    std::fs::create_dir_all(&ssh_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o700))?;
    }

    let key_name = name.split('/').last().unwrap_or(name);
    let pvt_path = ssh_dir.join(key_name);
    let pub_path = ssh_dir.join(format!("{}.pub", key_name));

    if let Some(pvt) = &entry.fields.pvt_key {
        std::fs::write(&pvt_path, pvt)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&pvt_path, std::fs::Permissions::from_mode(0o600))?;
        }
        printer.ok(&format!("Private key → {}", pvt_path.display()));
        let _ = std::process::Command::new("ssh-add")
            .arg(&pvt_path)
            .status();
    }
    if let Some(pub_key) = &entry.fields.pub_key {
        std::fs::write(&pub_path, format!("{}\n", pub_key))?;
        printer.ok(&format!("Public key  → {}", pub_path.display()));
    }
    Ok(())
}

pub fn cmd_ssh_list(vault: &Vault, printer: &Printer) -> Result<()> {
    let keys = vault.list_filtered("ssh/")?;
    if keys.is_empty() {
        printer.info("No SSH keys stored");
        return Ok(());
    }
    for k in &keys {
        let has_pvt = vault
            .read(k)
            .map(|e| e.fields.pvt_key.is_some())
            .unwrap_or(false);
        let has_pub = vault
            .read(k)
            .map(|e| e.fields.pub_key.is_some())
            .unwrap_or(false);
        println!(
            "  {:40}  pub:{}  pvt:{}",
            k,
            if has_pub { "✔" } else { "✖" },
            if has_pvt { "✔" } else { "✖" }
        );
    }
    Ok(())
}

pub fn cmd_ssh_generate(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    name: &str,
    comment: &str,
) -> Result<()> {
    let ssh_dir = dirs::home_dir().unwrap_or_default().join(".ssh");
    std::fs::create_dir_all(&ssh_dir)?;
    let key_name = format!("rtpv_{}", name.replace('/', "_"));
    let pvt_path = ssh_dir.join(&key_name);
    let pub_path = ssh_dir.join(format!("{}.pub", key_name));

    if pvt_path.exists() {
        bail!("Key file already exists: {}", pvt_path.display());
    }

    let comment_arg = if comment.is_empty() {
        name.to_string()
    } else {
        comment.to_string()
    };
    let status = std::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-a",
            "100",
            "-f",
            pvt_path.to_str().unwrap_or(""),
            "-C",
            &comment_arg,
            "-N",
            "",
        ])
        .status()?;
    if !status.success() {
        bail!("ssh-keygen failed");
    }

    cmd_ssh_add(
        vault,
        cfg,
        printer,
        name,
        pvt_path.to_str().unwrap_or(""),
        pub_path.to_str().map(|s| s),
    )?;

    let pub_key = std::fs::read_to_string(&pub_path)?;
    println!("\n  Public key:");
    println!("{}", pub_key.trim());
    Ok(())
}

// ── Recipients ───────────────────────────────────────────────────────────────

pub fn cmd_recipients_list(vault: &Vault, printer: &Printer) -> Result<()> {
    let rec_path = vault.recipients_path();
    let text = std::fs::read_to_string(&rec_path)?;
    for line in text.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            println!("  {}", line);
        }
    }
    Ok(())
}

pub fn cmd_recipients_add(vault: &Vault, printer: &Printer, pubkey: &str) -> Result<()> {
    vault.add_recipient(pubkey)?;
    printer.ok("Recipient added");
    printer.warn("Run 'rtpv recipients reencrypt' to re-encrypt vault with new recipient");
    Ok(())
}

pub fn cmd_recipients_remove(vault: &Vault, printer: &Printer, pubkey: &str) -> Result<()> {
    let rec_path = vault.recipients_path();
    let content = std::fs::read_to_string(&rec_path)?;
    let updated: String = content
        .lines()
        .filter(|l| l.trim() != pubkey)
        .map(|l| format!("{}\n", l))
        .collect();
    if updated == content {
        printer.warn("Recipient not found");
    } else {
        std::fs::write(&rec_path, updated)?;
        printer.ok("Recipient removed");
        printer.warn("Run 'rtpv recipients reencrypt' to apply");
    }
    Ok(())
}

pub fn cmd_recipients_reencrypt(vault: &Vault, printer: &Printer) -> Result<()> {
    printer.step("Re-encrypting vault...");
    let (ok, fail) = vault.reencrypt_all()?;
    printer.ok(&format!("Re-encrypted {} entries", ok));
    if fail > 0 {
        printer.warn(&format!("{} failed", fail));
    }
    Ok(())
}

// ── migrate from pass ────────────────────────────────────────────────────────

pub fn cmd_migrate(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    store_path: Option<&str>,
) -> Result<()> {
    let src = store_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".password-store"));

    if !src.exists() {
        bail!("Pass store not found: {}", src.display());
    }

    printer.banner(&format!("Migrating from {}", src.display()));
    let mut ok = 0usize;
    let mut fail = 0usize;

    for entry in walkdir::WalkDir::new(&src)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("gpg") {
            continue;
        }

        let rel = match path.strip_prefix(&src) {
            Ok(r) => r.with_extension("").to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        // Skip .rtpv meta files
        if rel.starts_with(".rtpv") {
            continue;
        }

        // Decrypt with GPG subprocess
        let out = std::process::Command::new("gpg")
            .args([
                "--quiet",
                "--batch",
                "--yes",
                "--decrypt",
                &path.to_string_lossy(),
            ])
            .output();

        match out {
            Ok(o) if o.status.success() => {
                let plaintext = String::from_utf8_lossy(&o.stdout).to_string();
                let rtpv_entry = Entry::from_pass_plaintext(&plaintext);

                match vault.write(&rel, &rtpv_entry) {
                    Ok(_) => {
                        printer.ok(&format!("  {}", rel));
                        ok += 1;
                    }
                    Err(e) => {
                        printer.warn(&format!("  skip {}: {}", rel, e));
                        fail += 1;
                    }
                }
            }
            _ => {
                printer.warn(&format!("  skip {}: GPG decrypt failed", rel));
                fail += 1;
            }
        }
    }

    println!();
    printer.ok(&format!("Migrated: {}  Skipped: {}", ok, fail));
    if ok > 0 {
        autosync(vault, cfg, "Migrate from pass store");
    }
    Ok(())
}

// ── import ───────────────────────────────────────────────────────────────────

pub fn cmd_import_bitwarden(
    vault: &Vault,
    cfg: &Config,
    printer: &Printer,
    path: &str,
) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let mut ok = 0usize;
    let mut skip = 0usize;

    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
        for item in items {
            if item.get("type").and_then(|v| v.as_u64()) != Some(1) {
                skip += 1;
                continue;
            }
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                skip += 1;
                continue;
            }
            let login = item.get("login");
            let pw = login
                .and_then(|l| l.get("password"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if pw.is_empty() {
                skip += 1;
                continue;
            }

            let entry_name = sanitize_path(name);
            let entry_name = unique_name(vault, &entry_name);

            let mut entry = Entry::new(EntryKind::Login);
            entry.fields.password = Some(pw.to_string());
            if let Some(u) = login
                .and_then(|l| l.get("username"))
                .and_then(|v| v.as_str())
            {
                if !u.is_empty() {
                    entry.fields.username = Some(u.to_string());
                }
            }
            if let Some(url) = login
                .and_then(|l| l.get("uris"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|u| u.get("uri"))
                .and_then(|v| v.as_str())
            {
                entry.fields.url = Some(url.to_string());
            }
            if let Some(n) = item.get("notes").and_then(|v| v.as_str()) {
                if !n.is_empty() {
                    entry.fields.notes = Some(n.to_string());
                }
            }

            match vault.write(&entry_name, &entry) {
                Ok(_) => {
                    printer.ok(&format!("  {}", entry_name));
                    ok += 1;
                }
                Err(e) => {
                    printer.warn(&format!("  skip {}: {}", entry_name, e));
                    skip += 1;
                }
            }
        }
    }

    println!();
    printer.ok(&format!("Imported: {}  Skipped: {}", ok, skip));
    if ok > 0 {
        autosync(vault, cfg, "Import from Bitwarden");
    }
    Ok(())
}

pub fn cmd_import_csv(vault: &Vault, cfg: &Config, printer: &Printer, path: &str) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut ok = 0;
    let mut skip = 0;
    let mut lines = text.lines();
    let _header = lines.next(); // skip header

    for line in lines {
        let cols: Vec<&str> = line.split(',').map(|c| c.trim_matches('"')).collect();
        if cols.len() < 2 {
            skip += 1;
            continue;
        }
        let name = cols[0];
        let pw = cols.get(1).copied().unwrap_or("");
        if name.is_empty() || pw.is_empty() {
            skip += 1;
            continue;
        }

        let entry_name = unique_name(vault, &sanitize_path(name));
        let mut entry = Entry::new(EntryKind::Login);
        entry.fields.password = Some(pw.to_string());
        if let Some(u) = cols.get(2) {
            if !u.is_empty() {
                entry.fields.username = Some(u.to_string());
            }
        }
        if let Some(e) = cols.get(3) {
            if !e.is_empty() {
                entry.fields.email = Some(e.to_string());
            }
        }
        if let Some(u) = cols.get(4) {
            if !u.is_empty() {
                entry.fields.url = Some(u.to_string());
            }
        }

        match vault.write(&entry_name, &entry) {
            Ok(_) => {
                printer.ok(&format!("  {}", entry_name));
                ok += 1;
            }
            Err(e) => {
                printer.warn(&format!("  skip {}: {}", entry_name, e));
                skip += 1;
            }
        }
    }

    printer.ok(&format!("Imported: {}  Skipped: {}", ok, skip));
    if ok > 0 {
        autosync(vault, cfg, "Import from CSV");
    }
    Ok(())
}

// ── export ───────────────────────────────────────────────────────────────────

pub fn cmd_export(vault: &Vault, printer: &Printer, output: &str) -> Result<()> {
    printer.warn("Exporting decrypted passwords — keep the output secure");
    if !ui::confirm("Continue?")? {
        printer.dim("Aborted");
        return Ok(());
    }

    let mut items = Vec::new();
    for path in vault.list()? {
        if let Ok(entry) = vault.read(&path) {
            let pw = entry.fields.password.as_deref().unwrap_or("");
            let u = entry.fields.username.as_deref().unwrap_or("");
            let url = entry.fields.url.as_deref().unwrap_or("");
            let notes = entry.fields.notes.as_deref().unwrap_or("");
            items.push(serde_json::json!({
                "type": 1,
                "name": path,
                "notes": notes,
                "login": {
                    "username": u,
                    "password": pw,
                    "uris": [{"uri": url}]
                }
            }));
        }
    }

    let json = serde_json::to_string_pretty(&serde_json::json!({
        "encrypted": false,
        "items": items
    }))?;
    std::fs::write(output, json)?;
    printer.ok(&format!("Exported {} entries to {}", items.len(), output));
    Ok(())
}

// ── doctor ───────────────────────────────────────────────────────────────────

pub fn cmd_doctor(vault: &Vault, cfg: &Config, printer: &Printer) {
    printer.banner("Doctor");

    let check = |label: &str, ok: bool, detail: &str| {
        if ok {
            printer.ok(&format!("{:<30} {}", label, detail));
        } else {
            printer.warn(&format!("{:<30} {}", label, detail));
        }
    };

    check(
        "vault directory",
        cfg.vault_dir.is_dir(),
        &cfg.vault_dir.display().to_string(),
    );
    check(
        "identity.age",
        vault.identity_path().exists(),
        &vault.identity_path().display().to_string(),
    );
    check(
        "recipients.txt",
        vault.recipients_path().exists(),
        &vault.recipients_path().display().to_string(),
    );
    check(
        "git repository",
        GitSync::is_repo(&cfg.vault_dir),
        &cfg.vault_dir.display().to_string(),
    );
    check("clipboard", clipboard::available(), "arboard");
    check(
        "xdotool/ydotool",
        which("xdotool") || which("ydotool"),
        "for autofill",
    );
    check("gpg", which("gpg"), "for migration from pass");
    check(
        "ssh-agent",
        std::env::var("SSH_AUTH_SOCK").is_ok(),
        "for git SSH sync",
    );
}

// ── recent ────────────────────────────────────────────────────────────────────

pub fn cmd_recent(cfg: &Config, printer: &Printer, count: usize) -> Result<()> {
    let mut entries: Vec<(String, std::time::SystemTime)> = Vec::new();

    for e in walkdir::WalkDir::new(&cfg.vault_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        if path.components().any(|c| c.as_os_str() == ".rtpv") {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("age") {
            continue;
        }
        if let Ok(meta) = path.metadata() {
            if let Ok(mt) = meta.modified() {
                if let Ok(rel) = path.strip_prefix(&cfg.vault_dir) {
                    let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
                    entries.push((name, mt));
                }
            }
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(count);

    for (name, mt) in &entries {
        let dt: chrono::DateTime<chrono::Local> = (*mt).into();
        println!("  {:40} {}", name, dt.format("%Y-%m-%d %H:%M"));
    }
    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn autosync(vault: &Vault, cfg: &Config, message: &str) {
    if !cfg.autosync {
        return;
    }
    if let Ok(git) = GitSync::open(&cfg.vault_dir) {
        let _ = git.commit(message);
        let _ = git.push("origin");
    }
}

pub fn git_commit(cfg: &Config, message: &str) {
    if let Ok(git) = GitSync::open(&cfg.vault_dir) {
        let _ = git.commit(message);
    }
}

fn sanitize_path(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '/' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase()
        .trim_matches('-')
        .to_string()
}

fn unique_name(vault: &Vault, base: &str) -> String {
    if !vault.exists(base) {
        return base.to_string();
    }
    let mut i = 2;
    loop {
        let candidate = format!("{}-{}", base, i);
        if !vault.exists(&candidate) {
            return candidate;
        }
        i += 1;
    }
}
