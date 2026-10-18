# Deck

自由制卡、拼版与裁切线。Create flexible cards and print-ready sheets.

Deck brings card content, images, quantities and print layout into one dedicated workspace.
Use complete card images, text-only cards, or images with text. Game-card fields are optional.
It reuses [Shape](https://github.com/glenzli/shape)'s CardKit and Qt PDF implementation at a
fixed source revision; no separate renderer or project database engine is introduced.

The first extraction is under active development. This repository is not a packaged public release.

## Make and print cards

Create or reopen a `.deck` project. Use **Add** to start from a blank card, images or a list.
The card stays in the center while the right-hand **Content / Layout** inspector edits its text,
images, quantities and layout. Extra fields fold away, and per-card layout can override the set
defaults. Card proportions are independent of the physical print size.

Drafts save automatically. **Accept and print** checks and adopts the current card version before
opening the sheet preview; an already accepted version shows **Print**. Export an actual PDF with
card quantities and crop marks. Card-face export and version actions live in the **More** menu.
Print the PDF at **100% / actual size** to keep its physical measurements.

The initial scope is single-sided home printing. Duplex registration, bleed and mixed-size
packing are not implemented. Incomplete cards remain saveable drafts; print readiness is checked
separately. Accepted versions remain available after subsequent edits.

## Work with other applications

- **Shape → Deck:** continue one saved card set with all its referenced materials. Deck creates
  an independent project and can return explicitly exported PDF/ZIP files to Shape. Old mixed
  `.shape` projects use this selected-card transfer; Deck does not migrate unrelated workflows.
- **Deck → Shape:** send one card's image instruction to a dedicated Shape task. Accept and return
  an image there, then review and use it on the original card in Deck. Other cards can be edited
  while waiting. Shape owns its own image-generation authorization.
- **Deck → Shadow:** refine a copy in [Shadow](https://github.com/glenzli/shadow)'s independent
  editor and explicitly use the returned result. The original image remains intact.

Basic card editing and printing do not require either external application. Select the installed
applications in settings when needed. An older or unavailable handoff interface reports an error
instead of silently treating a launch as completion.

Books remain in [Storybook Co-Editor](https://github.com/glenzli/storybook-co-editor).
[Vox](https://github.com/glenzli/vox) is the planning repository for later script-led narration;
existing Shape speech stays in place.

## Development

The desktop uses Rust and Qt 6.9 or later, CMake 3.28 or later, Ninja and Python 3. Build products
stay outside this checkout. Cargo.lock and the fixed Shape Git revision make the upstream source
reproducible; a developer's separate Shape checkout is not required.

```sh
CARGO_TARGET_DIR=../.deck-local-build/cargo cargo test --locked --workspace
cmake --preset desktop-dev
cmake --build --preset desktop-dev
ctest --preset desktop-dev
```

For the shared local debug application, `scripts/build_and_promote_debug.sh` builds and validates
Deck before atomically updating `../.deck-local-build/current-debug/Deck.app`. Launch that version
with `scripts/run_debug.sh`. The promotion helper also accepts an already validated bundle and
its native-smoke evidence, so the application being tested is the one being installed.

The Qt executable includes a sibling `deck-cli`. That packaged CLI exposes machine-readable
capabilities and exact-version actions using the same card services as the desktop:

```sh
deck-cli capabilities
deck-cli status /absolute/path/cards.deck
```

See [AI and CLI usage](docs/AI.md) for request examples and actual native PDF export. A print plan
is layout data; `print-pdf` invokes the real Qt writer and independently verifies its output.
Live image generation additionally requires an available, authorized Shape runtime.

See [source boundaries](DEV_SKELETON.md), [application boundaries](docs/BOUNDARIES.md) and
[upstream provenance](third_party/shape-ui/sources.json).
