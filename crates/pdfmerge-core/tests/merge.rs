use lopdf::{Document, Object, dictionary};
use pdfmerge_core::{MergeError, MergeReport, PageSpec, merge};
use std::fs;
use tempfile::TempDir;

#[derive(Clone, Copy)]
struct PageSpecInput {
    width: i64,
    height: i64,
    rotate: i64,
}

fn make_direct_pdf(path: &std::path::Path, pages: &[PageSpecInput]) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();
    for page in pages {
        let page_id = doc.new_object_id();
        let media_box = Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(page.width),
            Object::Integer(page.height),
        ]);
        let mut dictionary = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => media_box,
        };
        if page.rotate != 0 {
            dictionary.set(b"Rotate", Object::Integer(page.rotate));
        }
        doc.set_object(page_id, Object::Dictionary(dictionary));
        page_ids.push(Object::Reference(page_id));
    }
    let pages_tree = dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(page_ids),
        "Count" => Object::Integer(pages.len() as i64),
    };
    doc.set_object(pages_id, Object::Dictionary(pages_tree));
    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    doc.set_object(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(path).expect("save source");
}

fn make_inherited_pdf(path: &std::path::Path, width: i64, height: i64) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let intermediate_id = doc.new_object_id();
    let page_id = doc.new_object_id();
    let page = dictionary! {
        "Type" => "Page",
        "Parent" => intermediate_id,
    };
    doc.set_object(page_id, Object::Dictionary(page));
    let intermediate = dictionary! {
        "Type" => "Pages",
        "Parent" => pages_id,
        "Rotate" => Object::Integer(90),
        "MediaBox" => Object::Array(vec![Object::Integer(0), Object::Integer(0), Object::Integer(width), Object::Integer(height)]),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.set_object(intermediate_id, Object::Dictionary(intermediate));
    let pages_tree = dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(vec![Object::Reference(intermediate_id)]),
        "Count" => Object::Integer(1),
    };
    doc.set_object(pages_id, Object::Dictionary(pages_tree));
    let catalog_id = doc.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    doc.set_object(catalog_id, Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(path).expect("save inherited source");
}

fn dimensions(doc: &Document, id: lopdf::ObjectId) -> (i64, i64) {
    let dictionary = doc.get_dictionary(id).expect("page");
    let box_values = dictionary
        .get(b"MediaBox")
        .and_then(Object::as_array)
        .expect("media box");
    let x0 = box_values[0].as_i64().expect("x0");
    let y0 = box_values[1].as_i64().expect("y0");
    let x1 = box_values[2].as_i64().expect("x1");
    let y1 = box_values[3].as_i64().expect("y1");
    (x1 - x0, y1 - y0)
}

fn rotate(doc: &Document, id: lopdf::ObjectId) -> i64 {
    let dictionary = doc.get_dictionary(id).expect("page");
    dictionary
        .get(b"Rotate")
        .and_then(Object::as_i64)
        .unwrap_or(0)
}

