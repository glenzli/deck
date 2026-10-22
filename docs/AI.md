# Deck CLI and AI calls

## Card text suggestions

`deck-cli assist request.json` uses the official Infer Runtime SDK at the existing locked
revision `c74c503a27b74d76ab01bc1d9990d56e21c45969`. It discovers the dated Consumer Core and
Responses capability; no hard-coded endpoint, provider credentials or Shape token is used.
Set `DECK_INFER_CREDENTIAL_FILE` to Deck's own owner-only token file. The desktop selects this
file in Settings or discovers `infer.token` beside its own settings file. Configure an independent
Deck identity in Infer Runtime Apps & Access with `text.edit`, local-only placement, no fallback,
zero cost, interactive priority, and the foundational capability floor. Console identity changes
require restarting its managed Runtime before the identity can be used.

```json
{"schema":"deck.assistant/1","operation":"start","prompt":"Six nature observation cards for children","count":6,"language":"en"}
```

`operation:"probe"` checks Core/Responses availability and, when credentials are configured,
authenticated access. A start uses foreground SSE without requiring durable background storage. Its stdout is newline-delimited
JSON: a `deck.assistant.progress/1` receipt exposes the job identity, followed by one terminal
`deck.cli.result/1` envelope containing the validated suggestions. `operation:"cancel"` uses Core Job
cancellation for that identity and reports the actual confirmed
`cancelled` flag. A stopped wait is never reported as confirmed provider cancellation.

Suggestions are strictly bounded JSON with `title`, `rules` and optional `art_prompt`; counts
are 1–16 per request. They never contain trusted card IDs, material pins or automatic acceptance.
The desktop retains the project, kit and complete draft snapshot, previews the suggestions, and
appends fresh card IDs only after explicit review while that snapshot is still current. Existing
card data and accepted history remain owned by the CardKit draft service. AI callers likewise
apply reviewed text through the normal exact-digest `draft_save` action.

Image generation continues through the existing Shape handoff and Shape's Infer Runtime access;
Shadow remains the independent specialist image editor.

## Card authoring and printing

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

## One shared card back

The optional composition-level `back` applies to every card in that accepted kit version. Its two
forms are:

```json
{"mode":"solid","color":"#123456"}
```

```json
{"mode":"image","artwork":{"artifact_id":"<local accepted image UUID>","revision_id":"<exact accepted image revision UUID>"},"image_fit":"contain","background_color":"#ffffff"}
```

Colors use `#RRGGBB`. An image uses an exact local accepted material pin; `image_fit` is `contain`
(default) or `cover`, and `background_color` defaults to `#ffffff`. Import an image with
`import-raster` first if needed, then use its returned `source` as `back.artwork`. Removing `back`
or setting it to null selects no shared back. Historical compositions without this field keep
their existing front appearance and serialization.

These snippets are values for `draft.composition.back`, not replacement compositions. Load the
current saved draft, patch its complete composition, and use the exact current digest in
`draft_save`. Preview that saved digest and review both the printed fronts and
`result.preview.back.svg`; `result.preview.back.ready_to_print` reports back readiness separately
from the front `ready_to_export` flag. Submit `accept` with the same saved draft digest and the
returned `review_digest`, then use the newly returned accepted `source` for the print request.
A saved back draft does not change an earlier accepted source.

Image mode with `artwork:null` remains a recoverable saved or accepted state, but cannot print
backs. `duplex` or `backs` rejects an absent back with `card_print.back_required` and an image
without artwork with `card_print.back_incomplete`. Front-only printing ignores an unused back;
back-only printing checks the back without rendering or checking unprinted front text. Back image
pins participate in the same exact-head acceptance checks as front inputs. Historical accepted
versions continue to read their original image revision.
Compositions with a back require a consumer that understands this Deck extension; use exported
PDFs with older consumers.

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

`print_plan` accepts the same source and optional `options`. It returns unique SVG resources and
physical sheet/crop-mark geometry. It does not create a PDF. To paint a real PDF using the same
desktop Qt writer, save:

```json
{"schema":"deck.pdf-export/1","project":"/absolute/words.deck","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted kit revision UUID>"},"options":{"paper":"a4"},"destination":"/absolute/new-cards.pdf"}
```

```sh
deck-cli print-pdf /absolute/words.deck pdf.json --desktop /path/to/deck-desktop
deck-cli hash-file /absolute/new-cards.pdf
```

Options can be omitted or `{}` for defaults. Letter uses `"paper":"letter"`. Existing optional card
width/height, margin/gap and crop-mark settings retain their Core definitions; a missing axis derives
from the composition ratio. Print options do not rewrite accepted content. The same planner and
Qt painter support front-only, interleaved front/back and back-only home-print PDFs. Back alignment
uses the explicit millimetre offsets below; physical printer calibration, bleed and commercial
prepress remain outside this file-export contract.

