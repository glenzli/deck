use super::super::tests::{Fixture, png};
use super::*;

#[test]
fn handoff_is_no_clobber_and_cannot_write_inside_project() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    assert!(
        f.project
            .export_material_edit(&ticket, &f.root.join("forbidden"))
            .is_err()
    );
    let destination = f.root.with_extension("handoff");
    let first = f
        .project
        .export_material_edit(&ticket, &destination)
        .unwrap();
    assert_eq!(fs::read(&first.input_path).unwrap(), png(10));
    assert_eq!(
        ShapeProject::read_material_handoff(&destination.join("handoff.json"))
            .unwrap()
            .ticket,
        ticket
    );
    assert!(
        f.project
            .export_material_edit(&ticket, &destination)
            .is_err()
    );
    assert_eq!(fs::read(&first.input_path).unwrap(), png(10));
    fs::remove_dir_all(destination).unwrap();
}

#[test]
fn bounded_reads_reject_directories_and_symlinks_and_acceptance_pins_reviewed_bytes() {
    let mut f = Fixture::image();
    let ticket = f.stage();
    assert!(
        f.project
            .review_material_return_file(&ticket, &f.root)
            .is_err()
    );
    let file = f.root.join("return.png");
    fs::write(&file, png(77)).unwrap();
    let reviewed = f
        .project
        .review_material_return_file(&ticket, &file)
        .unwrap();
    fs::write(&file, png(78)).unwrap();
    assert!(
        f.project
            .propose_material_return_file(&ticket, &file, ContentDigest::from_bytes(&png(77)))
            .is_err()
    );
    f.project.accept_material_return(reviewed).unwrap();
    assert_eq!(
        f.project
            .read_accepted(ticket.output_artifact_id)
            .unwrap()
            .unwrap()
            .bytes,
        png(77)
    );
    #[cfg(unix)]
    {
        let link = f.root.join("link.png");
        std::os::unix::fs::symlink(file, &link).unwrap();
        assert!(read_regular(&link, 128 * 1024).is_err());
    }
}
