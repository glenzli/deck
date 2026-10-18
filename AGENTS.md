# Deck development conventions

Read DEV_SKELETON.md before changing ownership, persistence or integration boundaries.

- Deck owns card authoring and printing. Shape owns general material creation and transformation;
  Shadow owns specialist image editing. Keep each application's writable project state separate.
- Reuse the pinned Shape CardKit, material and print implementations before introducing a new
  renderer or persistence format. Preserve MIT attribution and exact dependency provenance.
- Do not depend on a developer's absolute Shape checkout. A clean clone must locate or fetch the
  locked source dependency using repository configuration.
- Apply maintain-source-cohesion when moving responsibilities. Keep models, lifecycle, validation
  and adjacent tests together; application entry points assemble those owners.
- Preserve old card IDs, accepted material versions, flexible/legacy layouts and unaccepted drafts.
  Never silently remove other content while opening an old mixed Shape project.
- AI and desktop actions use the same services. Requests name the exact target and expected
  version; execution returns a reviewable result and does not silently replace an accepted image.
- Shadow runs as an independent process through its request/result contract. Do not import its
  editor implementation or write its catalog.
- Canonical UI strings are English with finished Simplified Chinese translations.
- Build products and test projects stay outside the checkout, normally in ../.deck-local-build
  or a task-private temporary directory. Build native tests before running them.
- Use managed Dev Mesh claims and publication when agents share this repository. Preserve peer
  changes, and designate one native build steward.
