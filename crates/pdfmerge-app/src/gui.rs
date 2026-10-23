use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pdfmerge_core::output;
use pdfmerge_core::sources;
use pdfmerge_core::{MergeError, PageSpec, SourceEntry, SpecError};
#[cfg(target_os = "linux")]
use slint::winit_030::winit::platform::startup_notify::WindowAttributesExtStartupNotify;
#[cfg(target_os = "linux")]
use slint::winit_030::winit::window::ActivationToken;
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
    timestamp: bool,
}

fn source_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn source_folder(path: &std::path::Path) -> String {
    path.parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_owned())
}

fn source_panel_title(source_count: usize) -> String {
    if source_count == 0 {
        "Sources · 0 fichier".to_owned()
    } else if source_count == 1 {
        "Sources · 1 fichier".to_owned()
    } else {
        format!("Sources · {source_count} fichiers").to_owned()
    }
}

fn status_level_for_all(state: &Arc<Mutex<Vec<SourceState>>>) -> String {
    let lock = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if lock.is_empty() || lock.iter().all(|item| !item.error) {
        "neutral".to_owned()
    } else {
        "error".to_owned()
    }
}

fn page_word_for(count: usize) -> &'static str {
    if count > 1 { "pages" } else { "page" }
}

fn open_location(location: &PathBuf) -> Result<(), String> {
    opener::open(location)
        .map_err(|error| format!("Impossible d'ouvrir le fichier ou le dossier : {error}"))
}

fn refresh_derived_ui(
    app: &crate::AppWindow,
    state: &Arc<Mutex<Vec<SourceState>>>,
    output_state: &Arc<Mutex<OutputState>>,
) {
    let lock = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let source_count = lock.len();
    let total_pages: u32 = if lock.is_empty() {
        0
    } else {
        lock.iter()
            .filter_map(|item| {
                item.spec
                    .as_ref()
                    .ok()
                    .and_then(|spec| spec.resolve(item.page_count).ok())
                    .map(|pages| pages.len() as u32)
            })
            .sum()
    };
    let status = if lock.is_empty() {
        "Aucune source · Ajoutez au moins un PDF.".to_owned()
    } else if lock.iter().any(|item| item.error) {
        format!(
            "Corrigez les sélections en rouge. · {} sources · {} {}",
            source_count,
            total_pages,
            page_word_for(total_pages as usize)
        )
    } else if source_count == 1 {
        format!(
            "1 source · {} {} · prête",
            total_pages,
            page_word_for(total_pages as usize)
        )
    } else {
        format!(
            "{} sources · {} {} · prêtes",
            source_count,
            total_pages,
            page_word_for(total_pages as usize)
        )
    };
    let level = if lock.is_empty() || !lock.iter().any(|item| item.error) {
        "neutral".to_owned()
    } else {
        "error".to_owned()
    };
    drop(lock);
    let source_count = source_count as i32;
    let output = output_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = output.dir.clone();
    let name = output.name.clone();
    drop(output);

    app.set_source_count(source_count);
    app.set_source_panel_title(source_panel_title(source_count as usize).into());
    app.set_status_text(status.into());
    app.set_status_level(level.into());
    app.set_output_dir_str(dir.into());
    app.set_output_name_str(name.into());
    app.set_can_merge(compute_can_merge(state, output_state));
}

