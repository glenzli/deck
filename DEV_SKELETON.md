# Deck source boundaries

Deck is a dedicated card authoring and print-layout application. The first extraction reuses
Shape's accepted CardKit implementation at a fixed source revision instead of rewriting its
layout, material-version checks or PDF painter.

The README describes verified user-facing behavior. Source, adjacent tests and build evidence
own implementation facts. This initial repository is under active development.

## Ownership

- Deck's application shell owns card navigation, project selection, user settings and presentation.
  The icon image provider rasterizes packaged UI vectors at physical pixel density with theme
  tinting; it is independent of the card artwork preview and print owners.
- `CardLayoutPresetStore` owns the personal layout library beside application settings, with
  bounded JSON, atomic writes and a short file lock. Presets contain resolved layout settings,
  never card text or material pins. Workspace batch application uses stable card IDs and one
  draft save; its local undo checks the target state before restoring layouts.
- Block layouts extend the existing CardKit layout contract. Ordered, bounded rectangles bind
  to card fields rather than copying content. The domain validates geometry and styles, and the
  core renders both the authoring SVG and physical PDF faces. The QML editor owns selection,
  transient gestures and guarded commits; it does not introduce another card renderer or store.
- The pinned CardKit domain/core owners, adapted under `third_party/shape-domain` and
  `third_party/shape-core`, own card data, recoverable drafts, preview, accepted versions and
  card-face/print geometry. The reused Qt print owner paints both preview and PDF.
- Explicit integration adapters exchange selected material snapshots and receipts with Shape and
  Shadow. A connection must not become a second writable owner of either application's project.
- CLI/AI actions use the same card services as the desktop. Actual PDF export must consume the
  existing print plan and writer; a plan or material ZIP is not a PDF.
- Upstream dependency pins, imported source provenance and licenses remain reviewable. The
  vendored owners retain their complete crate boundaries and MIT licenses; `shape-cardkit-provenance.json`
  lists base file hashes and intentional adaptations. CLI and native bridge resolve the same Cargo
  patches. Image placeholders are authoring aids; missing artwork still blocks printing.
- Deck's transient sheet choices are admitted by the CLI print owner, shared by desktop preparation
  and PDF commands. The core planner fits a chosen grid and supplies shared dashed cut guides for
  tightly packed sheets, alongside compatible corner marks. It owns duplex slot reflection, empty
  slot preservation, back calibration and front/back page order. Authored shared backs belong to
  the CardKit domain and accepted material lifecycle; they are separate from sheet choices.
  Qt validates the plan and paints its coordinates without a second layout calculation. Card
  rendering, readable text checks and the single preview/PDF painter stay reused.
- The card draft assistant owns asynchronous text-candidate lifecycle and cancellation. The
  packaged CLI's assistant owner uses the pinned official Infer Runtime SDK for discovery,
  authentication, exact contracts and local-only requests. It never writes project content;
  explicit candidate application uses the existing CardKit draft service with a captured target.

## Initial scope

Flexible text, image and field-bound block cards; quantities; image/table intake; card-size settings;
home-print sheets with shared cut guides; personal presets and batch layouts; shared image or
solid card backs; single-sided or duplex PDF; independent open/save; selected-material integration;
machine-readable actions and output verification.

Bleed, mixed-size packing, per-card backs, a game engine, a general plugin SDK and a professional
desktop publishing engine are separate future decisions. Back offsets aid physical calibration;
the PDF workflow does not operate printers or guarantee their mechanical alignment.

## Compatibility

Keep old Shape card projects readable. Any extraction or migration must carry referenced image
bytes, unaccepted work and relevant identities, and must leave the original project intact.
Do not reinterpret historical layouts or rewrite accepted revisions during ordinary opening.
