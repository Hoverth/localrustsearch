use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use std::process::ExitCode;
use std::result::Result;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(feature = "xml")]
use std::io::BufReader;
#[cfg(feature = "xml")]
use xml::common::{Position, TextPosition};
#[cfg(feature = "xml")]
use xml::reader::{EventReader, XmlEvent};

mod model;
use model::*;
mod lexer;
pub mod snowball;
mod tui;

fn parse_entire_txt_file(file_path: &Path) -> Result<String, ()> {
    fs::read_to_string(file_path).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: coult not open file {file_path}: {err}",
                file_path = file_path.display()
            );
        }
    })
}

#[cfg(feature = "pdf")]
fn parse_entire_pdf_file(file_path: &Path) -> Result<String, ()> {
    use poppler::Document;
    use std::io::Read;

    let mut content = Vec::new();
    File::open(file_path)
        .and_then(|mut file| file.read_to_end(&mut content))
        .map_err(|err| {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                    "ERROR: could not read file {file_path}: {err}",
                    file_path = file_path.display()
                );
            }
        })?;

    let pdf = Document::from_data(&content, None).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: could not read file {file_path}: {err}",
                file_path = file_path.display()
            );
        }
    })?;

    let mut result = String::new();

    let n = pdf.n_pages();
    for i in 0..n {
        let page = pdf.page(i).expect(&format!(
            "{i} is within the bounds of the range of the page"
        ));
        if let Some(content) = page.text() {
            result.push_str(content.as_str());
            result.push(' ');
        }
    }

    Ok(result)
}

#[cfg(feature = "xml")]
fn parse_entire_xml_file(file_path: &Path) -> Result<String, ()> {
    let file = File::open(file_path).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: could not open file {file_path}: {err}",
                file_path = file_path.display()
            );
        }
    })?;
    let er = EventReader::new(BufReader::new(file));
    let mut content = String::new();
    for event in er.into_iter() {
        let event = event.map_err(|err| {
            let TextPosition { row, column } = err.position();
            let msg = err.msg();
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                    "{file_path}:{row}:{column}: ERROR: {msg}",
                    file_path = file_path.display()
                );
            }
        })?;

        if let XmlEvent::Characters(text) = event {
            content.push_str(&text);
            content.push(' ');
        }
    }
    Ok(content)
}

fn parse_entire_file_by_extension(file_path: &Path) -> Result<String, ()> {
    let extension = file_path
        .extension()
        .ok_or_else(|| {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                    "ERROR: can't detect file type of {file_path} without extension",
                    file_path = file_path.display()
                );
            }
        })?
        .to_string_lossy();
    match extension.as_ref() {
        #[cfg(feature = "xml")]
        "xhtml" | "xml" => parse_entire_xml_file(file_path),

        // TODO: specialized parser for markdown files
        "txt" | "md" => parse_entire_txt_file(file_path),

        #[cfg(feature = "pdf")]
        "pdf" => parse_entire_pdf_file(file_path),
        _ => {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                "ERROR: can't detect file type of {file_path}: unsupported extension {extension}",
                file_path = file_path.display(),
                extension = extension
            );
            }
            Err(())
        }
    }
}

fn save_model_as_json(model: &Model, index_path: &Path) -> Result<(), ()> {
    if env::var("LRS_LOG").is_ok() {
        println!("Saving {index_path}...", index_path = index_path.display());
    }

    let index_file = File::create(index_path).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: could not create index file {index_path}: {err}",
                index_path = index_path.display()
            );
        }
    })?;

    serde_json::to_writer(BufWriter::new(index_file), &model).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: could not serialize index into file {index_path}: {err}",
                index_path = index_path.display()
            );
        }
    })?;

    Ok(())
}

