# Deck application icon

Generated with **built-in imagegen** on 2026-10-18. The selected original artwork is
`app-icon-source.png`, preserved byte-for-byte from `exec-830bd87e-4d3c-4d0a-be34-51b27f2b04b3.png`.

- Original SHA-256: `bba3bfd3a7c2f67c10c661d6e9f5f11160599144044f38a381cc59c4aac67394`
- Design meaning: Three separate, gently fanned cards stand for flexible card creation and arranging content for print. Ivory, graphite and champagne connect Deck to the restrained Shadow/Echo family.

## Original generation prompt

```text
Use case: logo-brand. Create one finished macOS application icon for Deck, a flexible card-making and print-layout app.
Square 1024x1024 artwork, deep pure black background extending edge to edge, no rounded-square tile boundary drawn inside it. One centered sculptural emblem with a strong silhouette occupying about 74% of the canvas, ample black negative space.
The emblem is exactly THREE thin portrait-format cards, gently fanned and staggered, with softly rounded corners. The front card is a clean unprinted warm ivory metallic-paper plane, the two cards behind are graphite and muted champagne. Each card must remain visibly separate at small sizes. Elegant three-quarter perspective, almost frontal, restrained dimensionality.
Visual family: the Shadow app's two black-and-champagne planes illuminated through a narrow gold seam, and Echo's silver-white luminous sound rings on black. Match their quiet gallery-like lighting, subtle material grain, crisp bright edges and controlled luminous accents. Use a small soft amber glow only in the narrow gaps between the cards; most of the frame stays black. Warm ivory highlights, graphite shadows, no rainbow or colorful gradients.
No text, letters, words, numbers, suits, symbols, pictures, game motifs, extra objects, platform logos, watermark, drop-shadow tile, dramatic flares or floor. Professional restrained icon, not a realistic scene, instantly readable at 32px.
```

## Mechanical outputs

`app-icon.png` is 1024×1024. `app-icon.icns` contains the standard macOS 16, 32, 64, 128,
256, 512 and 1024 pixel representations, including the paired Retina entries.
Both derive directly from the opaque RGB original: macOS supplies the native outer presentation,
avoiding a second frame around a precomposed transparent tile. No repainting, cropping, filtering,
masking or background removal is applied. No Windows binary or ICO is claimed.

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
