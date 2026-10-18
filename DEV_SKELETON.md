# Deck source boundaries

Deck is a dedicated card authoring and print-layout application. The first extraction reuses
Shape's accepted CardKit implementation at a fixed source revision instead of rewriting its
layout, material-version checks or PDF painter.

The README describes verified user-facing behavior. Source, adjacent tests and build evidence
own implementation facts. This initial repository is under active development.

## Ownership

- Deck's application shell owns card navigation, project selection, user settings and presentation.
- The reused CardKit owners own card data, recoverable drafts, preview, accepted versions and
  card-face/print geometry. The reused Qt print owner paints both preview and PDF.
- Explicit integration adapters exchange selected material snapshots and receipts with Shape and
  Shadow. A connection must not become a second writable owner of either application's project.
- CLI/AI actions use the same card services as the desktop. Actual PDF export must consume the
  existing print plan and writer; a plan or material ZIP is not a PDF.
- Upstream dependency pins, imported source provenance and licenses remain reviewable.

## Initial scope

Flexible text, image and combined cards; quantities; image/table intake; card-size settings;
single-sided home-print sheets with crop marks; independent open/save; selected-material
integration; machine-readable actions and output verification.

Duplex registration, bleed, mixed-size packing, a game engine, a general plugin SDK and a
professional desktop publishing engine are separate future decisions.

## Compatibility

Keep old Shape card projects readable. Any extraction or migration must carry referenced image
bytes, unaccepted work and relevant identities, and must leave the original project intact.
Do not reinterpret historical layouts or rewrite accepted revisions during ordinary opening.
