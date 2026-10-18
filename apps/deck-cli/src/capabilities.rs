//! Discoverable request examples for real commands, not a workflow dispatcher.
use serde_json::{Value, json};
pub fn report() -> Value {
    json!({
        "schema":"deck.capabilities/1","shape_revision":crate::project::SHAPE_REVISION,
        "commands":{
            "create":{"argv":["deck-cli","create","<new.deck>","<request.json>"],
                "request":{"name":"My Deck","composition":{"name":"My Deck","layout":{"mode":"auto"},"cards":[{"id":"card-1","rules":"Moon · 月"}]}},
                "effect":"creates project and saved draft; no accepted kit","returns":"result.project_id, result.draft"},
            "status":{"argv":["deck-cli","status","<project.deck>"],"read_only":true,
                "returns":"result.open_guard, result.kits[].source and draft requests/versions/blockers"},
            "card-kit":{"argv":["deck-cli","card-kit","<project.deck>","<request.json>"],
                "actions":["draft_new","draft_list","draft_load","draft_save","draft_discard","preview","accept","read","export","print_plan"],
                "requests":{
                    "load":{"action":"draft_load","artifact_id":"<kit UUID>"},
                    "save":{"action":"draft_save","artifact_id":"<kit UUID>","draft_digest":"<last loaded/saved digest>","composition":"<patch the complete loaded composition, preserving other fields>"},
                    "check":{"action":"preview","artifact_id":"<kit UUID>","draft_digest":"<current draft digest>"},
                    "accept":{"action":"accept","artifact_id":"<kit UUID>","draft_digest":"<current draft digest>","review_digest":"<reviewed exact preview result.review_digest>"},
                    "read":{"action":"read","source":{"artifact_id":"<kit UUID>","revision_id":"<accepted revision UUID>"}},
                    "export":{"action":"export","source":{"artifact_id":"<kit UUID>","revision_id":"<accepted revision UUID>"},"destination":"<new ZIP path>"},
                    "print_plan":{"action":"print_plan","source":{"artifact_id":"<kit UUID>","revision_id":"<accepted revision UUID>"}}}},
            "import-raster":{"argv":["deck-cli","import-raster","<project.deck>","<request.json>"],
                "request":{"name":"Finished face","path":"<PNG or JPEG path>"},"returns":"result.source (new local accepted material pin); does not bind or accept a card"},
            "import-transfer":{"argv":["deck-cli","import-transfer","<new.deck>","<manifest.json>","--expected-blake3","<exact manifest hash>"],
                "schema":"deck.card-transfer/1","effect":"atomic new project; exact retry recovers same IDs; foreign pins become local pins"},
            "import-material-return":{"argv":["deck-cli","import-material-return","<project.deck>","<request.json>"],
                "request":{"task_path":"<original deck.material-task/1 JSON>","result_path":"<Shape terminal result JSON>","expected_request_blake3":"<captured exact task hash>","name":"Returned artwork"},
                "effect":"verify original card snapshot and returned PNG; import a local material, without binding a card or accepting a kit"},
            "print-pdf":{"argv":["deck-cli","print-pdf","<project.deck>","<request.json>","--desktop","<deck-desktop executable>"],
                "request_schema":"deck.pdf-export/1",
                "request":{"schema":"deck.pdf-export/1","project":"<absolute project path>","source":{"artifact_id":"<accepted kit UUID>","revision_id":"<accepted kit revision UUID>"},"options":{"paper":"a4"},"destination":"<absolute new PDF path>"},
                "source_required":"accepted kit pin returned by explicit accept; draft artifact/digest alone cannot be printed",
                "effect":"same native Qt PDF writer, followed by independent CLI byte/hash readback"},
            "hash-file":{"argv":["deck-cli","hash-file","<regular file>"],"read_only":true,"returns":"result.path, result.byte_length, result.blake3"}
        },
        "flow":["create or import-transfer","status; use draft load request","patch loaded composition; draft_save with its digest","preview; inspect ready_to_export, card issues and returned review_digest","explicit accept","read/export/print_plan with returned exact source"],
        "handoff":{"gui_capabilities_argv":["<Deck desktop executable>","--handoff-capabilities"],"open_argv":["<Deck desktop executable>","--project","<absolute Deck project>","--kit","<Deck kit UUID>"],
            "external_edit_argv":["<Deck desktop executable>","--external-edit","<deck.external-edit/1 request.json>","--result","<new result.json>"],
            "shape_capabilities_argv":["<Shape executable>","--handoff-capabilities"],"shape_material_argv":["<Shape executable>","--deck-material-task","<deck.material-task/1 task.json>","--result","<new result.json>"],
            "human_review_required":true,"headless_image_generation":false},
        "limits":{"cards":64,"materials":64,"transfer_material_bytes":134_217_728,"print_copies":4096,"print_pages":256},
        "boundaries":{"accepted_history":"immutable","acceptance":"explicit_action","incomplete_checklists":"save and accept permitted; export readiness is checked separately",
            "print_plan_is_pdf":false,"ai_image_generation":"through Shape or imported material; no borrowed Shape credentials",
            "runtime_checked":false,"other_session_candidates":"not_observed","mixed_shape_projects":"complete selected-kit transfer required"}
    })
}