#[test]
fn merges_selection_in_order() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.pdf");
    make_direct_pdf(
        &source,
        &[
            PageSpecInput {
                width: 612,
                height: 792,
                rotate: 0,
            },
            PageSpecInput {
                width: 595,
                height: 842,
                rotate: 90,
            },
            PageSpecInput {
                width: 612,
                height: 792,
                rotate: 0,
            },
            PageSpecInput {
                width: 595,
                height: 842,
                rotate: 90,
            },
            PageSpecInput {
                width: 612,
                height: 792,
                rotate: 0,
            },
            PageSpecInput {
                width: 595,
                height: 842,
                rotate: 90,
            },
            PageSpecInput {
                width: 612,
                height: 792,
                rotate: 0,
            },
            PageSpecInput {
                width: 595,
                height: 842,
                rotate: 90,
            },
            PageSpecInput {
                width: 612,
                height: 792,
                rotate: 0,
            },
            PageSpecInput {
                width: 595,
                height: 842,
                rotate: 90,
            },
        ],
    );
    let output = temp.path().join("out.pdf");
    let request = pdfmerge_core::MergeRequest {
        sources: vec![pdfmerge_core::SourceEntry {
            path: source,
            spec: PageSpec::parse("3-6,1,4-$,2,2").expect("valid"),
        }],
        output: output.clone(),
        overwrite: true,
    };
    let mut calls = Vec::new();
    let callback = |call: pdfmerge_core::Progress| calls.push(call);
    let report: MergeReport = merge(&request, callback).expect("merge");
    assert_eq!(report.page_count, 14);
    assert_eq!(report.output, output);

    let merged = Document::load(output).expect("load merged");
    let pages: Vec<_> = merged.get_pages().values().copied().collect();
    assert_eq!(pages.len(), 14);
    assert_eq!(dimensions(&merged, pages[0]), (612, 792));
    assert_eq!(rotate(&merged, pages[0]), 0);
    assert_eq!(dimensions(&merged, pages[9]), (595, 842));
    assert!(!pages.is_empty());
    assert!(calls.first().expect("progress starts at 0").processed == 0);
    let last = calls.last().expect("progress ends");
    assert_eq!(last.processed, 14);
    assert_eq!(last.total, 14);
}

#[test]
fn preserves_inherited_attributes() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.pdf");
    make_inherited_pdf(&source, 612, 792);
    let output = temp.path().join("out.pdf");
    let request = pdfmerge_core::MergeRequest {
        sources: vec![pdfmerge_core::SourceEntry {
            path: source,
            spec: PageSpec::all(),
        }],
        output: output.clone(),
        overwrite: true,
    };
    let callback = |_call: pdfmerge_core::Progress| {};
    merge(&request, callback).expect("merge");
    let merged = Document::load(output).expect("load merged");
    let pages: Vec<_> = merged.get_pages().values().copied().collect();
    assert_eq!(pages.len(), 1);
    assert_eq!(dimensions(&merged, pages[0]), (612, 792));
    assert_eq!(rotate(&merged, pages[0]), 90);
}

#[test]
fn refuses_to_overwrite_source() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.pdf");
    make_direct_pdf(
        &source,
        &[PageSpecInput {
            width: 1,
            height: 1,
            rotate: 0,
        }],
    );
    let request = pdfmerge_core::MergeRequest {
        sources: vec![pdfmerge_core::SourceEntry {
            path: source.clone(),
            spec: PageSpec::all(),
        }],
        output: source,
        overwrite: true,
    };
    let callback = |_call: pdfmerge_core::Progress| {};
    let error = merge(&request, callback).unwrap_err();
    assert!(matches!(error, MergeError::OutputOverwritesSource(_)));
}

#[test]
fn refuses_existing_output_without_overwrite() {
    let temp = TempDir::new().expect("temp dir");
    let source = temp.path().join("source.pdf");
    let output = temp.path().join("out.pdf");
    make_direct_pdf(
        &source,
        &[PageSpecInput {
            width: 1,
            height: 1,
            rotate: 0,
        }],
    );
    fs::write(&output, []).expect("output placeholder");
    let request = pdfmerge_core::MergeRequest {
        sources: vec![pdfmerge_core::SourceEntry {
            path: source,
            spec: PageSpec::all(),
        }],
        output,
        overwrite: false,
    };
    let callback = |_call: pdfmerge_core::Progress| {};
    let error = merge(&request, callback).unwrap_err();
    assert!(matches!(error, MergeError::OutputExists(_)));
}

#[test]
fn reports_no_sources() {
    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("out.pdf");
    let request = pdfmerge_core::MergeRequest {
        sources: Vec::new(),
        output,
        overwrite: true,
    };
    let callback = |_call: pdfmerge_core::Progress| {};
    let error = merge(&request, callback).unwrap_err();
    assert!(matches!(error, MergeError::NoSources));
}
