# Wi-Fi Analyzer

Read-only Wi-Fi AP scanner for FlipCTL on reComputer RK3576, ARM64 Armbian
Debian Bookworm. Based on the official `apps/hello` at FlipCTL revision
`8aeb387dc4d13fcef9056267b2a3dd7409099834`. Uses `flipctl-app`, its Slint pages,
256 x 144 canvas, bundled system fonts, and grayscale theme tokens. No FlipCTL
core or other app changes are required.

## Install and start

NetworkManager must be running with at least one managed, available Wi-Fi
interface and the Wi-Fi radio already enabled. `nmcli` must be in PATH. You do
not need to connect to a Wi-Fi network. The app never enables radios, connects,
disconnects, changes profiles, or changes network configuration. It never uses
monitor mode or password attacks. Scans are normal NetworkManager AP scans.

```sh
mkdir -p "$HOME/Apps/Network"
install -m 755 wifi-analyzer-aarch64.fap.AppImage "$HOME/Apps/Network/"
```

Open the existing FlipCTL WebUI, enable **Show controls**, then choose
**Menu -> Apps -> Network -> Wi-Fi Analyzer** and press **Enter**. The app runs
as a Wayland client hosted by FlipCTL. Keep the existing FlipCTL launch settings
(`device,slint,remote,wayland`; on this board `WLR_RENDERER=pixman`).

## Controls

| Key / WebUI control | Action |
| --- | --- |
| Up / Down | Select an AP; scroll details; switch 2.4GHz/5GHz in channel view |
| Left / Right | Jump four APs; return from details; page through channels |
| Enter | Open selected AP details; return to the list from details or channels |
| View / X | Open Channel Analysis; toggle AP count / strongest signal percentage chart |
| Run / B | Refresh manually; busy/repeated presses are ignored; allow 2 seconds after releasing |
| Close / Escape | Exit the app |
| Back | FlipCTL backgrounds the app (the SDK reserves this key) |

The list shows SSID, real `nmcli SIGNAL` **0-100%**, channel, selected BSSID,
total AP count and per-band counts. `*` marks an AP reported in use by nmcli.
`<hidden>` is an empty SSID; a literal `--` SSID is preserved. Details show full
SSID, BSSID, signal percentage, channel, frequency in MHz, security and connected
status. Scroll down for all fields and a full error message, when applicable.
Long SSIDs truncate only in the list; details wrap them. Glyph coverage follows
the official system fonts.

AP identity is BSSID, not SSID. The strongest duplicate record for a BSSID wins;
the list sorts by signal descending, then BSSID. Band classification uses MHz,
not channel numbers. Other bands count toward total APs but are not graphed as
2.4GHz or 5GHz. Channel bars show **observed AP counts**, or the strongest AP's
signal percentage on each channel; they are not airtime utilization or measured
interference. The UI does not display dBm because nmcli SIGNAL is not RSSI.

A dedicated worker scans immediately, then waits 25 seconds after each scan
finishes. The 50ms UI timer only drains completed messages and updates scan age;
it never invokes nmcli. Repainting the canvas at 20Hz keeps the pinned FlipCTL
host's 120ms damage-waiting capture active on this RK3576; a slower cadence
reproduced a host-side EAGAIN freeze. No global settings or core code are changed.
Manual refresh requests are bounded and coalesced. Run key repeats are suppressed
until two seconds of quiet, including repeats received while a scan is busy;
a held/reordered remote key cannot start back-to-back scans after completion. Every
nmcli process has a timeout and stdout/stderr are drained concurrently.

On errors, **ERROR** remains visible and the last successful snapshot is kept.
An empty successful scan means zero APs and clears old results; an unavailable
interface, disabled radio or malformed record is an error, not a zero-AP scan.
If the radio becomes unavailable, fix it outside the app and press Run. Errors
also go to stderr. No production screen loads example or simulated AP data.

## Build in the existing FlipCTL checkout

Place this directory at `apps/wifi-analyzer`, keeping the SDK at
`../../crates/flipctl-app`. No workspace membership change is necessary.

```sh
cd "$HOME/flipper-test/flipctl-slint"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo test --locked --release --manifest-path apps/wifi-analyzer/Cargo.toml
cargo build --locked --release --manifest-path apps/wifi-analyzer/Cargo.toml
sh apps/wifi-analyzer/build-appimage.sh
```

The packer and runtime must already be cached at `target/appimage/tools/` and
match the hashes in `tools/appimage/tools.lock`. The script uses the official
stager and packer, writes `target/appimage/wifi-analyzer-aarch64.fap.AppImage`,
and does not install it or restart FlipCTL. Native Bookworm builds avoid a newer
glibc dependency from a different distribution. Third-party license notices are
copied from Hello World because the dependency graph is unchanged (only the
root package is renamed). The app's own source is MIT; the linked binary is
GPL-3.0-only, as documented by the upstream SDK.

## Verification

Unit tests contain test-only fixtures. They cover duplicate/hidden SSIDs,
escaped BSSID colons, escaped SSID colons/backslashes, empty and malformed
results, signal endpoints, BSSID deduplication, sorting, per-band/channel counts,
retention after an error, and selection identity after reordering.

Run the explicit hardware integration test only with a usable real interface:

```sh
cd "$HOME/flipper-test/flipctl-slint"
export CARGO_TARGET_DIR="$PWD/target"
cargo test --locked --release --manifest-path apps/wifi-analyzer/Cargo.toml \
  real_scan_and_render -- --ignored --test-threads=1
```

It scans actual APs and saves five token-palette-checked 256 x 144 pages under
`apps/wifi-analyzer/target/wifi-verification/` (relative to Cargo's test working
directory). It fails if no usable interface exists; it never substitutes data.

For an exact comparison during a WebUI run, set `WIFI_ANALYZER_AUDIT_DIR` in the
environment inherited by the app (e.g. the FlipCTL process environment). Each
successful scan writes its unmodified nmcli output and a TSV of the exact UI
snapshot into a timestamped directory, plus a `latest` pointer. This is opt-in
and may include nearby network identifiers. The app only writes these files;
it never reads them as scan input. Compare with:

```sh
python3 apps/wifi-analyzer/verify-snapshot.py /path/to/audit/scan-TIMESTAMP
```

For normal use, leave the audit variable unset. See the delivered validation
report for the checks actually completed on the board.