fn add_folder_to_model(
    dir_path: &Path,
    model: Arc<Mutex<Model>>,
    processed: &mut usize,
) -> Result<(), ()> {
    let dir = fs::read_dir(dir_path).map_err(|err| {
        if env::var("LRS_LOG").is_ok() {
            eprintln!(
                "ERROR: could not open directory {dir_path} for indexing: {err}",
                dir_path = dir_path.display()
            );
        }
    })?;

    'next_file: for file in dir {
        let file = file.map_err(|err| {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                "ERROR: could not read next file in directory {dir_path} during indexing: {err}",
                dir_path = dir_path.display()
            );
            }
        })?;

        let file_path = file.path();

        let dot_file = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with("."))
            .unwrap_or(false);

        if dot_file {
            continue 'next_file;
        }

        let file_type = file.file_type().map_err(|err| {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                    "ERROR: could not determine type of file {file_path}: {err}",
                    file_path = file_path.display()
                );
            }
        })?;
        let last_modified = file
            .metadata()
            .map_err(|err| {
                if env::var("LRS_LOG").is_ok() {
                    eprintln!(
                        "ERROR: could not get the metadata of file {file_path}: {err}",
                        file_path = file_path.display()
                    );
                }
            })?
            .modified()
            .map_err(|err| {
                if env::var("LRS_LOG").is_ok() {
                    eprintln!(
                    "ERROR: could not get the last modification date of file {file_path}: {err}",
                    file_path = file_path.display()
                );
                }
            })?;

        if file_type.is_dir() {
            add_folder_to_model(&file_path, Arc::clone(&model), processed)?;
            continue 'next_file;
        }

        // TODO: how does this work with symlinks?

        let mut model = model.lock().unwrap();
        if model.requires_reindexing(&file_path, last_modified) {
            if env::var("LRS_LOG").is_ok() {
                println!("Indexing {:?}...", &file_path);
            }

            let content = match parse_entire_file_by_extension(&file_path) {
                Ok(content) => content.chars().collect::<Vec<_>>(),
                // TODO: still add the skipped files to the model to prevent their reindexing in the future
                Err(()) => continue 'next_file,
            };

            model.add_document(file_path, last_modified, &content);
            *processed += 1;
        }
    }

    Ok(())
}

fn usage() {
    eprintln!("Local Rust Search");
    eprintln!("Usage: lrs [--help] [directory] ");
}

fn entry() -> Result<(), ()> {
    let mut args = env::args();

    let _ = args.next();
    let option_or_dir = args.next().unwrap_or(String::from(""));

    if option_or_dir.as_str() == "--help" {
        usage();
        Ok(())
    } else {
        println!("Starting index...");
        let dir_path = args.next().unwrap_or(String::from("."));

        let mut index_path = Path::new(&dir_path).to_path_buf();
        index_path.push(".lrs.json");

        let exists = index_path.try_exists().map_err(|err| {
            if env::var("LRS_LOG").is_ok() {
                eprintln!(
                    "ERROR: could not check the existence of file {index_path}: {err}",
                    index_path = index_path.display()
                );
            }
        })?;

        let model: Arc<Mutex<Model>>;
        if exists {
            let index_file = File::open(&index_path).map_err(|err| {
                if env::var("LRS_LOG").is_ok() {
                    eprintln!(
                        "ERROR: could not open index file {index_path}: {err}",
                        index_path = index_path.display()
                    );
                }
            })?;

            model = Arc::new(Mutex::new(serde_json::from_reader(index_file).map_err(
                |err| {
                    if env::var("LRS_LOG").is_ok() {
                        eprintln!(
                            "ERROR: could not parse index file {index_path}: {err}",
                            index_path = index_path.display()
                        );
                    }
                },
            )?));
        } else {
            model = Arc::new(Mutex::new(Default::default()));
        }

        {
            let model = Arc::clone(&model);
            thread::spawn(move || {
                let mut processed = 0;
                // TODO: what should we do in case indexing thread crashes
                add_folder_to_model(Path::new(&dir_path), Arc::clone(&model), &mut processed)
                    .unwrap();
                if processed > 0 {
                    let model = model.lock().unwrap();
                    save_model_as_json(&model, &index_path).unwrap();
                }
                println!("\nFinished indexing!");
            });
        }

        //server::start(&address, Arc::clone(&model))
        tui::start(Arc::clone(&model))
        //Ok(())
    }
}

fn main() -> ExitCode {
    match entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

// TODO: search result must consist of clickable links
// TODO: synonym terms
