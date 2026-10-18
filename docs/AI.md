# Deck CLI and AI calls

Deck's CLI is a real consumer of the same pinned CardKit services used by the desktop. It does not
reimplement card layout, silently accept a proposal, use another application's inference credentials,
or inspect another desktop session's transient Candidates.

The dependency is Shape commit `983022bf35bcfda332ef57aeab81d8015cf5f280`. Cargo uses the public
Git dependency and Deck's lockfile, never a developer's absolute Shape checkout. Native builds locate
that same source through Cargo metadata and reuse its CXX bridge and Qt print painter.

## Discover before acting

```sh
deck-cli capabilities
deck-cli status /absolute/words.deck
```

Every command writes one JSON object to stdout with `schema: "deck.cli.result/1"`. A successful
command has `status: "ok", result: ...` and exits 0. A rejected command has `status: "error", code,
message` and exits 1. Request examples in capabilities contain clearly marked placeholders;
replace those values, not the action names. `argv` arrays are arguments, not shell programs.

Status uses the existing current-schema, read-only inspection transaction. It never migrates a
project, reads material payloads, initializes speech defaults or changes a saved draft. It reports
project and kit identities, exact accepted source pins, saved draft digests, all bound material
pins/current heads, version blockers and concrete load/check requests. `observed` explicitly says
that Runtime readiness, material bytes and other sessions' Candidates were not checked.

Before the desktop opens a project, it must inspect `result.open_guard.allowed`. Unsupported
artifact families or operators produce `allowed: false, needs_transfer: true`, with their exact
identities. This checks content, not a file extension. A mixed Shape project must be exported by
Shape as one complete selected-kit transfer; Deck must not open it through a mutable DesktopSession
or copy a live SQLite database/WAL. A card-only compatible project remains compatible.

## Create, patch, review and explicitly accept

Save this as `create.json`:

```json
{"name":"Words","composition":{"name":"Words","canvas_width":750,"canvas_height":1050,"layout":{"mode":"auto"},"cards":[{"id":"spring","rules":"SPRING\n春"},{"id":"summer","rules":"SUMMER\n夏"}]}}
```

```sh
deck-cli create /absolute/words.deck create.json
deck-cli status /absolute/words.deck
```

Creation atomically publishes a new project and saved draft. It does not create an accepted kit.
Existing destinations are never overwritten. New compositions cannot contain foreign material pins;
use import-transfer or import materials first.

Use `result.kits[].draft.requests.load` from status as a `card-kit` request:

```json
{"action":"draft_load","artifact_id":"<kit UUID>"}
```

```sh
deck-cli card-kit /absolute/words.deck load.json
```

The result contains `draft.artifact_id`, `draft.expected_head`, `draft.draft_digest` and the complete
`draft.composition`. Patch that complete composition; preserve stable card ids, hidden fields,
unselected cards and exact source pins. Then submit:

```json
{"action":"draft_save","artifact_id":"<kit UUID>","draft_digest":"<last loaded/saved digest>","composition":"<the complete patched composition object>"}
```

The composition placeholder above must become an object, not a string. Saving requires the exact
last observed draft and output head. Repeating an unchanged save with the current digest is safe;
a stale save fails instead of overwriting newer work. Do not add layout to an old composition that
has none unless the author explicitly chooses to upgrade its appearance.

Check the saved draft:

```json
{"action":"preview","artifact_id":"<kit UUID>","draft_digest":"<current saved digest>","card_id":"spring"}
```

Review `result.preview`, its card issues, `ready_to_export`, and `result.review_digest`. Matching
versions alone do not establish printable content. A checklist can be incomplete; image/text mode,
hidden content, typography and print requirements remain owned by the existing CardKit validator.

Adoption is a separate request:

```json
{"action":"accept","artifact_id":"<kit UUID>","draft_digest":"<current saved digest>","review_digest":"<digest from the exact reviewed preview>"}
```

Accept returns `result.source = {artifact_id, revision_id}` and a rebased draft. Use the returned
draft digest for subsequent edits. Old accepted revisions and their pinned inputs remain immutable.
An accepted checklist is still not necessarily export-ready.

## Read and publish the accepted version

```json
{"action":"read","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted kit revision UUID>"}}
```

Read returns the exact historical composition and authoritative preview; it never substitutes a
newer accepted head. For a ZIP of card materials, use:

```json
{"action":"export","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted kit revision UUID>"},"destination":"/absolute/new-materials.zip"}
```

ZIP export returns its byte length and BLAKE3. It is an offline material package, not a PDF or an
editable project transfer. An existing destination or an incomplete/overflowing kit is rejected.

`print_plan` accepts the same source and optional `options`. It returns unique card SVGs and physical
sheet/crop-mark geometry. It does not create a PDF. To paint a real PDF using the desktop's same Qt
writer, save:

```json
{"schema":"deck.pdf-export/1","project":"/absolute/words.deck","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted kit revision UUID>"},"options":{"paper":"a4"},"destination":"/absolute/new-cards.pdf"}
```

```sh
deck-cli print-pdf /absolute/words.deck pdf.json --desktop /path/to/deck-desktop
deck-cli hash-file /absolute/new-cards.pdf
```

