use anyhow::Result;
use clap::Parser;
use pdfmerge_core::output::{ensure_pdf_extension, resolve_output_path, validate_output};
use pdfmerge_core::{PageSpec, Progress, SourceEntry, merge, sources};

/// Interface en ligne de commande.
#[derive(Parser)]
#[command(
    name = "pdfmerge",
    version,
    about = "Fusion de PDF avec selection de pages"
)]
pub struct Cli {
    /// Fichiers PDF, dossiers, ou chemins avec une selection (ex. a.pdf[3-6,1]).
    #[arg(value_name = "ENTREE")]
    pub inputs: Vec<String>,

    /// Fichier de sortie complet (dossier + nom).
    #[arg(short = 'o', long = "output", conflicts_with_all = ["output_dir", "name"])]
    pub output: Option<std::path::PathBuf>,

    /// Dossier de sortie.
    #[arg(short = 'd', long = "output-dir", conflicts_with = "output")]
    pub output_dir: Option<std::path::PathBuf>,

    /// Nom du fichier de sortie.
    #[arg(short = 'n', long = "name", conflicts_with = "output")]
    pub name: Option<String>,

    /// Ecrase la sortie existante.
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// Affiche le plan sans ecrire le fichier.
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Sortie silencieuse.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Sortie detaillee.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Force le lancement de l'interface graphique.
    #[arg(long = "gui")]
    pub gui: bool,
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Selection(String),
    PdfRead(String),
    Output(String),
}

impl CliError {
    const fn code(&self) -> i32 {
        match self {
            Self::Usage(_) => 1,
            Self::Selection(_) => 2,
            Self::PdfRead(_) => 3,
            Self::Output(_) => 4,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Usage(message)
            | Self::Selection(message)
            | Self::PdfRead(message)
            | Self::Output(message) => message,
        }
    }
}

fn map_source_error(path: &std::path::Path, error: &pdfmerge_core::SourceError) -> CliError {
    match error {
        pdfmerge_core::SourceError::ReadDirectory { source, .. }
        | pdfmerge_core::SourceError::ReadFile { source, .. } => {
            if source.kind() == std::io::ErrorKind::NotFound {
                CliError::PdfRead(format!("source introuvable : {}", path.display()))
            } else {
                CliError::PdfRead(format!("impossible de lire {} : {source}", path.display()))
            }
        }
    }
}

fn parse_source(value: &str) -> Result<(std::path::PathBuf, Option<String>), CliError> {
    let trimmed = value.trim();
    if let Some(close) = trimmed.rfind(']') {
        if close == trimmed.len().saturating_sub(1) {
            if let Some(open) = trimmed.rfind('[') {
                if open > 0 {
                    let path = trimmed[..open].trim();
                    let selection = &trimmed[open + 1..close];
                    if path.is_empty() {
                        return Err(CliError::Usage(
                            "chemin source vide avant la selection".to_owned(),
                        ));
                    }
                    return Ok((std::path::PathBuf::from(path), Some(selection.to_owned())));
                }
            }
        }
    }
    Ok((std::path::PathBuf::from(trimmed), None))
}

fn expand_sources(cli: &Cli) -> Result<Vec<SourceEntry>, CliError> {
    if cli.inputs.is_empty() {
        return Err(CliError::Usage("aucune entree a traiter".to_owned()));
    }

    let mut sources = Vec::new();
    for input in &cli.inputs {
        let (path, selection) = parse_source(input)?;
        if let Some(selection) = selection.as_ref() {
            let _ = PageSpec::parse(selection).map_err(|error| {
                CliError::Selection(format!(
                    "selection invalide pour {} : {error}",
                    path.display()
                ))
            })?;
        }
        let expanded =
            sources::expand_input(&path).map_err(|error| map_source_error(&path, &error))?;
        for mut entry in expanded {
            if let Some(selection) = selection.as_ref() {
                entry.spec = PageSpec::parse(selection).map_err(|error| {
                    CliError::Selection(format!(
                        "selection invalide pour {} : {error}",
                        entry.path.display()
                    ))
                })?;
            }
            sources.push(entry);
        }
    }
    if sources.is_empty() {
        return Err(CliError::Usage("aucun fichier PDF trouve".to_owned()));
    }
    Ok(sources)
}

