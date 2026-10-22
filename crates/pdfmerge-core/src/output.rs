use std::path::{Path, PathBuf};

use crate::MergeError;

/// Produit le nom de sortie par défaut : `fusion_AAAAMMJJ-HHMMSS.pdf`.
pub fn default_output_name() -> String {
    format!(
        "fusion_{}.pdf",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

/// Dossier par défaut : celui de la première source.
pub fn default_output_dir(first_source: &Path) -> PathBuf {
    if first_source.is_dir() {
        return first_source.to_path_buf();
    }

    let looks_like_pdf = first_source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false);
    if looks_like_pdf {
        if let Some(parent) = first_source.parent() {
            return parent.to_path_buf();
        }
    }

    first_source.to_path_buf()
}

/// Construit un chemin de sortie avec valeur par défaut et extension `.pdf`.
pub fn default_output_path(first_source: &Path, name: Option<&str>) -> PathBuf {
    resolve_output_path(first_source, None, name)
}

/// Construit le chemin de sortie à partir des options CLI/GUI.
pub fn resolve_output_path(
    first_source: &Path,
    output_dir: Option<&Path>,
    output_name: Option<&str>,
) -> PathBuf {
    let dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_output_dir(first_source));
    let name = output_name
        .map(str::to_owned)
        .unwrap_or_else(default_output_name);
    ensure_pdf_extension(dir.join(name))
}

pub fn ensure_pdf_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
    {
        path
    } else {
        path.with_extension("pdf")
    }
}

/// Chemin canonique pour comparer des chemins hétérogènes (existant/non existant).
pub fn normalized_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }

    if let Some(parent) = path.parent() {
        if parent.exists() {
            if let Ok(parent_canonical) = std::fs::canonicalize(parent) {
                if let Some(file_name) = path.file_name() {
                    return parent_canonical.join(file_name);
                }
            }
        }
    }

    path.to_path_buf()
}

/// Valide qu'une sortie est licite avant écriture.
#[allow(dead_code)]
pub fn validate_output(
    output: &Path,
    sources: &[PathBuf],
    overwrite: bool,
) -> Result<(), MergeError> {
    for source in sources {
        let canonical_source = std::fs::canonicalize(source)?;
        if normalized_path(output) == canonical_source {
            return Err(MergeError::OutputOverwritesSource(canonical_source));
        }
    }

    if output.exists() && !overwrite {
        return Err(MergeError::OutputExists(output.to_path_buf()));
    }

    Ok(())
}

/// Nom temporaire dans le même dossier que la sortie, pour une écriture atomique.
#[allow(dead_code)]
pub fn temp_output_path(output: &Path) -> PathBuf {
    output.with_extension(format!("tmp-{}", std::process::id()))
}

/// Renomme le fichier temporaire vers la sortie. La validation a déjà eu lieu.
#[allow(dead_code)]
pub fn commit_output(temp: &Path, output: &Path) -> Result<(), MergeError> {
    std::fs::rename(temp, output).map_err(MergeError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn default_name_uses_expected_pattern() {
        let name = default_output_name();
        let stem = name.strip_prefix("fusion_").expect("prefix");
        let stem = stem.strip_suffix(".pdf").expect("suffix");
        let (date, time) = stem.split_once('-').expect("separator");
        assert_eq!(date.len(), 8);
        assert_eq!(time.len(), 6);
        assert!(date.chars().all(char::is_numeric));
        assert!(time.chars().all(char::is_numeric));
    }

    #[test]
    fn extension_is_added_when_missing() {
        assert_eq!(
            ensure_pdf_extension(PathBuf::from("/tmp/out")),
            PathBuf::from("/tmp/out.pdf")
        );
    }

    #[test]
    fn extension_is_preserved() {
        assert_eq!(
            ensure_pdf_extension(PathBuf::from("/tmp/out.PDF")),
            PathBuf::from("/tmp/out.PDF")
        );
    }

    #[test]
    fn default_dir_uses_source_directory() {
        assert_eq!(
            default_output_dir(Path::new("/tmp/folder/in.pdf")),
            PathBuf::from("/tmp/folder")
        );
        assert_eq!(
            default_output_dir(Path::new("/tmp/folder")),
            PathBuf::from("/tmp/folder")
        );
    }

    #[test]
    fn validate_rejects_same_canonical_path_as_source() {
        let temp = TempDir::new().expect("temp dir");
        let source = temp.path().join("source.pdf");
        fs::write(&source, b"").expect("source");
        let output = source.clone();

        let error = validate_output(&output, &[source.clone()], true).unwrap_err();
        assert!(matches!(error, MergeError::OutputOverwritesSource(_)));
    }

    #[test]
    fn validate_rejects_existing_output_without_overwrite() {
        let temp = TempDir::new().expect("temp dir");
        let source = temp.path().join("source.pdf");
        let output = temp.path().join("out.pdf");
        fs::write(&source, b"").expect("source");
        fs::write(&output, b"").expect("output");

        let error = validate_output(&output, &[source], false).unwrap_err();
        assert!(matches!(error, MergeError::OutputExists(_)));
    }

    #[test]
    fn validate_allows_existing_output_with_overwrite() {
        let temp = TempDir::new().expect("temp dir");
        let source = temp.path().join("source.pdf");
        let output = temp.path().join("out.pdf");
        fs::write(&source, b"").expect("source");
        fs::write(&output, b"").expect("output");

        assert!(validate_output(&output, &[source], true).is_ok());
    }

    #[test]
    fn temp_path_stays_in_output_directory() {
        let temp = temp_output_path(Path::new("/tmp/pdfmerge/out.pdf"));
        assert!(temp.to_string_lossy().contains(".tmp-"));
        let _ = temp;
    }

    #[test]
    fn commit_output_produces_final_file() {
        let temp = TempDir::new().expect("temp dir");
        let output = temp.path().join("out.pdf");
        let temporary = temp_output_path(&output);
        fs::write(&temporary, b"hello").expect("temporary");

        commit_output(&temporary, &output).expect("commit");
        assert!(output.exists());
        assert!(!temporary.exists());
    }
}
