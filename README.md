# rtpv

> Rust rewrite of [passx](https://github.com/tonycth7/passx) — native binary, age encryption, built-in TUI.

---

rtpv is a power-user password manager built in Rust. It shares the same philosophy and entry format as passx, but runs as a single native binary with no bash dependency, uses **age encryption by default** (no GPG needed), and ships a built-in **ratatui TUI** instead of relying on fzf or rofi.

---

## install

```sh
cargo install --git https://github.com/tonycth7/rtpv rtpv
# or install all three binaries:
cargo install --git https://github.com/tonycth7/rtpv rtpv rtpv-tui rtpv-menu
```

or clone and build:

```sh
git clone https://github.com/tonycth7/rtpv
cd rtpv
cargo build --release
# binaries in target/release/: rtpv, rtpv-tui, rtpv-menu
```

---

## vs passx

| feature | passx (bash) | rtpv (Rust) |
|---|---|---|
| encryption | GPG (subprocess) | age native (rage crate) |
| GPG compat | ✔ primary | ✔ `--backend gpg` |
| TUI picker | fzf (external) | ratatui (built-in) |
| launcher | rofi/wofi/dmenu | ratatui popup (built-in) |
| install | bash + deps | single binary |
| speed | fast | faster |
| OTP | oathtool (ext) | totp-rs (built-in) |
| HIBP check | curl (ext) | reqwest (built-in) |

---

## quick start

```sh
# initialize with age encryption (recommended)
rtpv init

# migrate an existing pass / passx store
rtpv --backend gpg migrate-to-age

# add entries
rtpv add github tony@mail.com
rtpv add github --password          # prompt for own password
rtpv add github --words 5           # 5-word passphrase
rtpv add github --pin 6             # numeric PIN

# access
rtpv show github                    # display entry
rtpv copy github                    # copy password  (auto-clears in 20s)
rtpv copy github -u                 # copy username
rtpv show github:email              # print email field

# OTP
rtpv otp copy github                # copy live TOTP code
rtpv otp show github                # show code + countdown
rtpv otp list                       # all OTP entries with live codes
rtpv otp import github "otpauth://totp/..."

# generate
rtpv gen                            # 32-char password → clipboard
rtpv gen --words 5                  # passphrase
rtpv gen --pin 6                    # PIN
rtpv gen --pronounceable            # speakable

# manage
rtpv edit github                    # open in $EDITOR
rtpv rotate github                  # generate new password
rtpv set-field github url https://github.com
rtpv rename github work/github
rtpv clone github github-personal
rtpv delete github

# security
rtpv audit                          # weak / duplicate / aged
rtpv audit --fix                    # auto-rotate weak entries
rtpv audit --hibp                   # check all against HIBP
rtpv hibp github                    # check one entry
rtpv strength github
rtpv entropy github

# templates
rtpv template                       # interactive picker
rtpv t --kind web-login             # shortcut
rtpv t --kind server --ask          # own password

# secrets
rtpv note add secrets/diary
rtpv note show secrets/diary
rtpv env aws/prod                   # export as shell env vars
rtpv dotenv db/prod > .env
rtpv run db/prod psql               # inject creds into subprocess

# SSH keys
rtpv ssh add mykey ~/.ssh/id_ed25519
rtpv ssh set mykey                  # restore to ~/.ssh + ssh-agent
rtpv ssh list

# import
rtpv import bitwarden export.json
rtpv import firefox logins.csv
rtpv import chrome passwords.csv
rtpv import keepass passwords.csv

# sync
rtpv sync                           # git pull + push
rtpv log
rtpv diff github

# maintenance
rtpv doctor
rtpv stats
rtpv gc
rtpv lint
rtpv gen-conf                       # write ~/.config/rtpv/rtpv.toml
```

---

## TUI picker — `rtpv-tui`

Built-in ratatui picker — no fzf needed.

```sh
rtpv-tui
```

| key | action |
|---|---|
| type | filter entries |
| `j` / `k` or `↑` / `↓` | navigate |
| `Enter` | open action menu |
| `y` | copy password |
| `u` | copy username |
| `o` | copy OTP code |
| `q` / `Esc` | quit |

---

## menu launcher — `rtpv-menu`

Ratatui popup launcher — bind to a key in your WM, no rofi/wofi/dmenu needed.

```sh
# sway / i3
bindsym $mod+p exec rtpv-menu
bindsym $mod+shift+p exec rtpv-menu otp
```

```sh
rtpv-menu              # main picker — Enter copies password, Tab opens actions
rtpv-menu otp          # OTP picker — Enter copies code
rtpv-menu gen          # generate password → clipboard
```

| key | action |
|---|---|
| type | filter |
| `↑` / `↓` or `j` / `k` | navigate |
| `Enter` | copy password |
| `Tab` | open action menu |
| `u` | copy username |
| `e` | copy email |
| `o` | copy OTP |
| `Esc` / `q` | quit |

---

## age encryption

rtpv uses **age** (via the [`rage`](https://github.com/str4d/rage) crate) for encryption by default:

- pure Rust — no external `gpg` binary required
- X25519 key exchange — faster and simpler than RSA
- identity stored at `~/.config/rtpv/identity.age` (mode 0600)
- recipients at `~/.config/rtpv/recipients.txt`

**GPG compatibility** is preserved for migrating existing pass stores:

```sh
# read/write .gpg files with your existing GPG key
rtpv --backend gpg show github

# migrate the whole store to age
rtpv migrate-to-age
```

---

## entry format

Identical to pass/passx — fully compatible:

```
mypassword
username: tony
email: tony@mail.com
url: https://github.com
notes: personal account
otp: otpauth://totp/GitHub:tony?secret=JBSWY3DPEHPK3PXP
token: ghp_xxxxxxxxxxxx
```

First line is always the password. Everything else is `key: value`. Files are stored as `.age` (or `.gpg` in compat mode).

---

## configuration

```sh
rtpv gen-conf     # write ~/.config/rtpv/rtpv.toml
```

Key options:

```toml
crypto_backend = "age"       # age | gpg
autosync       = false       # git push/pull on every change
clip_timeout   = 20          # seconds before clipboard clears
max_age_days   = 180         # audit threshold
theme          = "catppuccin" # catppuccin | nord | gruvbox | dracula | solarized
gen_length     = 32
gen_chars      = "A-Za-z0-9@#%+=_"
```

All options available as `RTPV_*` environment variables.

---

## hooks

Drop executable scripts in `~/.config/rtpv/hooks/`:

```
post-add.sh
post-edit.sh
post-rotate.sh
post-sync.sh
```

---

## workspace layout

```
rtpv/
├── Cargo.toml          workspace
├── config/
│   └── rtpv.sample.toml
├── rtpv-core/          shared library (crypto, store, OTP, audit, all commands)
├── rtpv-cli/           CLI binary  →  rtpv
├── rtpv-tui/           TUI binary  →  rtpv-tui
└── rtpv-menu/          menu binary →  rtpv-menu
```

---

## dependencies

| crate | used for |
|---|---|
| `rage` | age encryption (native) |
| `clap` | CLI argument parsing |
| `ratatui` + `crossterm` | TUI and menu |
| `totp-rs` | TOTP/HOTP OTP codes |
| `git2` | git sync (native, no subprocess) |
| `arboard` | clipboard (Wayland, X11, macOS, Windows) |
| `reqwest` | HIBP breach check |
| `zxcvbn` | password strength estimation |
| `fuzzy-matcher` | built-in fuzzy search |
| `serde` + `toml` | config file |

No external tools required for core functionality. Optional:
- `xdotool` or `ydotool` — autofill (`rtpv fill`)
- `notify-send` — desktop notifications

---

## status

**v0.1.0** — active development, passx feature-complete.

Roadmap:
- `[ ]` shell completions (bash, zsh, fish)
- `[ ]` QR code display for OTP setup
- `[ ]` `rtpv-menu` OTP countdown progress bar
- `[ ]` AUR and Homebrew packages
- `[ ]` Windows clipboard support testing

---

## license

MIT