pub fn run() {
    #[cfg(target_os = "linux")]
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(|attributes| {
            match std::env::var("XDG_ACTIVATION_TOKEN")
                .or_else(|_| std::env::var("DESKTOP_STARTUP_ID"))
            {
                Ok(token) => attributes.with_activation_token(ActivationToken::from_raw(token)),
                Err(_) => attributes,
            }
        });

    #[cfg(not(target_os = "linux"))]
    let selector = slint::BackendSelector::new().backend_name("winit".into());

    let selected = selector.select();
    if let Err(error) = selected {
        eprintln!("sélection du backend Slint impossible : {error}");
    }

    if let Err(error) = slint::set_xdg_app_id("io.github.Franck-Demongin.PDFMerge") {
        eprintln!("set_xdg_app_id a échoué : {error}");
    }

    let Ok(app) = crate::AppWindow::new() else {
        eprintln!("impossible de créer l'interface PDFMerge");
        return;
    };

    let app_weak = app.as_weak();
    let weak = app_weak.clone();
    let state = Arc::new(Mutex::new(Vec::<SourceState>::new()));
    let output_state = Arc::new(Mutex::new(OutputState {
        dir: String::new(),
        name: "fusion".to_owned(),
        dir_modified: false,
        name_modified: false,
        timestamp: false,
    }));
    let result_output = Arc::new(Mutex::new(None::<PathBuf>));
    let result_output_for_page = Arc::clone(&result_output);
    let result_output_for_output_dir = Arc::clone(&result_output);
    let result_output_for_output_name = Arc::clone(&result_output);

    let merging = Arc::new(AtomicBool::new(false));
    let progress_processed = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(0));

    let weak_add_files = weak.clone();
    let state_add_files = Arc::clone(&state);
    let output_add_files = Arc::clone(&output_state);
    app.on_add_files(move || {
        let weak = weak_add_files.clone();
        let state = Arc::clone(&state_add_files);
        let output = Arc::clone(&output_add_files);
        std::thread::spawn(move || {
            let Some(paths) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .pick_files()
            else {
                return;
            };
            enqueue_sources(weak, state, output, paths);
        });
    });

    let weak_add_directory = weak.clone();
    let state_add_directory = Arc::clone(&state);
    let output_add_directory = Arc::clone(&output_state);
    app.on_add_directory(move || {
        let weak = weak_add_directory.clone();
        let state = Arc::clone(&state_add_directory);
        let output = Arc::clone(&output_add_directory);
        std::thread::spawn(move || {
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            enqueue_sources(weak, state, output, vec![folder]);
        });
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

    let weak_move = weak.clone();
    let state_move = Arc::clone(&state);
    let output_move = Arc::clone(&output_state);
    app.on_move_source(move |from, to| {
        let Some(app) = weak_move.upgrade() else {
            return;
        };
        let from = from as usize;
        let to = to as usize;
        {
            let mut lock = state_move
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !lock.is_empty() && from < lock.len() && from != to {
                let entry = lock.remove(from);
                let target = to.min(lock.len());
                lock.insert(target, entry);
            }
        }
        rebuild_ui(&app, &state_move, &output_move);
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
                    name: source_name(&lock[index].path).into(),
                    folder: source_folder(&lock[index].path).into(),
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
        app.set_status_level(status_level_for_all(&state_pages).into());
        app.set_source_count(
            state_pages
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len() as i32,
        );
        if let Some(row) = updated_row {
            let model = app.get_source_model();
            model.set_row_data(index, row);
        }
        app.set_has_result(false);
        app.set_result_path("".into());
        if let Ok(mut guard) = result_output_for_page.lock() {
            *guard = None;
        }
    });

    let weak_output_dir = weak.clone();
    let output_dir_state = Arc::clone(&output_state);
    let merge_can_merge_output_dir = Arc::clone(&state);
    app.on_output_dir_changed(move |value| {
        let typed = value.to_string();
        if let Some(app) = weak_output_dir.upgrade() {
            let mut guard = output_dir_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.dir = typed.clone();
            guard.dir_modified = true;
            drop(guard);
            app.set_output_dir_str(slint::SharedString::from(typed.as_str()));
            app.set_can_merge(compute_can_merge(
                &merge_can_merge_output_dir,
                &output_dir_state,
            ));
            app.set_has_result(false);
            app.set_result_path("".into());
            if let Ok(mut guard) = result_output_for_output_dir.lock() {
                *guard = None;
            }
            refresh_derived_ui(&app, &merge_can_merge_output_dir, &output_dir_state);
        }
    });

    let weak_output_name = weak.clone();
    let output_name_state = Arc::clone(&output_state);
    let merge_can_merge_output_name = Arc::clone(&state);
    app.on_output_name_changed(move |value| {
        let typed = value.to_string();
        if let Some(app) = weak_output_name.upgrade() {
            let mut guard = output_name_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.name = typed.clone();
            guard.name_modified = true;
            drop(guard);
            app.set_output_name_str(slint::SharedString::from(typed.as_str()));
            app.set_can_merge(compute_can_merge(
                &merge_can_merge_output_name,
                &output_name_state,
            ));
            app.set_has_result(false);
            app.set_result_path("".into());
            if let Ok(mut guard) = result_output_for_output_name.lock() {
                *guard = None;
            }
            refresh_derived_ui(&app, &merge_can_merge_output_name, &output_name_state);
        }
    });

    let weak_timestamp = weak.clone();
    let output_timestamp_state = Arc::clone(&output_state);
    let state_timestamp = Arc::clone(&state);
    let result_output_for_timestamp = Arc::clone(&result_output);
    app.on_output_timestamp_changed(move |checked| {
        if let Some(app) = weak_timestamp.upgrade() {
            {
                let mut guard = output_timestamp_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.timestamp = checked;
            }
            app.set_output_timestamp(checked);
            app.set_has_result(false);
            app.set_result_path("".into());
            if let Ok(mut guard) = result_output_for_timestamp.lock() {
                *guard = None;
            }
            refresh_derived_ui(&app, &state_timestamp, &output_timestamp_state);
        }
    });

    let weak_merge = weak.clone();
    let state_merge = Arc::clone(&state);
    let output_merge = Arc::clone(&output_state);
    let merging_merge = Arc::clone(&merging);
    let progress_processed_merge = Arc::clone(&progress_processed);
    let progress_total_merge = Arc::clone(&progress_total);
    let result_output_merge = Arc::clone(&result_output);
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
            app.set_status_level("error".into());
            return;
        }
        if snapshot.iter().any(|item| item.error) {
            app.set_status_text("Corrigez les sélections en rouge.".into());
            app.set_status_level("error".into());
            return;
        }

        let output_dir_input = {
            let guard = output_merge
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.dir.clone()
        };
        let (output_name_input, output_name_modified, output_timestamp) = {
            let guard = output_merge
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (guard.name.clone(), guard.name_modified, guard.timestamp)
        };
        let output_name_arg = if output_timestamp {
            if output_name_modified && !output_name_input.trim().is_empty() {
                Some(output_name_input.as_str())
            } else {
                None
            }
        } else {
            if !output_name_input.trim().is_empty() {
                Some(output_name_input.as_str())
            } else {
                Some("fusion")
            }
        };
        let output_path = output::resolve_output_path(
            &snapshot[0].path,
            Some(std::path::Path::new(&output_dir_input)),
            output_name_arg,
        );

        let source_paths: Vec<PathBuf> = snapshot.iter().map(|item| item.path.clone()).collect();
        match output::validate_output(&output_path, &source_paths, false) {
            Ok(()) => {
                start_merge(
                    weak_merge.clone(),
                    Arc::clone(&state_merge),
                    Arc::clone(&output_merge),
                    Arc::clone(&merging_merge),
                    Arc::clone(&progress_processed_merge),
                    Arc::clone(&progress_total_merge),
                    Arc::clone(&result_output_merge),
                    false,
                );
            }
            Err(MergeError::OutputExists(_)) => {
                app.set_confirm_open(true);
                app.set_confirm_path(output_path.display().to_string().into());
                app.set_status_text(
                    "Le fichier de sortie existe déjà. Confirmer l'écrasement ?".into(),
                );
                app.set_status_level("neutral".into());
                app.set_has_result(false);
            }
            Err(error) => {
                app.set_status_text(error.to_string().into());
                app.set_status_level("error".into());
            }
        }
    });

    let weak_confirm = weak.clone();
    let state_confirm = Arc::clone(&state);
    let output_confirm = Arc::clone(&output_state);
    let merging_confirm = Arc::clone(&merging);
    let progress_processed_confirm = Arc::clone(&progress_processed);
    let progress_total_confirm = Arc::clone(&progress_total);
    let result_output_confirm = Arc::clone(&result_output);
    app.on_confirm_overwrite(move || {
        if let Some(app) = weak_confirm.upgrade() {
            app.set_confirm_open(false);
            start_merge(
                weak_confirm.clone(),
                Arc::clone(&state_confirm),
                Arc::clone(&output_confirm),
                Arc::clone(&merging_confirm),
                Arc::clone(&progress_processed_confirm),
                Arc::clone(&progress_total_confirm),
                Arc::clone(&result_output_confirm),
                true,
            );
        }
    });

    let weak_cancel = weak.clone();
    app.on_cancel_overwrite(move || {
        if let Some(app) = weak_cancel.upgrade() {
            app.set_confirm_open(false);
            app.set_status_text("Écrasement annulé.".into());
            app.set_status_level("neutral".into());
        }
    });

    let weak_duplicate = weak.clone();
    let state_duplicate = Arc::clone(&state);
    let output_duplicate = Arc::clone(&output_state);
    app.on_duplicate_source(move |index| {
        if let Some(app) = weak_duplicate.upgrade() {
            let index = index as usize;
            {
                let mut lock = state_duplicate
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if index < lock.len() {
                    let duplicated = lock[index].clone();
                    lock.push(duplicated);
                }
            }
            app.set_has_result(false);
            app.set_result_path("".into());
            rebuild_ui(&app, &state_duplicate, &output_duplicate);
        }
    });

    let weak_clear = weak.clone();
    let state_clear = Arc::clone(&state);
    let output_clear = Arc::clone(&output_state);
    app.on_clear_sources(move || {
        if let Some(app) = weak_clear.upgrade() {
            {
                let mut lock = state_clear
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                lock.clear();
            }
            app.set_has_result(false);
            app.set_result_path("".into());
            app.set_status_text("Liste vide.".into());
            app.set_status_level("neutral".into());
            rebuild_ui(&app, &state_clear, &output_clear);
        }
    });

    let weak_browse = weak.clone();
    let output_browse = Arc::clone(&output_state);
    let state_browse = Arc::clone(&state);
    app.on_browse_output_directory(move || {
        let weak = weak_browse.clone();
        let state = Arc::clone(&state_browse);
        let output = Arc::clone(&output_browse);
        std::thread::spawn(move || {
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            {
                let mut guard = output
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.dir = folder.to_string_lossy().to_string();
                guard.dir_modified = true;
            }
            let weak = weak.clone();
            let state = Arc::clone(&state);
            let output = Arc::clone(&output);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_has_result(false);
                    app.set_result_path("".into());
                    refresh_derived_ui(&app, &state, &output);
                }
            });
        });
    });

    let weak_open_file = weak.clone();
    app.on_open_result_file(move || {
        if let Some(app) = weak_open_file.upgrade() {
            let path = app.get_result_path().to_string();
            if path.is_empty() {
                return;
            }
            match open_location(&PathBuf::from(path)) {
                Ok(()) => {
                    app.set_status_text("Fichier ouvert.".into());
                    app.set_status_level("success".into());
                }
                Err(error) => {
                    app.set_status_text(error.into());
                    app.set_status_level("error".into());
                }
            }
        }
    });

    let weak_open_directory = weak.clone();
    app.on_open_result_directory(move || {
        if let Some(app) = weak_open_directory.upgrade() {
            let path = app.get_result_path().to_string();
            if path.is_empty() {
                return;
            }
            let location = PathBuf::from(&path)
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(&path));
            match open_location(&location) {
                Ok(()) => {
                    app.set_status_text("Dossier ouvert.".into());
                    app.set_status_level("success".into());
                }
                Err(error) => {
                    app.set_status_text(error.into());
                    app.set_status_level("error".into());
                }
            }
        }
    });

    rebuild_ui(&app, &state, &output_state);
    app.set_result_path("".into());
    app.set_has_result(false);
    app.set_confirm_open(false);
    app.set_merging(false);
    app.set_progress_processed(0);
    app.set_progress_total(0);
    app.set_progress_ratio(0.0);
    let _ = app.run();
}

