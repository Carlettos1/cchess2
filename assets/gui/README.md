# Minimal GUI art

Original vector placeholders for the proposed desktop client. No downloaded fonts,
images or icons are required. This directory is presentation-only; gameplay content
continues to come from the existing RON registries.

- `manifest.json` maps every current piece/card definition to an asset.
- `pieces/` contains neutral 64×64 tokens matching the TUI abbreviations.
- `cards/` contains 160×220 templates. Render real card text/costs as UI text.
- `theme.css` contains the proposed dark theme colors.
- `app-icon.svg` is the source for future platform icon exports.

Rebuild from the repository root with `python3 scripts/generate_gui_assets.py`.
The generator also writes the mockup and contact sheet in `docs/gui/`.

Token components should inline the SVG and override its group color, then overlay
the seat marker. Owner markers: P0 circle, P1 diamond, P2 square, P3 triangle.
Do not rely on tint alone. Text labels remain upright when the board rotates.

The contact sheet and match concept are design references, not running gameplay.
See `docs/gui/IMPLEMENTATION_PLAN.md` for the full implementation and QA plan.
