use super::*;
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use shape_domain::IntentSpec;

struct Fixture {
    root: PathBuf,
    project: ShapeProject,
    request: PictureBookRequest,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("shape-book-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let mut project = ShapeProject::create(root.join("input.shape"), "Book fixture").unwrap();
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&[32, 80, 120, 255].repeat(8), 4, 2, ColorType::Rgba8.into())
            .unwrap();
        let image = project.import_raster_bytes(png, "Art").unwrap();
        let en = project
            .create_text_document(
                "English",
                "[Cover] Moon\n[Title] Night\n[Author] Ada\n[1] Hello",
            )
            .unwrap();
        let zh = project
            .create_text_document("中文", "[封面] 月亮\n[扉页] 夜晚\n[作者] 艾达\n[1] 你好")
            .unwrap();
        let pin = |r: &shape_domain::ArtifactRevision| WorkingInput {
            artifact_id: r.artifact_id,
            revision_id: r.id,
        };
        let request = PictureBookRequest {
            name: "Moon / 月亮".into(),
            default_language: "zh-CN".into(),
            languages: BTreeMap::from([("en".into(), pin(&en)), ("zh-CN".into(), pin(&zh))]),
            pages: vec![
                PictureBookPage::Image {
                    source: pin(&image),
                    print_only: false,
                },
                PictureBookPage::Blank { print_only: false },
                PictureBookPage::Image {
                    source: pin(&image),
                    print_only: false,
                },
                PictureBookPage::Blank { print_only: true },
            ],
            canvas_width: 1024,
            canvas_height: 768,
            font_size: 26,
        };
        Self {
            root,
            project,
            request,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn native_archive_preserves_page_roles_languages_source_bytes_and_independent_repeated_artwork() {
    let f = Fixture::new();
    let snapshot = f.project.snapshot().unwrap();
    let path = f.root.join("book.scproj");
    let receipt = f.project.export_picture_book(&f.request, &path).unwrap();
    assert_eq!(
        receipt.output_blake3,
        shape_domain::ContentDigest::from_bytes(&fs::read(&path).unwrap()).to_string()
    );
    let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let mut read_json = |name: &str| -> Value {
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        serde_json::from_str(&text).unwrap()
    };
    let project = read_json("project.json");
    let origin = read_json("shape-origin.json");
    assert_eq!(project["schema_version"], "20260907.01");
    assert_eq!(project["visible_images"].as_array().unwrap().len(), 4);
    assert_ne!(project["visible_images"][0], project["visible_images"][2]);
    assert_eq!(project["page_settings"]["3"]["print_only"], true);
    assert_eq!(origin["parsed_scripts"]["en"]["page_text"]["2"], "Hello");
    assert_eq!(origin["parsed_scripts"]["zh-CN"]["page_text"]["2"], "你好");
    assert_eq!(origin["parsed_scripts"]["en"]["author"], "Ada");
    // These values use Storybook's native 0..100 effect scale, not opacity fractions.
    for language in ["en", "zh-CN"] {
        for role in ["cover", "title", "inner", "author"] {
            let settings = &project["languages"][language][format!("{role}_text_settings")];
            assert_eq!(settings["readability_mode"], "wash");
            assert_eq!(settings["max_width_percent"], 90);
            assert_eq!(settings["text_effects"]["strength"], 55);
            assert_eq!(settings["text_effects"]["feather"], 80);
            assert_eq!(settings["text_effects"]["color"], "#ffffff");
        }
    }
    // Native page settings are indexed, not keyed by the visible asset path.
    assert!(project["page_settings"].get("blank://shape-3").is_none());
    assert!(
        project["languages"]["en"]["cover_text_settings"]["offset_y"]
            .as_i64()
            .unwrap()
            < -40
    );
    for index in [0, 2] {
        let mut bytes = Vec::new();
        archive
            .by_name(project["visible_images"][index].as_str().unwrap())
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        let PictureBookPage::Image { source, .. } = f.request.pages[index] else {
            panic!("image")
        };
        assert_eq!(
            bytes,
            f.project
                .read_revision_content(source.revision_id)
                .unwrap()
                .bytes
        );
    }
    assert_eq!(f.project.snapshot().unwrap(), snapshot);
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    if let Some(root) = std::env::var_os("SHAPE_BOOK_FIXTURE_DIR") {
        let root = PathBuf::from(root);
        fs::create_dir(&root).unwrap();
        fs::copy(path, root.join("book.scproj")).unwrap();
        fs::write(
            root.join("export.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("project.json"),
            serde_json::to_vec_pretty(&project).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("origin.json"),
            serde_json::to_vec_pretty(&origin).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn changed_input_wrong_media_and_ambiguous_language_roles_publish_nothing() {
    let mut f = Fixture::new();
    let path = f.root.join("book.scproj");
    let original = f.request.clone();
    let body = f
        .project
        .create_text_document("No title", "[Cover] Moon [1] Hello")
        .unwrap();
    f.request.languages.insert(
        "en".into(),
        WorkingInput {
            artifact_id: body.artifact_id,
            revision_id: body.id,
        },
    );
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    assert!(!path.exists());
    f.request = original.clone();
    f.request.pages[0] = PictureBookPage::Image {
        source: f.request.languages["en"],
        print_only: false,
    };
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    f.request = original;
    let input = f.request.languages["en"];
    let candidate = f
        .project
        .propose_text(
            input.artifact_id,
            Some(input.revision_id),
            "[Cover] New [Title] Title [1] New body",
            IntentSpec::new("Revise").unwrap(),
            vec![],
        )
        .unwrap();
    f.project.accept_text(candidate).unwrap();
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    assert!(!path.exists());
    assert!(
        !fs::read_dir(&f.root).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("partial"))
    );
}

#[test]
fn bounds_internal_writes_and_existing_files_are_rejected_without_clobber() {
    let mut f = Fixture::new();
    let path = f.root.join("existing.scproj");
    fs::write(&path, b"keep").unwrap();
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    assert_eq!(fs::read(path).unwrap(), b"keep");
    assert!(
        f.project
            .export_picture_book(&f.request, &f.root.join("input.shape/book.scproj"))
            .is_err()
    );
    let path = f.root.join("book.scproj");
    f.request
        .pages
        .resize(65, PictureBookPage::Blank { print_only: false });
    assert!(f.project.export_picture_book(&f.request, &path).is_err());
    assert!(!path.exists());
}

#[test]
fn book_candidate_is_transient_and_acceptance_survives_reopen_with_exact_materials() {
    let mut f = Fixture::new();
    let before = f.project.snapshot().unwrap();
    let candidate = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    assert_eq!(f.project.snapshot().unwrap(), before);
    let book = f.project.accept_picture_book(candidate).unwrap();
    assert_eq!(
        book.content.media_type,
        shape_domain::PICTURE_BOOK_MEDIA_TYPE
    );
    let project = ShapeProject::open(f.root.join("input.shape")).unwrap();
    assert_eq!(
        project
            .read_picture_book(WorkingInput {
                artifact_id: book.artifact_id,
                revision_id: book.id
            })
            .unwrap(),
        f.request
    );
    let preview = project.preview_picture_book(&f.request).unwrap();
    assert_eq!(preview["parsed_scripts"]["zh-CN"]["page_text"]["2"], "你好");
    assert!(preview["stale_inputs"].as_array().unwrap().is_empty());
}

#[test]
fn stale_book_or_source_rejects_acceptance_but_old_composition_stays_readable() {
    let mut f = Fixture::new();
    let initial = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    let book = f.project.accept_picture_book(initial).unwrap();
    let pin = WorkingInput {
        artifact_id: book.artifact_id,
        revision_id: book.id,
    };
    let pending = f
        .project
        .propose_picture_book(Some(pin), f.request.clone())
        .unwrap();
    let other = f
        .project
        .propose_picture_book(Some(pin), f.request.clone())
        .unwrap();
    let next = f.project.accept_picture_book(other).unwrap();
    assert!(f.project.accept_picture_book(pending).is_err());
    let pending = f
        .project
        .propose_picture_book(
            Some(WorkingInput {
                revision_id: next.id,
                ..pin
            }),
            f.request.clone(),
        )
        .unwrap();
    let source = f.request.languages["en"];
    let update = f
        .project
        .propose_text(
            source.artifact_id,
            Some(source.revision_id),
            "[Cover] Changed\n[Title] Night\n[1] Hello",
            IntentSpec::new("Source update").unwrap(),
            vec![],
        )
        .unwrap();
    let newer = f.project.accept_text(update).unwrap();
    assert!(f.project.accept_picture_book(pending).is_err());
    let saved = f.project.read_picture_book(pin).unwrap();
    let preview = f.project.preview_picture_book(&saved).unwrap();
    assert_eq!(preview["stale_inputs"].as_array().unwrap().len(), 1);
    assert_eq!(preview["parsed_scripts"]["en"]["page_text"]["0"], "Moon");
    f.project
        .export_accepted_picture_book(pin, &f.root.join("old.scproj"))
        .unwrap();
    f.request.languages.insert(
        "en".into(),
        WorkingInput {
            artifact_id: newer.artifact_id,
            revision_id: newer.id,
        },
    );
    let candidate = f
        .project
        .propose_picture_book(
            Some(WorkingInput {
                revision_id: next.id,
                ..pin
            }),
            f.request.clone(),
        )
        .unwrap();
    let newest = f.project.accept_picture_book(candidate).unwrap();
    assert_eq!(newest.parents, vec![next.id]);
}

#[test]
fn editing_script_branches_original_and_missing_translation_never_borrows_text() {
    let mut f = Fixture::new();
    let original = f.request.languages["zh-CN"];
    let edited = f
        .project
        .adopt_picture_book_script(
            "中文修订",
            Some(original),
            "[封面] 月亮\n[扉页] 夜晚\n[作者] 艾达",
        )
        .unwrap();
    assert_ne!(edited.artifact_id, original.artifact_id);
    assert_eq!(
        f.project
            .read_accepted(original.artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        original.revision_id
    );
    f.request.languages.insert(
        "zh-CN".into(),
        WorkingInput {
            artifact_id: edited.artifact_id,
            revision_id: edited.id,
        },
    );
    let preview = f.project.preview_picture_book(&f.request).unwrap();
    assert_eq!(
        preview["missing_text"],
        json!([{"language":"zh-CN","page":2}])
    );
    assert!(
        preview["parsed_scripts"]["zh-CN"]["page_text"]
            .get("2")
            .is_none()
    );
    let snapshot = f.project.snapshot().unwrap();
    assert!(
        f.project
            .adopt_picture_book_script("Bad", None, "[Cover] A\n[Cover] B")
            .is_err()
    );
    assert_eq!(f.project.snapshot().unwrap(), snapshot);
}

#[test]
fn review_identity_binds_composition_target_and_source_revisions() {
    let mut f = Fixture::new();
    let first = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    let again = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    assert_eq!(first.review_digest(), again.review_digest());
    f.request.font_size = 30;
    let changed = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    assert_ne!(first.review_digest(), changed.review_digest());
    let accepted = f.project.accept_picture_book(first).unwrap();
    let update = f
        .project
        .propose_picture_book(
            Some(WorkingInput {
                artifact_id: accepted.artifact_id,
                revision_id: accepted.id,
            }),
            f.request.clone(),
        )
        .unwrap();
    assert_ne!(changed.review_digest(), update.review_digest());
}

#[test]
fn returned_artwork_is_explicitly_bound_then_frozen_in_each_book_version() {
    use shape_domain::{ContentDigest, MaterialEditor};
    let mut f = Fixture::new();
    let source = f.request.pages[0].source().unwrap();
    let candidate = f
        .project
        .propose_picture_book(None, f.request.clone())
        .unwrap();
    let old = f.project.accept_picture_book(candidate).unwrap();
    let old_pin = WorkingInput {
        artifact_id: old.artifact_id,
        revision_id: old.id,
    };
    let ticket = f
        .project
        .create_material_edit(source, "Shadow derived art", MaterialEditor::Shadow)
        .unwrap();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            &[220, 130, 20, 255].repeat(8),
            4,
            2,
            ColorType::Rgba8.into(),
        )
        .unwrap();
    let digest = ContentDigest::from_bytes(&bytes);
    let returned = f
        .project
        .propose_material_return(&ticket, bytes.clone(), digest)
        .unwrap();
    assert_eq!(f.project.read_picture_book(old_pin).unwrap(), f.request);
    let edited = f.project.accept_material_return(returned).unwrap();
    assert_eq!(
        f.project
            .read_accepted(source.artifact_id)
            .unwrap()
            .unwrap()
            .revision
            .id,
        source.revision_id
    );
    f.request.pages[2] = PictureBookPage::Image {
        source: WorkingInput {
            artifact_id: edited.artifact_id,
            revision_id: edited.id,
        },
        print_only: false,
    };
    let reviewed = f
        .project
        .propose_picture_book(Some(old_pin), f.request.clone())
        .unwrap();
    let new = f.project.accept_picture_book(reviewed).unwrap();
    let reopened = ShapeProject::open(f.root.join("input.shape")).unwrap();
    assert_eq!(
        reopened.read_picture_book(old_pin).unwrap().pages[2].source(),
        Some(source)
    );
    let new_pin = WorkingInput {
        artifact_id: new.artifact_id,
        revision_id: new.id,
    };
    assert_eq!(reopened.read_picture_book(new_pin).unwrap(), f.request);
    let path = f.root.join("returned-art.scproj");
    reopened
        .export_accepted_picture_book(new_pin, &path)
        .unwrap();
    let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    let state: Value = serde_json::from_reader(archive.by_name("project.json").unwrap()).unwrap();
    let mut exported = Vec::new();
    archive
        .by_name(state["visible_images"][2].as_str().unwrap())
        .unwrap()
        .read_to_end(&mut exported)
        .unwrap();
    assert_eq!(exported, bytes);
}
