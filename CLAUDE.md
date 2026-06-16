# CLAUDE.md

Web App Hub is a Rust desktop app (GTK4 + libadwaita) for managing web apps with isolated browser profiles.

Workspace members:
- `workspaces/app` — binary `web-app-hub` (the GUI)
- `workspaces/common` — shared library code
- `workspaces/tools` — release tooling (`git-cliff`-based changelog/release binary)

## Build

System dependencies (Fedora):

```sh
sudo dnf install -y gtk4-devel libadwaita-devel
```

Then:

```sh
cargo build --release
```

## Run

```sh
cargo run -p app --release
```

or run the built binary directly: `target/release/web-app-hub`

## Build script side effects

`workspaces/app/build.rs` runs on every build and modifies the user's home directory:

- Creates `dev-config`, `dev-data`, `dev-cache` symlinks in the repo root pointing to `~/.config/web-app-hub`, `~/.local/share/web-app-hub`, `~/.cache/web-app-hub`.
- Installs/refreshes the app's desktop entry (`org.pvermeer.WebAppHub.desktop`) and icon into `~/.local/share/applications` and `~/.local/share/icons/hicolor/256x256/apps/` — same app ID as a Flatpak/Flathub install, so both can show up in the app launcher.

## Formatting

```sh
cargo fmt
```

Edition 2024, see `rustfmt.toml`.

## Flatpak build

```sh
./flatpak/build.sh
```

Requires `flatpak-builder` plus the GNOME 50 SDK/Platform and the Rust SDK extension. Installs as `org.pvermeer.WebAppHub`.

## Local RPM build (Fedora)

```sh
./packaging/rpm/build-rpm.sh
```

Requires `rpm-build` and `rpmdevtools` (`sudo dnf install -y rpm-build rpmdevtools`). The script runs `cargo build --release`, stages the binary plus the desktop entry, icon and metainfo from `assets/desktop/`, builds the RPM under `target/rpmbuild/`, and copies the result to `target/web-app-hub-<version>-1.<dist>.x86_64.rpm`.

The spec (`packaging/rpm/web-app-hub.spec`) packages the pre-built release binary (no compile inside `rpmbuild`, `debug_package` disabled) — this is fast and avoids running `build.rs`'s home-directory side effects under the rpm sandbox. The version is read from `Cargo.toml`. Runtime deps (`gtk4`, `libadwaita`, …) are auto-detected from the ELF; unlike the Flatpak it links against system GTK4 and runs unsandboxed.

Install with `sudo dnf install ./target/web-app-hub-<version>-1.<dist>.x86_64.rpm`, and uninstall with `sudo dnf remove web-app-hub`.