Both `print_plan` and `deck.pdf-export/1` also accept an optional top-level `imposition`:

```json
{"columns":2,"rows":3,"content":"one_each"}
```

Columns and rows must both be supplied (1–8 each), or both omitted for fixed-size automatic
packing. An explicit grid proportionally fits the accepted canvas and overrides the optional card
dimensions. `content` is `quantities` (default), `one_each`, or `repeat`. Repeat requires `card_id`
from that exact accepted version; optional `copies` is 1–4096, and omission fills one sheet.
Printed fronts still pass Shape's resource/readability checks. Deck translates its admitted
slot/crop template and reuses its SVGs; accepted quantities, drafts and history never change.

For a shared back, `imposition.sides` is `fronts` (default), `duplex`, or `backs`. `flip_edge` is
`long` (default) or `short`. Signed `back_offset_x_mm` / `back_offset_y_mm` default to zero and must
be finite values between -10 and 10 mm with at most three decimal places. Positive values move
the back right/down in its page preview. The margin sets the front layout; back offsets may enter
that blank area, but complete cards, content and cut-line strokes must remain on the paper.
An unsafe offset returns `card_print.back_offset`.

For example, after explicitly accepting a kit with its shared back, save this as `duplex-plan.json`:

```json
{"action":"print_plan","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted revision including the shared back UUID>"},"options":{"paper":"a4","margin_mm":5,"cut_line_style":"dashed_grid","gap_mm":0.2,"stroke_width_mm":0.2},"imposition":{"columns":3,"rows":3,"content":"quantities","sides":"duplex","flip_edge":"long","back_offset_x_mm":0,"back_offset_y_mm":0}}
```

```sh
deck-cli card-kit /absolute/words.deck duplex-plan.json
```

Use the same options and imposition to export the real PDF:

```json
{"schema":"deck.pdf-export/1","project":"/absolute/words.deck","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted revision including the shared back UUID>"},"options":{"paper":"a4","margin_mm":5,"cut_line_style":"dashed_grid","gap_mm":0.2,"stroke_width_mm":0.2},"imposition":{"columns":3,"rows":3,"content":"quantities","sides":"duplex","flip_edge":"long","back_offset_x_mm":0,"back_offset_y_mm":0},"destination":"/absolute/new-duplex-cards.pdf"}
```

Save that request as `duplex-pdf.json` and run:

```sh
deck-cli print-pdf /absolute/words.deck duplex-pdf.json --desktop /path/to/deck-desktop
```

Duplex pages are interleaved `front0, back0, front1, back1`. Each page has a zero-based `index`,
`sheet_index` and `side:"front"|"back"`. Slots retain the original `card_id` and one-based
`copy_index`, plus `face`; `resource_id` selects an entry in `cards[]` and falls back to `card_id`
on fronts. Every back references the single resource `@deck/back`. Artwork remains upright;
the planner reflects physical slot and cut-line positions for the selected flip edge. Consumers
paint the returned `rect_mm`, `content_rect_mm` and `crop_marks` directly, without another mirror
or reorder operation.

`total_copies` counts physical cards. New back/duplex plans add `total_placements` (twice that count
for duplex, equal for backs) and `sheet_count` (physical sheets); `page_count` counts PDF pages.
The final partial sheet receives backs only for its occupied copies. `sides:"backs"` returns
`back0, back1` without blank front pages or front SVG payloads, using the same physical copy slots.
The job limits are 4096 physical cards, 8192 duplex placements, 256 physical sheets and 512 duplex
PDF pages (256 for a single side). Up to 64 front SVGs and one shared back SVG are retained once,
with at most 512 slots per page.

The plan includes normalized `imposition` settings with its physical geometry. Omitting the new
fields keeps front-only behavior. Legacy requests without `imposition` return the unchanged Shape
plan; old plans can omit the new side/resource/count fields. For those plans, placements equal
`total_copies`, and sheets equal `page_count`.

Open the exported PDF in a viewer and print at **100% / Actual size**, with the same long/short
flip edge selected for duplex printing. Print one sheet to check paper feeding and alignment;
adjust the back offsets if needed before exporting the full job. Deck exports the PDF and leaves
printer selection and paper feeding to the printing application.

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

Materials must cover every composition input exactly once, including hidden artwork/text and the
shared back's image pin. Reused pins appear once. Bounds
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

## Deck card-layout extensions

In flexible composition/card `layout`, optional `image_position` is `auto`, `top`, `bottom`,
`left`, `right` or `background`; `image_share_percent` is 20–80. `text_vertical_align` is
`auto`, `top`, `center` or `bottom`. Signed `text_offset_x_percent` / `text_offset_y_percent`
are -30–30, relative to the complete canvas width/height, and move the complete text block.
Readability checks still reject printing when positioned text leaves the card. The same owner
renders draft previews, accepted card files and physical sheets. A missing-image placeholder
exists only in authoring previews; it never substitutes for accepted artwork in a PDF.

