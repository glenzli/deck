# Deck application icon

The current balanced flat mark was created with **built-in imagegen** on 2026-10-18.
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

`app-icon.png` is 1024×1024. `app-icon.icns` contains the standard macOS 16, 32, 64, 128,
256, 512 and 1024 pixel representations, including the paired Retina entries.
Both derive directly from the opaque RGB original, reusing the established native black-background
presentation. No repainting, cropping, filtering, masking or background removal is applied.
No Windows binary or ICO is claimed.

From this asset directory on macOS, reproduce the outputs with:

```sh
sips --resampleHeightWidth 1024 1024 app-icon-source.png --out app-icon.png
ICON_WORK=$(mktemp -d)
mkdir "$ICON_WORK/app-icon.iconset"
for ICON_ENTRY in 16x16:16 16x16@2x:32 32x32:32 32x32@2x:64 128x128:128 128x128@2x:256 256x256:256 256x256@2x:512 512x512:512 512x512@2x:1024; do
    ICON_LABEL=${ICON_ENTRY%:*}
    ICON_SIZE=${ICON_ENTRY#*:}
    sips --resampleHeightWidth "$ICON_SIZE" "$ICON_SIZE" app-icon.png --out "$ICON_WORK/app-icon.iconset/icon_$ICON_LABEL.png"
done
iconutil --convert icns "$ICON_WORK/app-icon.iconset" --output app-icon.icns
```

The temporary iconset stays outside the repository. These commands never regenerate the original.
