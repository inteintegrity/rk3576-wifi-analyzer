# Wi-Fi Analyzer for reComputer RK3576

A standalone ARM64 AppImage for nearby Wi-Fi access-point scanning and visualization
inside the existing **FlipCTL WebUI**. Built with Rust, `flipctl-app` and Slint on
reComputer RK3576 running Armbian Debian Bookworm.

## Features

- Network list, full AP details, and 2.4GHz / 5GHz channel charts on a 256 x 144 canvas.
- Real `nmcli SIGNAL` percentages from 0 to 100; no estimated RSSI/dBm values.
- BSSID-based identity, including multiple APs with the same SSID and hidden SSIDs.
- Signal sorting, AP totals per band, channel AP counts and strongest-signal charts.
- Background scans with a 25-second wait after completion, plus manual refresh.
- Explicit errors and retention of the last successful scan; no simulated runtime data.
- Read-only operation: no connections, disconnections, radio toggles, network-profile
  changes, monitor mode or password attacks.
- Uses the existing FlipCTL SDK and system fonts without changing FlipCTL core.

## Install and launch

Download `wifi-analyzer-aarch64.fap.AppImage` and `SHA256SUMS` from
[Releases](https://github.com/inteintegrity/rk3576-wifi-analyzer/releases).
Check the AppImage's SHA256 against its entry in `SHA256SUMS`, then on the board:

```sh
mkdir -p "$HOME/Apps/Network"
install -m 755 wifi-analyzer-aarch64.fap.AppImage "$HOME/Apps/Network/"
```

NetworkManager must be active, with `nmcli` in PATH and a managed, available Wi-Fi
interface. The radio must already be enabled; connecting to a hotspot is not required.
The app detects a usable interface automatically and prefers a connected managed radio.

Open your existing FlipCTL WebUI, enable **Show controls**, then choose
**Menu → Apps → Network → Wi-Fi Analyzer → Enter**.
FlipCTL needs `device,slint,remote,wayland`. The tested RK3576 host used
`WLR_RENDERER=pixman`. Do not run upstream hardware deployment/service scripts
just to install this app.

## Controls

| Key | Action |
| --- | --- |
| Up / Down | Select APs, scroll details, switch the chart's frequency band |
| Left / Right | Jump four APs, return from details, page through channels |
| Enter | Open details or return to the list |
| View / X | Open channel charts; toggle AP counts / strongest signal percentage |
| Run / B | Refresh manually; busy/repeated input is ignored; allow two seconds after release |
| Close / Z | Exit the app |
| Back | FlipCTL backgrounds the app |

Charts count observed APs; they do not measure airtime utilization or interference.
An empty successful scan clears old APs. A scanner/interface error keeps the last
successful snapshot and displays **ERROR**. The app does not change system settings
to recover an unavailable radio.

## Source layout and build

The application is kept in [`apps/wifi-analyzer`](apps/wifi-analyzer), in its exact
tested FlipCTL app layout. The SDK is an upstream dependency, not a modified fork.
The release's `wifi-analyzer-source.tar.gz` includes the full pinned upstream SDK
and application source in a ready-to-build tree.

To build from this repository, first prepare the pinned official SDK on the ARM64 board:

```sh
git clone https://github.com/inteintegrity/rk3576-wifi-analyzer.git
git clone --no-checkout https://github.com/flipperdevices/flipctl-slint.git flipctl-wifi-build
git -C flipctl-wifi-build checkout --detach 8aeb387dc4d13fcef9056267b2a3dd7409099834
cp -a rk3576-wifi-analyzer/apps/wifi-analyzer flipctl-wifi-build/apps/
cd flipctl-wifi-build

export CARGO_TARGET_DIR="$PWD/target"
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo test --locked --release --manifest-path apps/wifi-analyzer/Cargo.toml
cargo build --locked --release --manifest-path apps/wifi-analyzer/Cargo.toml
```

Slint 1.17 requires Rust 1.92 or newer; the verified toolchain was Rust/Cargo 1.96.0.
Building natively on Bookworm avoids a newer distribution's glibc requirement.

For packaging, cache the two ARM64 tools specified in the upstream `tools.lock`.
This download block validates their pinned hashes:

```sh
(
  set -eu
  mkdir -p target/appimage/tools
  while read -r name url checksum; do
    case "$name" in
      appimagetool-aarch64|runtime-aarch64)
        tool="target/appimage/tools/$name"
        if [ ! -f "$tool" ] || ! printf '%s  %s\n' "$checksum" "$tool" | sha256sum -c --status; then
          curl -fL --retry 3 --connect-timeout 20 --max-time 180 -o "$tool" "$url"
        fi
        printf '%s  %s\n' "$checksum" "$tool" | sha256sum -c -
        chmod +x "$tool"
        ;;
    esac
  done < tools/appimage/tools.lock
)
sh apps/wifi-analyzer/build-appimage.sh
```

Output: `target/appimage/wifi-analyzer-aarch64.fap.AppImage`.
Packaging does not install the app or restart FlipCTL. Move an existing output aside
before rebuilding. See the [application README](apps/wifi-analyzer/README.md) for
hardware rendering tests and opt-in local scan auditing.

## Verified status

Tested on a real reComputer RK3576 with Armbian Debian Bookworm, NetworkManager 1.42.4,
and a managed `aicwf_sdio` Wi-Fi interface:

- 16 unit tests and one explicit real-scan/render integration test passed.
- Menu launch, AP selection/details, scrolling, charts, paging, automatic/manual refresh,
  and held-Run protection were exercised through the actual FlipCTL WebUI.
- The audit snapshot's AP counts, signal values, channels and chart statistics matched
  its original nmcli output; no nearby-network identifiers or raw scan captures are published here.
- Native binary requires up to GLIBC_2.35; the tested board provides glibc 2.36.

Connected-hotspot positive cases, real radio/driver fault injection and multiple Wi-Fi
adapters have not been tested on hardware. `cargo fmt` could not run because the
board lacked `cargo-fmt`. See [validation details](docs/VALIDATION.md).

## Licensing and upstream

Application source is MIT, derived from the official
[Hello World template](https://github.com/flipperdevices/flipctl-slint/tree/8aeb387dc4d13fcef9056267b2a3dd7409099834/apps/hello).
The linked binary is **GPL-3.0-only** through Slint. Font licenses and dependency
notices are preserved in the AppImage and complete source archive.
See [NOTICE](NOTICE.md) and the application's
[third-party notices](apps/wifi-analyzer/THIRD-PARTY-LICENSES.md).
