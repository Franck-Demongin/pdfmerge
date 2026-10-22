use std::path::{Path, PathBuf};

use natord;

use crate::{PageSpec, SourceEntry, SourceError};

/// Développe un chemin source en entrées PDF.
///
/// Un fichier PDF devient une seule entrée. Un répertoire est développé en
/// une entrée par PDF, sans récursivité (`read_dir` seulement), en acceptant
/// les extensions `.pdf` quelle que soit leur casse. Les entrées sont triées par
/// ordre alphabétique naturel.
pub fn expand_input(path: &Path) -> Result<Vec<SourceEntry>, SourceError> {
    if path.is_file() {
        return Ok(vec![SourceEntry {
            path: path.to_path_buf(),
            spec: PageSpec::all(),
        }]);
    }

    if !path.is_dir() {
        return Err(SourceError::ReadFile {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "source does not exist or is not a directory",
            ),
        });
    }

    let read_dir = std::fs::read_dir(path).map_err(|source| SourceError::ReadDirectory {
        path: path.to_path_buf(),
        source,
    })?;

    let mut entries = Vec::new();
    for item in read_dir {
        let item = item.map_err(|source| SourceError::ReadDirectory {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = item.path();
        let file_type = item
            .file_type()
            .map_err(|source| SourceError::ReadDirectory {
                path: path.to_path_buf(),
                source,
            })?;

        // Les sous-dossiers sont ignorés en v1.
        if !file_type.is_file() {
            continue;
        }
        let is_pdf = entry_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false);
        if !is_pdf {
            continue;
        }

        entries.push(SourceEntry {
            path: entry_path,
            spec: PageSpec::all(),
        });
    }

    entries.sort_by(|left, right| {
        natord::compare(
            left.path.to_string_lossy().as_ref(),
            right.path.to_string_lossy().as_ref(),
        )
    });

    Ok(entries)
}

/// Alias conservant le type normalisé attendu à l'extérieur.
pub fn normalize_source_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch_files(dir: &TempDir, names: &[&str]) {
        for name in names {
            fs::write(dir.path().join(name), b"".as_ref()).expect("test PDF placeholder");
        }
    }

    #[test]
    fn file_input_becomes_one_entry() {
        let temp = TempDir::new().expect("temp dir");
        let file = temp.path().join("example.pdf");
        fs::write(&file, []).expect("create source");

        let entries = expand_input(&file).expect("single source");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, file);
        assert!(entries[0].spec.is_all());
    }

    #[test]
    fn directory_is_expanded_non_recursively_and_sorted_naturally() {
        let temp = TempDir::new().expect("temp dir");
        touch_files(&temp, &["a.pdf", "doc2.pdf", "doc10.pdf", "readme.txt"]);
        fs::create_dir(temp.path().join("nested")).expect("nested directory");
        fs::write(temp.path().join("nested").join("nested.pdf"), []).expect("nested PDF");

        let entries = expand_input(temp.path()).expect("directory source");
        let names: Vec<_> = entries
            .iter()
            .map(|entry| entry.path.file_name().unwrap().to_string_lossy())
            .collect();
        assert_eq!(names, vec!["a.pdf", "doc2.pdf", "doc10.pdf"]);
    }

    #[test]
    fn pdf_extension_is_case_insensitive() {
        let temp = TempDir::new().expect("temp dir");
        fs::write(temp.path().join("SCAN.PDF"), []).expect("uppercase extension");

        let entries = expand_input(temp.path()).expect("directory");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn missing_source_is_an_error() {
        let path = std::env::temp_dir().join("this-pdfmerge-source-should-not-exist.pdf");
        assert!(matches!(
            expand_input(&path),
            Err(SourceError::ReadFile { .. })
        ));
    }
}
