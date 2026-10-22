use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pdfmerge_core::output;
use pdfmerge_core::sources;
use pdfmerge_core::{PageSpec, SourceEntry, SpecError};
use slint::{ComponentHandle, Model, VecModel};

#[derive(Clone)]
struct SourceState {
    path: PathBuf,
    page_count: u32,
    spec_input: String,
    spec: Result<PageSpec, SpecError>,
    status: String,
    error: bool,
}

#[derive(Clone)]
struct OutputState {
    dir: String,
    name: String,
    dir_modified: bool,
    name_modified: bool,
}

pub fn run() {
    let Ok(app) = crate::AppWindow::new() else {
        eprintln!("impossible de creer l'interface PDFMerge");
        return;
    };

    let app_weak = app.as_weak();
    let weak = app_weak.clone();
    let state = Arc::new(Mutex::new(Vec::<SourceState>::new()));
    let output_state = Arc::new(Mutex::new(OutputState {
        dir: String::new(),
        name: output::default_output_name(),
        dir_modified: false,
        name_modified: false,
    }));

    let merging = Arc::new(AtomicBool::new(false));
    let progress_processed = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(0));

    let weak_add_files = weak.clone();
    let state_add_files = Arc::clone(&state);
    let output_add_files = Arc::clone(&output_state);
    app.on_add_files(move || {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_files()
        {
            enqueue_sources(
                weak_add_files.clone(),
                Arc::clone(&state_add_files),
                Arc::clone(&output_add_files),
                paths,
            );
        }
    });

    let weak_add_directory = weak.clone();
    let state_add_directory = Arc::clone(&state);
    let output_add_directory = Arc::clone(&output_state);
    app.on_add_directory(move || {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            enqueue_sources(
                weak_add_directory.clone(),
                Arc::clone(&state_add_directory),
                Arc::clone(&output_add_directory),
                vec![folder],
            );
        }
    });

    let weak_remove = weak.clone();
    let state_remove = Arc::clone(&state);
    let output_remove = Arc::clone(&output_state);
    app.on_remove_source(move |index| {
        let Some(app) = weak_remove.upgrade() else {
            return;
        };
        let index = index as usize;
        {
            let mut lock = state_remove
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if index < lock.len() {
                lock.remove(index);
            }
        }
        rebuild_ui(&app, &state_remove, &output_remove);
    });

    let weak_up = weak.clone();
    let state_up = Arc::clone(&state);
    let output_up = Arc::clone(&output_state);
    app.on_move_up(move |index| {
        let Some(app) = weak_up.upgrade() else {
            return;
        };
        let index = index as usize;
        {
            let mut lock = state_up
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if index > 0 && index < lock.len() {
                lock.swap(index, index - 1);
            }
        }
        rebuild_ui(&app, &state_up, &output_up);
    });

    let weak_down = weak.clone();
    let state_down = Arc::clone(&state);
    let output_down = Arc::clone(&output_state);
    app.on_move_down(move |index| {
        let Some(app) = weak_down.upgrade() else {
            return;
        };
        let index = index as usize;
        {
            let mut lock = state_down
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if index + 1 < lock.len() {
                lock.swap(index, index + 1);
            }
        }
        rebuild_ui(&app, &state_down, &output_down);
    });

    let weak_pages = weak.clone();
    let state_pages = Arc::clone(&state);
    let output_pages = Arc::clone(&output_state);
    app.on_pages_edited(move |index, value| {
        let Some(app) = weak_pages.upgrade() else {
            return;
        };
        let index = index as usize;
        let typed = value.to_string();
        let mut updated_row = None;
        {
            let mut lock = state_pages
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if index < lock.len() {
                lock[index].spec_input = typed.clone();
                validate_source(&mut lock[index]);
                updated_row = Some(crate::SourceRow {
                    path: lock[index].path.to_string_lossy().to_string().into(),
                    page_count: lock[index].page_count as i32,
                    pages: typed.clone().into(),
                    status: lock[index].status.clone().into(),
                    status_error: lock[index].error,
                });
            }
        }
        let can_merge = compute_can_merge(&state_pages, &output_pages);
        app.set_can_merge(can_merge);
        app.set_status_text(status_for_all(&state_pages).into());
        if let Some(row) = updated_row {
            let model = app.get_source_model();
            model.set_row_data(index, row);
        }
    });

    let weak_output_dir = weak.clone();
    let output_dir_state = Arc::clone(&output_state);
    let merge_can_merge_output_dir = Arc::clone(&state);
    app.on_output_dir_changed(move |value| {
        if let Some(app) = weak_output_dir.upgrade() {
            let mut guard = output_dir_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.dir = value.to_string();
            guard.dir_modified = true;
            let can_merge = compute_can_merge(&merge_can_merge_output_dir, &output_dir_state);
            drop(guard);
            app.set_can_merge(can_merge);
        }
    });

    let weak_output_name = weak.clone();
    let output_name_state = Arc::clone(&output_state);
    let merge_can_merge_output_name = Arc::clone(&state);
    app.on_output_name_changed(move |value| {
        if let Some(app) = weak_output_name.upgrade() {
            let mut guard = output_name_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.name = value.to_string();
            guard.name_modified = true;
            let can_merge = compute_can_merge(&merge_can_merge_output_name, &output_name_state);
            drop(guard);
            app.set_can_merge(can_merge);
        }
    });

    let weak_merge = weak.clone();
    let state_merge = Arc::clone(&state);
    let output_merge = Arc::clone(&output_state);
    let merging_merge = Arc::clone(&merging);
    let progress_processed_merge = Arc::clone(&progress_processed);
    let progress_total_merge = Arc::clone(&progress_total);
    app.on_merge_clicked(move || {
        let Some(app) = weak_merge.upgrade() else {
            return;
        };
        if merging_merge.load(Ordering::Relaxed) {
            return;
        }

        let snapshot = state_merge
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if snapshot.is_empty() {
            app.set_status_text("Ajoutez au moins un PDF.".into());
            return;
        }
        if snapshot.iter().any(|item| item.error) {
            app.set_status_text("Corrigez les selections en rouge.".into());
            return;
        }

        let output_guard = output_merge
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let output_dir = output_guard.dir.clone();
        let output_name = output_guard.name.clone();
        drop(output_guard);

        let first = &snapshot[0].path;
        let output_dir = if output_dir.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(output_dir))
        };
        let output_path = if output_name.trim().is_empty() && output_dir.is_none() {
            output::default_output_path(first, None)
        } else {
            output::resolve_output_path(first, output_dir.as_deref(), Some(&output_name))
        };

        let mut request_sources = Vec::new();
        for item in &snapshot {
            let spec = match &item.spec {
                Ok(spec) => spec.clone(),
                Err(_) => {
                    app.set_status_text("Selection invalide.".into());
                    return;
                }
            };
            request_sources.push(SourceEntry {
                path: item.path.clone(),
                spec,
            });
        }

        let source_paths: Vec<PathBuf> = request_sources
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        if let Err(error) = output::validate_output(&output_path, &source_paths, false) {
            app.set_status_text(error.to_string().into());
            return;
        }

        let request = pdfmerge_core::MergeRequest {
            sources: request_sources,
            output: output_path.clone(),
            overwrite: false,
        };

        merging_merge.store(true, Ordering::Relaxed);
        progress_processed_merge.store(0, Ordering::Relaxed);
        progress_total_merge.store(0, Ordering::Relaxed);
        app.set_status_text("Fusion en cours…".into());
        app.set_can_merge(false);

        let state_thread = Arc::clone(&state_merge);
        let output_thread = Arc::clone(&output_merge);
        let merging_thread = Arc::clone(&merging_merge);
        let processed_thread = Arc::clone(&progress_processed_merge);
        let total_thread = Arc::clone(&progress_total_merge);
        let weak_thread = weak.clone();
        let last_publish = Arc::new(Mutex::new(Instant::now()));
        thread::spawn(move || {
            let result = pdfmerge_core::merge(&request, |progress| {
                let processed = processed_thread.clone();
                let total = total_thread.clone();
                processed.store(progress.processed, Ordering::Relaxed);
                total.store(progress.total, Ordering::Relaxed);

                let should_publish = progress.total == 0
                    || progress.processed == progress.total
                    || progress.processed == 0
                    || {
                        let mut last = last_publish
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if last.elapsed() >= Duration::from_millis(100) {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    };
                if !should_publish {
                    return;
                }

                let weak = weak_thread.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_progress_processed(processed.load(Ordering::Relaxed) as i32);
                        app.set_progress_total(total.load(Ordering::Relaxed) as i32);
                    }
                });
            });

            merging_thread.store(false, Ordering::Relaxed);
            let total_value = match &result {
                Ok(report) => report.page_count,
                Err(_) => total_thread.load(Ordering::Relaxed),
            };
            total_thread.store(total_value, Ordering::Relaxed);
            processed_thread.store(
                if result.is_ok() {
                    total_value
                } else {
                    processed_thread.load(Ordering::Relaxed)
                },
                Ordering::Relaxed,
            );

            let weak = weak_thread.clone();
            let state = Arc::clone(&state_thread);
            let output = Arc::clone(&output_thread);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    let status = match &result {
                        Ok(report) => format!(
                            "Fusion terminee : {} ({} pages)",
                            report.output.display(),
                            report.page_count
                        ),
                        Err(error) => error.to_string(),
                    };
                    app.set_status_text(status.into());
                    app.set_progress_processed(processed_thread.load(Ordering::Relaxed) as i32);
                    app.set_progress_total(total_thread.load(Ordering::Relaxed) as i32);
                    app.set_can_merge(compute_can_merge(&state, &output));
                }
            });
        });
    });

    if let Some(app) = app.as_weak().upgrade() {
        rebuild_ui(&app, &state, &output_state);
    }
    let _ = app.run();
}

