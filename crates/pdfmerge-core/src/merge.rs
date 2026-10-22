use std::fs;
use std::path::PathBuf;

use crate::output;
use crate::{MergeError, MergeReport, MergeRequest, Progress};
use lopdf::{Dictionary, Document, Object, ObjectId};

fn inherited_attribute(document: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page_id;
    for _ in 0..1_024 {
        let page = document.get_dictionary(current).ok()?;
        if let Ok(value) = page.get(key) {
            return Some(value.clone());
        }
        match page.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

#[allow(clippy::expect_used)]
fn materialise_page(
    source: &Document,
    source_page_id: ObjectId,
    pages_root: ObjectId,
) -> Dictionary {
    let mut page = source
        .get_dictionary(source_page_id)
        .expect("selected page exists")
        .clone();

    for key in [b"MediaBox".as_slice(), b"CropBox", b"Resources", b"Rotate"] {
        if !page.has(key) {
            if let Some(value) = inherited_attribute(source, source_page_id, key) {
                page.set(key, value);
            }
        }
    }

    page.set(b"Parent", Object::Reference(pages_root));
    page
}

/// Assemble plusieurs PDF en conservant la sélection et l'orientation des pages.
pub fn merge(
    request: &MergeRequest,
    mut progress: impl FnMut(Progress),
) -> Result<MergeReport, MergeError> {
    if request.sources.is_empty() {
        return Err(MergeError::NoSources);
    }

    let output = output::ensure_pdf_extension(request.output.clone());
    let source_paths: Vec<PathBuf> = request
        .sources
        .iter()
        .map(|source| source.path.clone())
        .collect();
    output::validate_output(&output, &source_paths, request.overwrite)?;

    let mut loaded = Vec::with_capacity(request.sources.len());
    for entry in &request.sources {
        let source = Document::load(&entry.path)?;
        if source.is_encrypted() {
            return Err(MergeError::Encrypted(entry.path.clone()));
        }
        let page_count = source.get_pages().len() as u32;
        let selected = entry
            .spec
            .resolve(page_count)
            .map_err(|error| MergeError::Selection {
                path: entry.path.clone(),
                position: error.position,
                message: error.message,
            })?;
        loaded.push((entry.clone(), source, selected));
    }

    let total_pages: usize = loaded.iter().map(|(_, _, pages)| pages.len()).sum();
    progress(Progress {
        processed: 0,
        total: total_pages,
    });

    let mut target = Document::with_version("1.5");
    let pages_root = target.add_object(Dictionary::new());
    let catalog = target.add_object(Dictionary::new());
    let mut selected_refs = Vec::with_capacity(total_pages);
    let mut processed = 0;

    for (_entry, mut source, selected) in loaded {
        let renumber_from = target.max_id + 1;
        source.renumber_objects_with(renumber_from);
        target.max_id = target.max_id.max(source.max_id);

        let page_ids = source.get_pages();
        for page_number in &selected {
            let source_page_id = *page_ids.get(page_number).ok_or(MergeError::Pdf(
                lopdf::Error::PageNumberNotFound(*page_number),
            ))?;
            let page = materialise_page(&source, source_page_id, pages_root);
            let page_id = target.add_object(Object::Dictionary(page));
            selected_refs.push(Object::Reference(page_id));
            processed += 1;
            progress(Progress {
                processed,
                total: total_pages,
            });
        }

        for (id, object) in source.objects {
            target.objects.insert(id, object);
        }
        target.max_id = target.max_id.max(source.max_id);
    }

    let mut pages_dict = Dictionary::new();
    pages_dict.set(b"Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set(b"Kids", Object::Array(selected_refs.clone()));
    pages_dict.set(b"Count", Object::Integer(selected_refs.len() as i64));
    target.set_object(pages_root, Object::Dictionary(pages_dict));

    let mut catalog_dict = Dictionary::new();
    catalog_dict.set(b"Type", Object::Name(b"Catalog".to_vec()));
    catalog_dict.set(b"Pages", Object::Reference(pages_root));
    target.set_object(catalog, Object::Dictionary(catalog_dict));
    target.trailer.set(b"Root", Object::Reference(catalog));

    target.prune_objects();
    target.renumber_objects();

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(MergeError::from)?;
    }
    let temporary = output::temp_output_path(&output);
    let save_result = target.save(&temporary);
    if let Err(error) = save_result {
        let _ = fs::remove_file(&temporary);
        return Err(MergeError::Pdf(lopdf::Error::IO(error)));
    }
    if let Err(error) = output::commit_output(&temporary, &output) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    Ok(MergeReport {
        output,
        page_count: total_pages,
    })
}