fn format_pages(pages: &[u32]) -> String {
    if pages.len() > 24 {
        let mut head = pages[..12]
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        head.push_str("...");
        head
    } else {
        pages
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub fn run(cli: Cli) -> i32 {
    match run_inner(cli) {
        Ok(()) => 0,
        Err(error) => {
            let stderr = std::io::stderr();
            writeln_simple(stderr, error.message());
            error.code()
        }
    }
}

fn run_inner(cli: Cli) -> Result<(), CliError> {
    let sources = expand_sources(&cli)?;
    let first = sources
        .first()
        .map(|source| source.path.clone())
        .ok_or_else(|| CliError::Usage("aucune entree a traiter".to_owned()))?;

    // Preflight so read errors have the right exit code before writing.
    for entry in &sources {
        let document = lopdf::Document::load(&entry.path).map_err(|error| {
            CliError::PdfRead(format!(
                "impossible de lire {} : {error}",
                entry.path.display()
            ))
        })?;
        if document.is_encrypted() {
            return Err(CliError::PdfRead(format!(
                "fichier PDF chiffre non pris en charge : {}",
                entry.path.display()
            )));
        }
        let count = document.get_pages().len() as u32;
        entry
            .spec
            .resolve(count)
            .map_err(|error| CliError::Selection(error.to_string()))?;
    }

    let output = if let Some(path) = cli.output.clone() {
        ensure_pdf_extension(path)
    } else {
        resolve_output_path(
            &first,
            cli.output_dir.as_ref().map(std::path::Path::new),
            cli.name.as_deref(),
        )
    };

    if cli.dry_run {
        for entry in &sources {
            let document = lopdf::Document::load(&entry.path).map_err(|error| {
                CliError::PdfRead(format!(
                    "impossible de lire {} : {error}",
                    entry.path.display()
                ))
            })?;
            let count = document.get_pages().len() as u32;
            let pages = entry
                .spec
                .resolve(count)
                .map_err(|error| CliError::Selection(error.to_string()))?;
            println!(
                "{} ({} pages) -> {}",
                entry.path.display(),
                count,
                format_pages(&pages)
            );
        }
        println!("Sortie : {}", output.display());
        return Ok(());
    }

    validate_output(
        &output,
        &sources
            .iter()
            .map(|source| source.path.clone())
            .collect::<Vec<_>>(),
        cli.force,
    )
    .map_err(map_output_error)?;

    let request = pdfmerge_core::MergeRequest {
        sources,
        output: output.clone(),
        overwrite: cli.force,
    };
    merge(&request, |_progress: Progress| {}).map_err(map_merge_error)?;

    if !cli.quiet {
        println!("Cree : {}", output.display());
    }
    Ok(())
}

fn map_output_error(error: pdfmerge_core::MergeError) -> CliError {
    match error {
        pdfmerge_core::MergeError::OutputExists(path) => CliError::Output(format!(
            "la sortie existe deja : {} (utiliser --force)",
            path.display()
        )),
        pdfmerge_core::MergeError::OutputOverwritesSource(path) => CliError::Output(format!(
            "la sortie ne peut pas ecraser une source : {}",
            path.display()
        )),
        other => CliError::Output(other.to_string()),
    }
}

fn map_merge_error(error: pdfmerge_core::MergeError) -> CliError {
    match error {
        pdfmerge_core::MergeError::Selection { .. } => CliError::Selection(error.to_string()),
        pdfmerge_core::MergeError::Encrypted(path) => CliError::PdfRead(format!(
            "fichier PDF chiffre non pris en charge : {}",
            path.display()
        )),
        pdfmerge_core::MergeError::Io(ref io_error) => {
            if io_error.kind() == std::io::ErrorKind::NotFound {
                CliError::PdfRead(error.to_string())
            } else {
                CliError::Output(error.to_string())
            }
        }
        pdfmerge_core::MergeError::Pdf(_) => CliError::PdfRead(error.to_string()),
        pdfmerge_core::MergeError::OutputExists(path) => CliError::Output(format!(
            "la sortie existe deja : {} (utiliser --force)",
            path.display()
        )),
        pdfmerge_core::MergeError::OutputOverwritesSource(path) => CliError::Output(format!(
            "la sortie ne peut pas ecraser une source : {}",
            path.display()
        )),
        pdfmerge_core::MergeError::NoSources => CliError::Usage(error.to_string()),
    }
}

fn writeln_simple(mut writer: std::io::Stderr, message: &str) {
    use std::io::Write;
    let _ = writeln!(writer, "{message}");
}