#[allow(clippy::too_many_arguments)]
fn start_merge(
    weak: slint::Weak<crate::AppWindow>,
    state: Arc<Mutex<Vec<SourceState>>>,
    output_state: Arc<Mutex<OutputState>>,
    merging: Arc<AtomicBool>,
    progress_processed: Arc<AtomicUsize>,
    progress_total: Arc<AtomicUsize>,
    result_output: Arc<Mutex<Option<PathBuf>>>,
    overwrite: bool,
) {
    let Some(app) = weak.upgrade() else {
        return;
    };
    if merging.load(Ordering::Relaxed) {
        return;
    }

    let snapshot = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if snapshot.is_empty() {
        app.set_status_text("Ajoutez au moins un PDF.".into());
        app.set_status_level("error".into());
        return;
    }
    if snapshot.iter().any(|item| item.error) {
        app.set_status_text("Corrigez les sélections en rouge.".into());
        app.set_status_level("error".into());
        return;
    }

    let output_dir_value = {
        let guard = output_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.dir.clone()
    };
    let (output_name_value, output_name_modified, output_timestamp) = {
        let guard = output_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (guard.name.clone(), guard.name_modified, guard.timestamp)
    };
    let output_dir = if output_dir_value.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(output_dir_value))
    };
    let requested_output_name = if output_timestamp {
        if output_name_modified && !output_name_value.trim().is_empty() {
            Some(output_name_value)
        } else {
            None
        }
    } else if !output_name_value.trim().is_empty() {
        Some(output_name_value)
    } else {
        Some("fusion".to_owned())
    };
    let output_path = output::resolve_output_path(
        &snapshot[0].path,
        output_dir.as_deref(),
        requested_output_name.as_deref(),
    );
    let source_paths: Vec<PathBuf> = snapshot.iter().map(|item| item.path.clone()).collect();
    if let Err(error) = output::validate_output(&output_path, &source_paths, overwrite) {
        app.set_status_text(error.to_string().into());
        app.set_status_level("error".into());
        return;
    }

    let mut request_sources = Vec::new();
    for item in &snapshot {
        let spec = match &item.spec {
            Ok(spec) => spec.clone(),
            Err(_) => {
                app.set_status_text("Sélection invalide.".into());
                app.set_status_level("error".into());
                return;
            }
        };
        request_sources.push(SourceEntry {
            path: item.path.clone(),
            spec,
        });
    }

    merging.store(true, Ordering::Relaxed);
    progress_processed.store(0, Ordering::Relaxed);
    progress_total.store(0, Ordering::Relaxed);
    app.set_merging(true);
    app.set_confirm_open(false);
    app.set_has_result(false);
    app.set_result_path("".into());
    if let Ok(mut guard) = result_output.lock() {
        *guard = None;
    }
    app.set_status_text("Fusion en cours...".into());
    app.set_status_level("neutral".into());
    app.set_can_merge(false);
    app.set_progress_processed(0);
    app.set_progress_total(0);
    app.set_progress_ratio(0.0);

    let state_thread = Arc::clone(&state);
    let output_thread = Arc::clone(&output_state);
    let merging_thread = Arc::clone(&merging);
    let processed_thread = Arc::clone(&progress_processed);
    let total_thread = Arc::clone(&progress_total);
    let result_thread = Arc::clone(&result_output);
    let weak_thread = weak.clone();
    let last_publish = Arc::new(Mutex::new(Instant::now()));
    let request = pdfmerge_core::MergeRequest {
        sources: request_sources,
        output: output_path.clone(),
        overwrite,
    };
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
                    let processed = processed.load(Ordering::Relaxed);
                    let total = total.load(Ordering::Relaxed);
                    let ratio = if total == 0 {
                        0.0
                    } else {
                        processed as f32 / total as f32
                    };
                    app.set_progress_processed(processed as i32);
                    app.set_progress_total(total as i32);
                    app.set_progress_ratio(ratio);
                }
            });
        });

        merging_thread.store(false, Ordering::Relaxed);
        let total_value = match &result {
            Ok(report) => report.page_count,
            Err(_) => total_thread.load(Ordering::Relaxed),
        };
        let processed_value = if result.is_ok() {
            total_value
        } else {
            progress_processed.load(Ordering::Relaxed)
        };
        total_thread.store(total_value, Ordering::Relaxed);
        progress_processed.store(processed_value, Ordering::Relaxed);

        let weak = weak_thread.clone();
        let state = Arc::clone(&state_thread);
        let output = Arc::clone(&output_thread);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                let status_level = if result.is_ok() { "success" } else { "error" };
                let status = match &result {
                    Ok(report) => {
                        if let Ok(mut guard) = result_thread.lock() {
                            *guard = Some(report.output.clone());
                        }
                        app.set_result_path(report.output.display().to_string().into());
                        app.set_has_result(true);
                        let page_word = if report.page_count > 1 {
                            "pages"
                        } else {
                            "page"
                        };
                        format!(
                            "Fusion terminée : {} ({} {})",
                            report.output.display(),
                            report.page_count,
                            page_word
                        )
                    }
                    Err(error) => error.to_string(),
                };
                app.set_status_text(status.into());
                app.set_status_level(status_level.into());
                app.set_progress_processed(processed_value as i32);
                app.set_progress_total(total_value as i32);
                let ratio = if total_value == 0 {
                    0.0
                } else {
                    processed_value as f32 / total_value as f32
                };
                app.set_progress_ratio(ratio);
                app.set_merging(false);
                app.set_can_merge(compute_can_merge(&state, &output));
                refresh_derived_ui(&app, &state, &output);
            }
        });
    });
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
        let _invoke_result = slint::invoke_from_event_loop(move || {
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
                            output.name = "fusion".to_owned();
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
                    spec: Err(SpecError::new(1, "document chiffré".to_owned())),
                    status: "PDF chiffré non pris en charge".to_owned(),
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
        let pages = if item.page_count > 1 { "pages" } else { "page" };
        item.status = format!("Toutes ({} {pages})", item.page_count);
        item.error = false;
        return;
    }
    match PageSpec::parse(&item.spec_input) {
        Ok(spec) => match spec.resolve(item.page_count) {
            Ok(pages) => {
                item.spec = Ok(spec);
                let word = if pages.len() > 1 { "pages" } else { "page" };
                item.status = format!("{} {word}", pages.len());
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
        return "Aucune source · Ajoutez au moins un PDF.".to_owned();
    }
    let source_count = lock.len();
    let total_pages: u32 = lock
        .iter()
        .filter_map(|item| {
            item.spec
                .as_ref()
                .ok()
                .and_then(|spec| spec.resolve(item.page_count).ok())
                .map(|pages| pages.len() as u32)
        })
        .sum();
    if lock.iter().any(|item| item.error) {
        format!(
            "Corrigez les sélections en rouge. · {} sources · {} {}",
            source_count,
            total_pages,
            page_word_for(total_pages as usize)
        )
    } else if source_count == 1 {
        format!(
            "1 source · {} {} · prête",
            total_pages,
            page_word_for(total_pages as usize)
        )
    } else {
        format!(
            "{} sources · {} {} · prêtes",
            source_count,
            total_pages,
            page_word_for(total_pages as usize)
        )
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
    // Les champs de sortie sont activés par défaut : un répertoire peut rester vide
    // et être complété par le défaut, tandis que le nom "fusion" n'est jamais bloquant.
    let _output = output_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    true
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
                name: source_name(&item.path).into(),
                folder: source_folder(&item.path).into(),
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

    app.set_has_result(false);
    app.set_result_path("".into());

    // rebuild_ui must refresh the aggregate UI state too: otherwise source-
    // count stays at its old value and the newly created model is not shown
    // until another operation (such as the output-directory dialog) happens
    // to update those properties.
    refresh_derived_ui(app, state, output_state);
}