fn enqueue_sources(
    weak: slint::Weak<crate::AppWindow>,
    state: Arc<Mutex<Vec<SourceState>>>,
    output_state: Arc<Mutex<OutputState>>,
    paths: Vec<PathBuf>,
) {
    thread::spawn(move || {
        let rows = collect_sources(paths);
        let state_event = Arc::clone(&state);
        let output_event = Arc::clone(&output_state);
        let weak_event = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak_event.upgrade() {
                let was_empty = {
                    let lock = state_event
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    lock.is_empty()
                };
                {
                    let mut lock = state_event
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    lock.extend(rows);
                }
                if was_empty {
                    let lock = state_event
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    if !lock.is_empty() {
                        let mut output = output_event
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if !output.dir_modified {
                            output.dir = output::default_output_dir(&lock[0].path)
                                .to_string_lossy()
                                .to_string();
                        }
                        if !output.name_modified {
                            output.name = output::default_output_name();
                        }
                        drop(output);
                    }
                    drop(lock);
                }
                rebuild_ui(&app, &state_event, &output_event);
            }
        });
    });
}

fn collect_sources(paths: Vec<PathBuf>) -> Vec<SourceState> {
    let mut result = Vec::new();
    for path in paths {
        match sources::expand_input(&path) {
            Ok(entries) => {
                for entry in entries {
                    result.push(create_source_state(entry));
                }
            }
            Err(error) => {
                result.push(SourceState {
                    path,
                    page_count: 0,
                    spec_input: String::new(),
                    spec: Err(SpecError::new(1, error.to_string())),
                    status: error.to_string(),
                    error: true,
                });
            }
        }
    }
    result
}

