# RK3576 hardware validation

Validated on 2026-11-24 on a real reComputer RK3576 with Armbian Debian Bookworm
(aarch64), glibc 2.36, Rust/Cargo 1.96.0, Slint 1.17.1 and NetworkManager 1.42.4.
The board's `aicwf_sdio` radio was managed by NetworkManager. `iw dev` confirmed
managed mode. Wireless was enabled with the device owner's approval, and no
hotspot connection/disconnection or network-profile changes were performed.

## Checks actually completed

All Cargo commands ran in the pinned FlipCTL checkout, with its shared `target`
directory, release LTO disabled and 16 codegen units.

| Check | Result |
| --- | --- |
| `cargo build --locked --offline --release --manifest-path apps/wifi-analyzer/Cargo.toml` | Passed; native ARM64 ELF, highest glibc symbol GLIBC_2.35 |
| `cargo test --locked --offline --release --manifest-path apps/wifi-analyzer/Cargo.toml -- --test-threads=1` | 16 unit tests passed; hardware test intentionally ignored here |
| Same Cargo test command selecting `real_scan_and_render -- --ignored --test-threads=1` | One real-scan/render integration test passed |
| `sh -n apps/wifi-analyzer/build-appimage.sh` | Passed |
| Official AppImage staging and ARM64 packaging | Passed; packer/runtime hashes matched upstream `tools.lock` |
| `verify-snapshot.py` against the same scan's raw nmcli output and UI snapshot | Passed on the board and again after copying the snapshot to the development host |
| FlipCTL binary and pre-existing WebUI checksums before/after | Unchanged |
| `cargo fmt` | Not completed: `cargo-fmt` was not installed |

The tests cover escaped colons/backslashes, duplicate/hidden SSIDs, BSSID identity,
signal endpoints, empty/malformed output, sorting, band/channel statistics,
retention after errors, selection identity and held-Run suppression.
All five rendered pages were 256 x 144 and used only the SDK grayscale palette.

## WebUI and data verification

The installed AppImage was launched from **Apps → Network → Wi-Fi Analyzer**.
The running process binary matched the final build's SHA256. No Slint debug
environment variables were present in the final app's process.

Verified AP selection, Enter/details, detail scrolling, frequency-band switching,
AP-count/signal chart switching, channel paging through page 2/2, manual refresh,
automatic refresh, held-Run suppression and app exit.

A preserved audit snapshot contained 42 unique BSSIDs: 15 in 2.4GHz and 27 in 5GHz.
All BSSIDs, signal percentages, channels, frequencies, connection flags, band
counts, sort order and channel-chart data matched the corresponding unmodified
nmcli output. Nearby SSIDs, BSSIDs, raw captures, device login information and
development-host paths are deliberately not included in this repository.

Recorded completion intervals included 26, 27, 28, 10 and 27 seconds; the 10-second
interval was a manual refresh. Holding Run for ten seconds did not cause repeated
scans after the first completion. At the final evidence collection the final
instance had nine successful scans with no capture-stopped errors.

## Application-only compatibility fixes

A 250ms redraw interval reproduced an EAGAIN capture freeze in the pinned host's
120ms damage-waiting capture path. Repainting the app's canvas every 50ms kept
capture active. This UI clock never runs nmcli; scanning stays on its worker.

Held/repeated Run input could previously request another scan after completion.
The app now requires two seconds of quiet between distinct Run gestures, updating
the guard on repeats even while busy. A regression test simulates a ten-second hold.
Neither fix changes FlipCTL core, its WebUI or any other application.

## Not verified on hardware

- A connected AP with `Connected=yes`; the tested interface remained disconnected.
  The positive marker parser is covered by unit tests.
- Real radio disablement, network disconnection or driver fault injection.
  Error/last-success retention is covered by unit tests.
- Multiple Wi-Fi adapters.

Source and binary checksums are in the repository and release `SHA256SUMS` file.
