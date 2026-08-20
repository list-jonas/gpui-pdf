# Local GPUI Patch

This is GPUI 0.2.2 from crates.io. It backports GPUI's native `PinchEvent` and macOS magnify handling until the application can move to a compatible upstream release.

## macOS vibrancy on macOS 26+

`WindowBackgroundAppearance::Blurred` used `NSVisualEffectMaterial::Selection`. macOS 26
stopped vending a backdrop layer for that material, so blurred windows rendered flat.
The blurred view now uses `UnderWindowBackground` (the material AppKit ships for
window-level vibrancy) with `BehindWindow` blending.

## Destination alpha on transparent windows

Quad, sprite, and surface pipelines wrote destination alpha additively
(`MTLBlendFactor::One`), so any translucent fill accumulated toward opaque and the
desktop blur behind the window disappeared. They now use Porter-Duff OVER
(`OneMinusSourceAlpha`) for the alpha channel as well.
