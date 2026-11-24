# Upstream and license notices

This application was derived from `apps/hello` in the official Flipper Devices
FlipCTL Slint repository at revision `8aeb387dc4d13fcef9056267b2a3dd7409099834`:

https://github.com/flipperdevices/flipctl-slint

The application uses the unmodified upstream `flipctl-app` SDK, Slint components,
256 x 144 theme and bundled fonts. The application source is MIT licensed, and
the original Flipper Devices copyright and MIT terms are retained in `LICENSE`.

As explained by upstream, binaries linking this SDK and Slint are distributed
under GPL-3.0-only. The fonts retain their own licenses: Busy9px is MIT,
HaxrCorp 4090 is CC-BY-SA-3.0, and Born2bSportyV2 is Unlicense.

`apps/wifi-analyzer/THIRD-PARTY-LICENSES.md` is copied from the official Hello
World app because the dependency graph is unchanged except for the root app's
name. The official staging tool copies the dependency, project and font notices
into the AppImage. The release also supplies the full pinned SDK and application
source as `wifi-analyzer-source.tar.gz`.

This project is an independent reComputer RK3576 application and is not an
official Flipper Devices product.
