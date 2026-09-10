//! The revision asynchronous Annotators compare, independent of edit history.

use quill_engine::document::Document;

#[test]
fn insert_and_delete_step_generation_without_changing_document_equality() {
    let mut edited = Document::untitled();
    assert_eq!(edited.generation(), 0);
    edited.insert(0, "Alice reads.");
    assert_eq!(edited.generation(), 1);
    edited.insert(5, " quickly");
    assert_eq!(edited.generation(), 2);
    edited.delete(5..13);
    assert_eq!(edited.generation(), 3);

    let mut fresh = Document::untitled();
    fresh.insert(0, "Alice reads.");
    assert_eq!(fresh.generation(), 1);
    assert_eq!(edited, fresh);
}

#[test]
fn reload_invalidates_results_from_before_the_file_changed() {
    let mut document = Document::untitled();
    document.insert(0, "Alice reads.");
    document.reload("Alice sleeps.".into());
    assert_eq!(document.generation(), 2);
    let mut fresh = Document::untitled();
    fresh.insert(0, "Alice sleeps.");
    assert_eq!(document, fresh);
    document.insert(document.text().len(), " Again.");
    assert_eq!(document.generation(), 3);
}
