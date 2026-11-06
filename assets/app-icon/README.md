# Spider PDF Manager app icon

<img src="spider-pdf-manager-small.svg" alt="Spider PDF Manager app icon: a red spider on a black shield emblem" width="128">

**Emblem:** a 3D red spider on a black shield/hexagon emblem, generated from the project owner's photo.

**Style:** a rendered 3D emblem with a transparent background — no tile, no frame, no roundel. The emblem floats on transparency, so it works on light and dark surfaces.

**Palette:** the render is shaded, but the two anchor colours are:

| Colour | Hex | Used for |
|---|---|---|
| Spider red (app colour) | `#b00102` | the spider; also the app accent (`UPDATE_APP` in `crates/ui-egui/src/lib.rs`) |
| Shield black | `#171717` | the shield emblem |

**Layout:** the master is a 1406 px transparent square. Windows and Linux icons use the emblem as-is. macOS icons put it on Apple's grid (an 824 px body centred on a transparent 1024 px canvas).

**Provenance:** generated from the owner's photo for the Spider brand, then converted to transparent PNGs (flood-fill background removal; see `spider-pdf-manager-icon-master.png`, the 1406 px master). The previous lion icon was replaced in the fork; see [LICENSE.txt](LICENSE.txt).
Licence: [LICENSE.txt](LICENSE.txt) (`MIT OR Apache-2.0`, like the repo).

## Files

| File | What it is |
|---|---|
| `spider-pdf-manager-icon-master.png` | the 1406 px transparent master; every PNG is rendered from it |
| `spider-pdf-manager.svg` | the master (a rendered 512 px PNG wrapped in SVG — no vector master exists yet); the hicolor scalable icon is a copy of it |
| `spider-pdf-manager-small.svg` | a lighter version (a rendered 256 px PNG wrapped in SVG) for places where size matters, such as this README |
| `spider-pdf-manager-1024.png` | 1024 px on Apple's grid; also the runtime Dock icon on macOS |
| `spider-pdf-manager.icns` | macOS icon (16–1024 px) |
| `spider-pdf-manager.ico` | Windows icon (16–256 px), embedded in `spider-pdf-manager.exe` by `apps/pdfcraft/build.rs` |
| `spider-pdf-manager-document.svg` | Windows PDF file icon master: a Lucide document outline, red badge and outlined PDF lettering; no font dependency |
| `spider-pdf-manager-document.ico` | Windows PDF file icon (16–256 px), embedded as resource 2 in `spider-pdf-manager.exe` |
| `hicolor/<n>x<n>/apps/com.spider.pdfmanager.png` | Linux hicolor theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/com.spider.pdfmanager.svg` | Linux scalable icon (copy of the master) |

Where it shows: `apps/pdfcraft/src/main.rs` sets the window icon (Dock, taskbar, Alt-Tab, launcher) and the
Wayland app id `com.spider.pdfmanager`. (The Linux `.desktop` file still uses the old id — `packaging/linux` is out of scope for the v1.0 rebrand.)

Windows PDF files use resource 2 (`"spider-pdf-manager.exe",-2`) when Spider PDF Manager is chosen as the default
reader; the app keeps resource 1. The document outline is adapted from Lucide `file-text`
1.49.0 (ISC); its badge and outlined lettering are original (`MIT OR Apache-2.0`), covered by
[LICENSE-document.txt](LICENSE-document.txt). Both icons are embedded, requiring no runtime
sidecar. This provides a file type icon; first-page thumbnails still need a thumbnail provider.

## Regenerate

```sh
# From the 1406 px master, with PIL (no resvg/iconutil needed for the PNG set):
#   hicolor sizes, the 1024 px Apple-grid canvas, the SVG wrappers and the .icns.
# The document .ico is a recolour of the old one (badge and strokes to the Spider red);
# a from-SVG rebuild needs resvg — see packaging/icons.sh.
cargo xtask assets        # then update the sha256 values in ATTRIBUTION.toml and run with --write
```