fn create_source_state(entry: pdfmerge_core::SourceEntry) -> SourceState {
    let path = entry.path.clone();
    match lopdf::Document::load(&path) {
        Ok(document) => {
            if document.is_encrypted() {
                SourceState {
                    path,
                    page_count: 0,
                    spec_input: String::new(),
                    spec: Err(SpecError::new(1, "document chiffre".to_owned())),
                    status: "PDF chiffre non pris en charge".to_owned(),
                    error: true,
                }
            } else {
                let page_count = document.get_pages().len() as u32;
                let mut item = SourceState {
                    path,
                    page_count,
                    spec_input: String::new(),
                    spec: Ok(PageSpec::all()),
                    status: String::new(),
                    error: false,
                };
                validate_source(&mut item);
                item
            }
        }
        Err(error) => SourceState {
            path,
            page_count: 0,
            spec_input: String::new(),
            spec: Err(SpecError::new(1, error.to_string())),
            status: format!("Erreur : {error}"),
            error: true,
        },
    }
}

fn validate_source(item: &mut SourceState) {
    if item.page_count == 0 {
        if !item.error {
            item.status = "Aucune page".to_owned();
        }
        return;
    }
    if item.spec_input.trim().is_empty() {
        item.spec = Ok(PageSpec::all());
        item.status = format!("Toutes ({} pages)", item.page_count);
        item.error = false;
        return;
    }
    match PageSpec::parse(&item.spec_input) {
        Ok(spec) => match spec.resolve(item.page_count) {
            Ok(pages) => {
                item.spec = Ok(spec);
                item.status = format!("{} page(s)", pages.len());
                item.error = false;
            }
            Err(error) => {
                item.spec = Err(error.clone());
                if incomplete_selection(&item.spec_input) {
                    item.status = "Saisie en cours…".to_owned();
                    item.error = false;
                } else {
                    item.status = format!("Erreur pos. {} : {}", error.position, error.message);
                    item.error = true;
                }
            }
        },
        Err(error) => {
            item.spec = Err(error.clone());
            if incomplete_selection(&item.spec_input) {
                item.status = "Saisie en cours…".to_owned();
                item.error = false;
            } else {
                item.status = format!("Erreur pos. {} : {}", error.position, error.message);
                item.error = true;
            }
        }
    }
}

