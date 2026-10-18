# Deck application icon

The current balanced flat mark was created with **built-in imagegen** on 2026-10-18,
then normalized with **Icon Forge** on the same date.
`app-icon-source.png` is preserved byte-for-byte from
`exec-521ed80d-7398-4030-b005-3eff81d5fc5f.png`.

- Original SHA-256: `e008d8b0c860d3cf7683c568af971aa58af1c1fa534f9b3f17d142f75a4303fb`
- Design meaning: Three shorter, wider blank cards form a compact flat fan with balanced overlap and black breathing room. Ivory, charcoal and muted champagne express flexible card creation and arranging content for print.
- Previous sources and complete prompts are preserved in the
  [dimensional archive](archive/2026-10-18-dimensional/README.md) and
  [first flat study](archive/2026-10-18-flat-study/README.md).

## Imagegen edit prompt

The archived first flat study was the exact reference for this proportion edit.

```text
Use case: precise-object-edit. Edit target: the provided flat Deck icon.
Redesign the proportions and optical balance; the current front card is too tall, too dominant, and the fan fills too much of the canvas.
Keep exactly three blank overlapping cards on opaque pure black, using warm ivory, charcoal gray, and muted champagne. Keep flat two-dimensional graphic rendering. No textures, shadows, gradients, bevels, glows, or side faces.
Rebuild all three cards as IDENTICALLY SIZED standard playing-card rectangles: width to height about 0.72, subtly rounded corners. The front ivory card must be visibly shorter and wider than in the reference, not a long tall slab. The three cards form a compact, gentle fan around a shared lower pivot, with modest rotations near minus 16, zero, and plus 16 degrees, not a broad spread. Show a comfortably broad exposed strip of each rear card; they must not look like skinny slivers. Avoid heavy outlines.
Scale the complete group down so its total silhouette occupies approximately 56 percent of the square canvas width and 58 percent of its height. Center it OPTICALLY on the canvas, with balanced black space around it. The whole fan should feel compact, balanced and deliberate, without the ivory front card overpowering the dark and gold cards.
Perfectly clean matte solid-color shapes, premium restrained app icon, immediately recognizable as a deck at 32 pixels.
No text, suits, marks, borders, new symbols, or outer rounded tile. The square black background stays opaque. The goal is improved relative sizes, card aspect ratios, overlaps, breathing room and visual center.
```

## Mechanical outputs

`app-icon.png` is Icon Forge's 1024×1024 RGBA rounded mother image. Its default geometry places
the original artwork in a centered 824×824 tile with a 186 px corner radius and 100 px of
transparent space on each side. The intended black background stays inside the tile; AI
background removal is skipped. The source artwork remains byte-identical.

`app-icon.icns` is generated from that same mother image and contains the standard macOS 16,
32, 64, 128, 256, 512 and 1024 pixel representations, including paired Retina entries.
Qt window branding and the macOS bundle therefore use the same rounded artwork and padding.
Only these PNG/ICNS outputs are used by Deck; the tool's other platform outputs are temporary
conversion products, not evidence of Deck builds for those platforms.

- Icon Forge source: `https://github.com/glenzli/icon-forge`, revision
  `e9cd09e88e65390018b59921441843ec80ad3ca2`
- Locked conversion tools: `sharp@0.33.5`, `@tauri-apps/cli@2.11.2`
- PNG SHA-256: `e855513c31b52d12fcff82cc3e9e0a71f4002dfa240c296f53828354c6a62484`
- ICNS SHA-256: `c26e8dc5803d03e3d88d3960668393d8acf6f8846daea12b7cfddc4510d8d400`

From the Deck repository root, set `ICON_FORGE_ROOT` to a checkout of that revision with its
locked Node.js dependencies installed, then reproduce the outputs with:

```sh
DECK_ROOT="$PWD"
DECK_ICON_OUTPUT=$(mktemp -d)
(
    cd "$ICON_FORGE_ROOT"
    node forge.js --input "$DECK_ROOT/apps/desktop/assets/app-icon-source.png" \
        --output "$DECK_ICON_OUTPUT" --skip-rembg
)
cp "$DECK_ICON_OUTPUT/rounded-source.png" apps/desktop/assets/app-icon.png
cp "$DECK_ICON_OUTPUT/tauri/icon.icns" apps/desktop/assets/app-icon.icns
```

Temporary conversion products stay outside the repository. These commands never regenerate the
original artwork, and Deck builds use the checked-in outputs without requiring Icon Forge.