Absent fields preserve old layout behavior and serialization. Editable compositions using these
new options require Deck or another consumer implementing these extensions; older Shape builds
can consume exported PDFs/card images. Opening old projects never adds these fields or rewrites
accepted revisions. Local adaptations and exact upstream hashes are in
`third_party/shape-cardkit-provenance.json`.

## Field-bound card blocks

`layout.mode:"blocks"` places text and artwork through a bounded `layout.blocks` array. Each
block names a card field; it contains no copied card text, arbitrary text body or material pin.
Applying the same layout to another card reads that card's own fields. A `rules` binding uses the
card's exact `text_material` when present, and an `artwork` binding uses its existing accepted
image pin. Editing blocks leaves those contents and references unchanged.

The array contains 0–16 blocks, in drawing order from back to front. IDs are unique ASCII strings
of 1–64 characters from `[A-Za-z0-9_-]`. Multiple blocks can bind the same field. Every block has
exactly these required fields, including text-style fields on artwork blocks:

| Field | Contract |
| --- | --- |
| `id` | Stable unique block ID within the array. |
| `field` | `title`, `rules`, `artwork`, `card_type`, `cost`, `stats`, or `flavor`. |
| `x`, `y` | Integers 0–10000, measured from the whole canvas's top-left. |
| `width`, `height` | Integers 1–10000; `x + width <= 10000`, `y + height <= 10000`. |
| `text_scale_percent` | Integer 50–300. |
| `text_align` | `left`, `center`, or `right`. |
| `text_vertical_align` | `top`, `center`, or `bottom`; block alignment has no `auto` value. |
| `bold`, `italic` | Booleans. |
| `text_color` | `#RRGGBB`. |
| `image_fit` | `contain` or `cover`. |

Geometry uses integral ten-thousandths: 100 units is 1% of that complete canvas axis. It is
independent of canvas pixels, paper millimetres and the automatic layout's padding. Extra block
keys, numeric strings, fractional geometry, invalid types, duplicate IDs and rectangles outside
the canvas are rejected.

For example, this layout fragment binds a heading and a body to their existing fields:

```json
{"mode":"blocks","blocks":[{"id":"heading","field":"title","x":600,"y":600,"width":8800,"height":1800,"text_scale_percent":110,"text_align":"center","text_vertical_align":"center","bold":true,"italic":false,"text_color":"#202020","image_fit":"contain"},{"id":"body","field":"rules","x":600,"y":2800,"width":8800,"height":6600,"text_scale_percent":100,"text_align":"left","text_vertical_align":"top","bold":false,"italic":false,"text_color":"#202020","image_fit":"contain"}]}
```

Patch it into the complete loaded composition's layout or a card's layout, preserving the other
layout settings and every card field. Use the normal exact-digest `draft_save`, review the new
preview and explicitly `accept` before printing the returned version. Per-card layouts still
require a flexible kit layout. Composition size remains bounded to 60 KiB.

The block list inherits as a whole: an absent per-card `blocks` field inherits the kit list,
`blocks:[]` explicitly clears it, and a supplied list replaces it rather than merging by ID.
Switching to another mode preserves a stored list while that mode ignores it. Returning to
`blocks` reuses the list. Old layouts without blocks retain their previous behavior and do not
gain a block array during ordinary reading. A resolved blocks layout always contains an array,
including `[]` for an unconfigured layout.

An empty array is a valid incomplete draft. Unbound content stays on the card and is reported as
hidden; a block that requests artwork still needs an accepted image. Preview and print retain
the existing missing-content, crop and readable-text checks. The same rendering owner supplies
authoring previews, accepted SVGs and physical print plans.

## Personal layout presets

The desktop's app-local `card-layout-presets.json` library remains
`schema:"deck.card-layout-presets/1"`. A preset contains its ID, name and a complete resolved
`configuration`; it carries layout and field bindings, not card content. The existing 13 fields
are `mode`, `image_fit`, `background_color`, `text_color`, `border`, `padding_percent`,
`text_align`, `text_scale_percent`, `image_position`, `image_share_percent`,
`text_vertical_align`, `text_offset_x_percent`, and `text_offset_y_percent`.

Existing 13-field files remain readable. Configurations can additionally contain the validated
`blocks` array as a fourteenth field. `mode:"blocks"` requires that array, even when empty;
other modes retain it when supplied. Preset projection removes `canvas_width`, `canvas_height`,
`legacy` and card content, while preserving block order, geometry and field names. Every block
retains the strict contract above, which rejects arbitrary text fields and material references.

The library retains its 64-preset and 128 KiB file bounds. Writes use a short lock and atomic
publication. Invalid entries or damaged files are rejected without replacing the saved file or
the last successfully loaded in-memory library. Applying a preset supplies layout settings to
the selected card IDs through the same saved-draft lifecycle; each target keeps its content,
quantities and material pins.
