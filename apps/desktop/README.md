# Deck desktop

Deck owns one direct card-editing window, project admission, local settings, Shape handoffs and
explicit output return. It does not load Shape's graph, picture-book or audio product shell.

The card face occupies the main canvas. A compact card list and one Add menu sit on the left;
the right inspector switches between content and layout without leaving the card. Image-only
cards start with their image and copy count; preserved hidden text stays expandable. Whole-set
proportions, defaults and creative direction are edited in the same inspector. The top bar keeps
printing prominent, while project, version, inspection and archive actions live in named menus.
Saving remains automatic; choosing a version or an external image remains an explicit action.

## Reused owners

The source revision is fixed by Cargo to `983022bf35bcfda332ef57aeab81d8015cf5f280`.
`scripts/locate_shape_source.py` locates that exact `shape-desktop-bridge` package through Cargo
metadata. CMake compiles the original desktop backend, card authoring controller, image cache and Shadow
working-copy controller from that source. Generated CXX headers come only from the current Cargo
build-script output. A developer Shape checkout is not a dependency.

`third_party/shape-ui/sources.json` records original hashes for the supplied MIT card UI and PDF
owner. The local card workspace is a Page; it retains stable IDs, recoverable drafts, flexible and
legacy card layouts, batch intake and explicit version acceptance. Direct Infer controls are
replaced by a real Shape image task. Shadow retains its original independent process and strict
candidate-return contract. Source provenance is not a claim that adapted files remain byte-identical.

The complete physical print controller, contract and painter are retained in `src/card_print_export.*`. GUI previews and
PDFs use the same plan, SVG resources and geometry. Both `.deck` and `.shape` project destinations
are excluded from PDF publication. Exports and receipts use new files without overwriting.

## Build and run

```sh
cmake --preset desktop-dev
cmake --build ../.deck-local-build/desktop-dev --parallel 2
ctest --test-dir ../.deck-local-build/desktop-dev --output-on-failure
scripts/build_and_promote_debug.sh --app /path/to/validated.app --evidence /path/to/passed-smoke
scripts/run_debug.sh
```

`deck-package-cli` refreshes the bundled CLI even after a CLI-only rebuild. Promotion checks the
exact application and packaged CLI hashes against passed native evidence, copies an immutable
release, then atomically advances `current-debug`. It does not rebuild a supplied validated app.

## Application entry points

- `--project <bundle> --kit <id>` opens a specific Deck card set after read-only content admission.
  Mixed Shape projects must be transferred explicitly; extension alone is never authority.
- `--external-edit <request.json> --result <new-result.json>` receives a Shape card transfer or
  an associated Deck project. Only explicit PDF/ZIP exports followed by Return outputs complete
  the handoff. Closing returns cancellation and preserves saved work.
- `--export-pdf <request.json> --result <new-result.json>` is file-only and uses the same Qt painter.
  Its request schema is `deck.pdf-export/1`; omitted options use the core defaults. The packaged
  CLI independently verifies the completed PDF receipt and bytes.
- `--handoff-capabilities` prints protocol versions without opening a GUI or project.

Shape image tasks run the actual selected Shape application with its own generation credentials.
Protocol negotiation is bounded; human editing has no short timeout. Results carry request hashes
and captured card identity. Import validates the complete image return through Deck's public CLI;
Use this image is a separate explicit card binding. Changing other cards does not retarget a task.

## Localization and verification

`scripts/update_translations.py` extracts actual QML/C++ identities and restores the supplied and
Deck-owned Simplified Chinese catalog. `scripts/check_translations.py` rejects unfinished entries.
The linked `--smoke-native` consumer creates and reopens real projects, checks a Unicode/space/#
path, saves/accepts cards, produces multi-page PDFs through GUI and CLI, and captures 1280px and
1120px English/Chinese workbenches. Actual pointer interactions cover adding, stable-ID row
reordering/removal, inline layout scopes, image-task and project menus, hidden text and printing.
It also exercises late import and cancelled/unsupported Shape-probe guards.
`DECK_SHADOW_BINARY` opts into the real isolated Shadow pipeline consumer; no user profile or
inference credentials are used by the default smoke. Evidence is written outside the checkout.