Options can be omitted or `{}` for defaults. Letter uses `"paper":"letter"`. Existing optional card
width/height, margin/gap and crop-mark settings retain their Core definitions; a missing axis derives
from the composition ratio. Print options do not rewrite accepted content. This remains single-sided
home printing, without bleed, duplex registration or commercial prepress claims.

The CLI canonicalizes the project identity, snapshots the admitted request and invokes
`deck-desktop --export-pdf REQUEST --result NEWRESULT`. A successful native receipt is required.
The CLI then independently reads the regular output, checks its identity/length/PDF header/BLAKE3,
and returns `independently_verified: true`. This verifies the published bytes; native rendering tests
own visual correctness. Select the writer with `--desktop`, `DECK_DESKTOP_EXECUTABLE`, or a packaged
sibling executable. No separate Rust PDF renderer is used.

## Import local artwork and complete transfers

`deck-cli import-raster PROJECT REQUEST` takes
`{"name":"Finished face","path":"/absolute/face.png"}` (PNG or JPEG). It returns a new local accepted
material `source` and revision. It neither binds a card nor accepts a kit. Bind the exact returned pin
with an explicit draft_save. Core owns any canonical PNG materialization.

```sh
deck-cli import-transfer /absolute/new.deck /absolute/transfer/manifest.json --expected-blake3 HEX
```

Transfer schema is `deck.card-transfer/1`:
`schema, request_id, origin, composition, materials`. Origin identifies the foreign Shape project,
kit, expected head and saved draft digest. Each material has foreign `source`, `content` (existing
ContentRef), `contract` (null is allowed for legacy text), and `relative_path`. ContentRef.digest is
the existing array of 32 byte values; separate *_blake3 fields are hexadecimal strings.

Materials must cover every composition input exactly once, including hidden artwork/text. Bounds
are 64 cards/materials, 20 MiB per PNG, 8 KiB per text and 128 MiB total material bytes. Paths remain
inside the transfer directory and cannot traverse parents or symbolic links. Unknown schemas,
missing/extra/duplicate material records, changed bytes or mismatched contracts fail before a target
project is published. A random staging directory is removed on failure.

Deck imports new local material identities, rewrites only those pins and preserves card ids,
unfinished text, prompts, hidden fields, legacy layout absence and all other authored settings.
`result.mappings` records source/local content and contracts, including any image canonicalization.
The imported kit remains an unaccepted draft. A local `deck-transfer.json` receipt is published
atomically with the project.

Retrying the same destination and exact manifest returns the same project/kit with
`already_imported: true`. It never resets edits made after import. `draft` is the original import
receipt; `current` is the newly inspected state on retries. Load the current draft before editing.
Another manifest cannot replace that destination.

## Shape image generation and human handoffs

Discover real GUI routes first:

```sh
/path/to/deck-desktop --handoff-capabilities
/path/to/shape-desktop --handoff-capabilities
```

Deck opens its own document with `--project ABSOLUTE_PROJECT --kit KIT_ID`. Shape can request an
interactive Deck edit with `--external-edit REQUEST --result NEWRESULT` using
`deck.external-edit/1`. Only explicit exported outputs and a terminal receipt establish completion;
launching, saving, or generating a preview does not. PDF/ZIP returns are verified links in Shape,
not fabricated image artifacts or foreign local pins.

For one new image, Deck launches Shape's real
`--deck-material-task TASK --result NEWRESULT` entry point. Shape owns its inference authorization,
Candidates, explicit image acceptance and Return to Deck. This is an interactive handoff, not
headless image generation, and handshake availability is not model readiness.

A task uses `deck.material-task/1` with `request_id`,
`target:{project_id,kit_id,card_id,draft_digest,artwork,card_snapshot?}`, `prompt`, and
`canvas:{width,height}`. New callers include the complete captured CardKitCard as card_snapshot;
it is bounded to 24 KiB and must agree with the original card id/artwork. The draft digest remains
the original request context. Other cards can be edited or reordered while waiting. Changing the
original card prevents silent binding. Older tasks without a card snapshot use the stricter whole
saved-draft digest check.

When Shape explicitly returns an image, this real CLI action verifies it:

```json
{"task_path":"/absolute/task.json","result_path":"/absolute/result.json","expected_request_blake3":"<exact original task hash>","name":"Returned artwork"}
```

```sh
deck-cli import-material-return /absolute/words.deck import-return.json
```

The result must be `deck.material-task-result/1`, completed for the same request id/hash, with an
actual PNG, exact byte length/BLAKE3 and a complete raster contract. PNGs are bounded to 20 MiB,
4096 per axis and 16,777,216 pixels; the original importer verifies their interpretation before
the live project is mutated. Deck also rechecks the original project, kit and card snapshot.
It returns a new local accepted material pin with `bound_to_card:false, accepted_kit:false`.
The foreign Shape source remains provenance only. The desktop offers explicit **Use this image**;
AI callers explicitly bind the returned local pin through draft_save, then review/accept the kit
separately. Stale returns remain available as files for an explicit manual import.

The full request/result and transfer contracts are documented in Shape's
[Deck handoff contract](https://github.com/glenzli/shape/blob/main/docs/DECK_HANDOFF.md).
