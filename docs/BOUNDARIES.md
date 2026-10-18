# Application boundaries

Deck owns the card project: card content, layout, quantities, selected materials and print output.
Shape owns general AI creation and material transformation. Shadow owns specialist image edits.
Storybook Co-Editor continues to own books. Existing Shape speech remains unchanged.

A user may work directly in Deck. Shape may also show a Deck document or a bounded Deck operation.
AI must be able to discover real supported actions, inspect a named project and its version,
submit an explicit request, and read the resulting state or files without driving the UI.

An integration carries exact source/target identity, a request identity, progress or terminal
status, and verifiable output bytes. Launching an application is not proof that an operation
completed. A returned image is reviewable until explicitly selected for the intended card.
A late return must not update another card, project or newer material version.

The first release reuses existing Shape implementation owners with a fixed dependency revision.
It does not introduce a general plugin SDK. The first real card/Shape/Shadow consumer defines the
smallest useful interoperability contract. Cross-application projects do not share a writable
database as an integration mechanism.

Old Shape CardKit projects are compatibility inputs. Complete migration must account for images,
drafts, IDs, revisions and other content in a mixed project; a material ZIP is only an export.
