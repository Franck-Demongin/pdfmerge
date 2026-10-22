use assert_cmd::Command;
use lopdf::{Document, Object, dictionary};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[derive(Clone, Copy)]
struct DirectPage {
    width: i64,
    height: i64,
}

fn make_pdf(path: &Path, pages: &[DirectPage]) -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let mut page_ids = Vec::with_capacity(pages.len());

    for page in pages {
        let page_id = document.new_object_id();
        let media_box = Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(page.width),
            Object::Integer(page.height),
        ]);
        let page_object = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => media_box,
        };
        document.set_object(page_id, Object::Dictionary(page_object));
        page_ids.push(Object::Reference(page_id));
    }

    let pages_tree = dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(page_ids),
        "Count" => Object::Integer(pages.len() as i64),
    };
    document.set_object(pages_id, Object::Dictionary(pages_tree));

    let catalog_id = document.new_object_id();
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    };
    document.set_object(catalog_id, Object::Dictionary(catalog));
    document.trailer.set("Root", Object::Reference(catalog_id));

    document.save(path)?;
    Ok(())
}

fn pdfmerge_command() -> Result<Command, assert_cmd::cargo::CargoError> {
    Command::cargo_bin("pdfmerge")
}

fn page_dimensions(path: &Path) -> Result<Vec<(i64, i64)>, Box<dyn std::error::Error>> {
    let document = Document::load(path)?;
    let mut dimensions = Vec::new();
    for (_, page_id) in document.get_pages() {
        let page = document.get_dictionary(page_id)?;
        let media_box = page.get(b"MediaBox")?.as_array()?;
        let width = media_box[2].as_i64()?;
        let height = media_box[3].as_i64()?;
        dimensions.push((width, height));
    }
    Ok(dimensions)
}

#[test]
fn help_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let output = pdfmerge_command()?.arg("--help").output()?;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Fusion de PDF"));
    Ok(())
}

#[test]
fn missing_source_is_read_error() -> Result<(), Box<dyn std::error::Error>> {
    let output = pdfmerge_command()?.arg("/does/not/exist.pdf").output()?;
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    Ok(())
}

#[test]
fn invalid_selection_exits_with_code_2() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.pdf");
    let output = temp.path().join("out.pdf");
    make_pdf(
        &source,
        &[DirectPage {
            width: 100,
            height: 100,
        }],
    )?;

    let output_result = pdfmerge_command()?
        .arg(format!("{}[2]", source.display()))
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(!output_result.status.success());
    assert_eq!(output_result.status.code(), Some(2));
    Ok(())
}

#[test]
fn merges_single_source() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.pdf");
    let output = temp.path().join("merged.pdf");
    make_pdf(
        &source,
        &[
            DirectPage {
                width: 100,
                height: 100,
            },
            DirectPage {
                width: 200,
                height: 200,
            },
        ],
    )?;

    let result = pdfmerge_command()?
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists());

    let loaded = Document::load(&output)?;
    assert_eq!(loaded.get_pages().len(), 2);
    Ok(())
}

#[test]
fn merges_sources_in_argument_order() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    let output = temp.path().join("merged.pdf");
    make_pdf(
        &first,
        &[DirectPage {
            width: 111,
            height: 100,
        }],
    )?;
    make_pdf(
        &second,
        &[
            DirectPage {
                width: 222,
                height: 200,
            },
            DirectPage {
                width: 333,
                height: 300,
            },
        ],
    )?;

    let result = pdfmerge_command()?
        .arg(&first)
        .arg(format!("{}[1]", second.display()))
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert_eq!(page_dimensions(&output)?, vec![(111, 100), (222, 200)]);
    Ok(())
}

#[test]
fn expands_directory_in_natural_order() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let folder = temp.path().join("sources");
    fs::create_dir_all(&folder)?;
    let output = temp.path().join("merged.pdf");

    make_pdf(
        &folder.join("doc10.pdf"),
        &[DirectPage {
            width: 10,
            height: 10,
        }],
    )?;
    make_pdf(
        &folder.join("doc2.pdf"),
        &[DirectPage {
            width: 20,
            height: 20,
        }],
    )?;

    let result = pdfmerge_command()?
        .arg(&folder)
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert_eq!(page_dimensions(&output)?, vec![(20, 20), (10, 10)]);
    Ok(())
}

#[test]
fn dry_run_does_not_create_output() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.pdf");
    let output = temp.path().join("dry.pdf");
    make_pdf(
        &source,
        &[
            DirectPage {
                width: 1,
                height: 1,
            },
            DirectPage {
                width: 2,
                height: 2,
            },
        ],
    )?;

    let result = pdfmerge_command()?
        .arg(&source)
        .arg("--dry-run")
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(result.status.success());
    assert!(!output.exists());

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("(2 pages)"));
    assert!(stdout.contains("Sortie :"));
    Ok(())
}

#[test]
fn overwriting_existing_output_requires_force() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.pdf");
    let output = temp.path().join("existing.pdf");
    make_pdf(
        &source,
        &[DirectPage {
            width: 1,
            height: 1,
        }],
    )?;
    fs::write(&output, "existing output")?;

    let result = pdfmerge_command()?
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()?;
    assert!(!result.status.success());
    assert_eq!(result.status.code(), Some(4));
    assert_eq!(fs::read_to_string(&output)?, "existing output");
    Ok(())
}

#[test]
fn output_cannot_overwrite_a_source() -> Result<(), Box<dyn std::error::Error>> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.pdf");
    make_pdf(
        &source,
        &[DirectPage {
            width: 1,
            height: 1,
        }],
    )?;

    let result = pdfmerge_command()?
        .arg(&source)
        .arg("-o")
        .arg(&source)
        .output()?;
    assert!(!result.status.success());
    assert_eq!(result.status.code(), Some(4));
    Ok(())
}