fn incomplete_selection(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.is_empty()
        || trimmed.ends_with('-')
        || trimmed.ends_with(',')
        || trimmed.ends_with("..")
}

fn status_for_all(state: &Arc<Mutex<Vec<SourceState>>>) -> String {
    let lock = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if lock.is_empty() {
        "Ajoutez des PDFs.".to_owned()
    } else if lock.iter().any(|item| item.error) {
        "Corrigez les selections en rouge.".to_owned()
    } else {
        format!("{} source(s) prete(s).", lock.len())
    }
}

fn compute_can_merge(
    state: &Arc<Mutex<Vec<SourceState>>>,
    output_state: &Arc<Mutex<OutputState>>,
) -> bool {
    let lock = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if lock.is_empty() || lock.iter().any(|item| item.error || item.spec.is_err()) {
        return false;
    }
    drop(lock);
    let output = output_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    !output.dir.trim().is_empty() && !output.name.trim().is_empty()
}

fn rebuild_ui(
    app: &crate::AppWindow,
    state: &Arc<Mutex<Vec<SourceState>>>,
    output_state: &Arc<Mutex<OutputState>>,
) {
    let model = {
        let lock = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let model = VecModel::default();
        for item in &*lock {
            model.push(crate::SourceRow {
                path: item.path.to_string_lossy().to_string().into(),
                page_count: item.page_count as i32,
                pages: item.spec_input.clone().into(),
                status: item.status.clone().into(),
                status_error: item.error,
            });
        }
        std::rc::Rc::new(model)
    };
    app.set_source_model(model.into());

    let output = output_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    app.set_output_dir_str(output.dir.clone().into());
    app.set_output_name_str(output.name.clone().into());
    app.set_status_text(status_for_all(state).into());
    app.set_can_merge(compute_can_merge(state, output_state));
}
