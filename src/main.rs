use clap::{Parser, Subcommand};
use glob::glob;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use render::get_path_for_name;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::{path::Path, time::Duration};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::css_for_theme_with_class_style;
use syntect::html::ClassStyle;

mod book;
mod comment;
mod config;
mod doctest;
mod parser;
mod render;
mod report;
mod templates;

use report::{report_error, report_warning};

use syntect::dumps::{from_binary, from_dump_file};
use syntect::parsing::SyntaxSet;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Serialize)]
struct Pages {
    index: render::Page,
    extra: Vec<render::Page>,
}

#[derive(Serialize)]
struct SearchIndex {
    id: i32,
    name: String,
    link: String,
    kind: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[clap(name = "init", about = "Generates a sample duck configuration")]
    Init,

    #[clap(name = "build", about = "Build documentation for the project")]
    Build {
        /// Dump JSON output
        #[clap(short, long)]
        dump_json: bool,

        /// Configuration file to use
        #[arg(short, long, default_value = "duck.toml", value_name = "FILE")]
        config_file: Option<String>,

        /// Cached JSON output to use
        #[arg(long, value_name = "FILE")]
        cached: Option<String>,
    },
}

const THEME_DUMP: &[u8] = include_bytes!("../assets/all.themedump");

fn render_page(
    page: &book::Page,
    index: &HashMap<String, String>,
    doctests: &mut Vec<doctest::Doctest>,
    config: &config::Config,
    highlight_state: &render::HighlightState,
) -> Vec<render::Page> {
    let mut ret = vec![];

    match std::fs::read_to_string(&page.path) {
        Ok(source) => {
            let mut page_ret =
                render::process_markdown(&source, &index, doctests, &config, highlight_state);

            page_ret.title = page.name.clone();
            page_ret.path = Path::new(&page.path).to_path_buf();

            for sub_page in &page.sub_pages {
                ret.extend(render_page(
                    &sub_page,
                    &index,
                    doctests,
                    &config,
                    highlight_state,
                ));
            }

            ret.push(page_ret);

            ret
        }
        Err(e) => {
            report_warning(&format!("Error reading page \"{0}\": {e}", page.path));
            vec![]
        }
    }
}

thread_local! {
    static THREAD_PARSER: RefCell<parser::Parser<'static>> = {
        let clang = Box::new(clang::Clang::new().unwrap());
        let clang_ref: &'static clang::Clang = Box::leak(clang);
        let parser = parser::Parser::new(clang_ref);
        RefCell::new(parser)
    };
}

fn init_new_project() {
    let mut file = match std::fs::File::create_new("duck.toml") {
        Ok(f) => f,
        Err(e) => {
            report_error(&format!("writing config file: {}", e));
            std::process::exit(1);
        }
    };

    let base_config = "[project]
name = \"Project\"
version = \"0.1.0\"

[input]
# Unix-style glob of files where API reference documentation is present
glob = \"**/*.h\"
# Additional compiler arguments, add -xc++ for C++ support
compiler_arguments = []

# [pages]
# Uncomment this to generate an index page from a markdown file
# index = \"README.md\"

# Uncomment this to generate a book from a directory
#book = \"pages\"

[output]
# Static directory that is copied over to the output directory
static_dir = \"static\"
# Output directory
path = \"docs\"

# Base URL
base_url = \"\"
";

    file.write_all(base_config.as_bytes()).unwrap();

    println!("duck initialized in current directory");
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Build {
            dump_json,
            config_file,
            cached,
        } => {
            let config_file = config_file.unwrap_or("duck.toml".to_string());
            let m = MultiProgress::new();

            let config = match config::Config::new(&config_file) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Error reading config file: {}", e);
                    std::process::exit(1);
                }
            };

            let mut output: parser::Output = Default::default();

            if !dump_json {
                println!("\n{:>3}quack! >o)\n{:>10}(_>\n{:>10}duck", " ", " ", " ");
            }

            if cached.is_none() {
                let files: Vec<_> = glob(&config.input.glob)
                    .expect("Failed to read glob pattern")
                    .filter_map(Result::ok)
                    .collect();

                let bar = m.add(ProgressBar::new_spinner());

                // Run the parser in parallel and collect all the results
                let results: Vec<parser::Output> = files
                    .par_iter()
                    .map(|file| {
                        let mut output = parser::Output::default();
                        THREAD_PARSER.with(|state| {
                            let mut state = state.borrow_mut();
                            let parser = &mut *state;

                            if !dump_json {
                                bar.set_message(format!("Parsing {}", file.to_string_lossy()));
                            }

                            parser.parse(&config, file.to_string_lossy().as_ref(), &mut output);

                            bar.tick();
                        });

                        output
                    })
                    .collect();

                // Merge the results
                let mut final_output = parser::Output::default();

                for r in results {
                    final_output.merge(r);
                }

                output = final_output;

                if !dump_json {
                    bar.finish_with_message("Parsing complete");
                }
            } else {
                let data = std::fs::read_to_string(cached.unwrap()).expect("Unable to read file");
                match serde_json::from_str::<parser::Output>(&data) {
                    Ok(out) => output = out,
                    Err(e) => {
                        report_error(&format!("Error reading cached database: {e:}"));
                    }
                }
            }

            if dump_json {
                let json = serde_json::to_string(&output).unwrap();
                println!("{}", json);
                return;
            }

            let mut highlight_state = render::HighlightState {
                syntax_set: SyntaxSet::load_defaults_newlines(),
                theme_set: Default::default(),
            };

            let mut theme: Theme;

            if let Some(ref p) = config.output.theme_dump_file {
                highlight_state.theme_set = match from_dump_file(&p) {
                    Ok(s) => s,
                    Err(e) => {
                        report_error(&format!("Could not load theme set: {}", e));
                        std::process::exit(1);
                    }
                }
            } else {
                highlight_state.theme_set = from_binary(THEME_DUMP);
            }

            if let Some(ref t) = config.output.theme {
                if !highlight_state.theme_set.themes.contains_key(t) {
                    report_error(&format!("Could not find theme {}", t));
                    std::process::exit(1);
                }

                theme = highlight_state.theme_set.themes[t].clone();
            } else if let Some(ref t) = config.output.theme_file {
                highlight_state.theme_set = ThemeSet::new();

                theme = match ThemeSet::get_theme(t) {
                    Ok(x) => x,
                    Err(e) => {
                        report_error(&format!("Could not load theme: {}", e));
                        std::process::exit(1);
                    }
                }
            } else {
                theme = highlight_state.theme_set.themes["one-dark"].clone();
            }

            for style in theme.scopes.iter_mut() {
                if let Some(_) = style.style.font_style {
                    style.style.font_style = None;
                }
            }

            let root_namespace = if let Some(ref root_namespace) = config.output.root_namespace {
                // Find namespace
                output
                    .root
                    .namespaces
                    .iter_mut()
                    .find(|ns| ns.name == *root_namespace)
                    .expect("Invalid root namespace name")
            } else {
                &mut output.root
            };

            let mut doctests = Vec::new();

            render::process_namespace(
                root_namespace,
                &output.index,
                &mut doctests,
                &config,
                &highlight_state,
            );

            let pages_config = config.pages.as_ref();

            let index = match pages_config.and_then(|p| p.index.as_ref()) {
                Some(x) => match std::fs::read_to_string(x) {
                    Ok(s) => s,
                    Err(e) => {
                        report_error(&format!("Could not read index file: {}", e));
                        "".into()
                    }
                },
                None => match root_namespace.comment {
                    Some(ref comment) => comment.description.clone(),
                    None => String::new(),
                },
            };

            let mut index_html = render::process_markdown(
                &index,
                &output.index,
                &mut doctests,
                &config,
                &highlight_state,
            );

            index_html.path = "index.html".into();

            let mut extra_pages = Vec::new();

            let mut summary: book::Summary = Default::default();

            if let Some(book_dir) = pages_config.and_then(|p| p.book.as_ref()) {
                let path = std::path::Path::new(book_dir).join("SUMMARY.md");
                let contents = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => {
                        report_error(&format!("Could not find book dir summary file: {:?}", path));
                        "".into()
                    }
                };
                summary = book::parse_summary(&contents, book_dir);
            }

            for segment in &summary.segments {
                if let book::Segment::Page(p) = segment {
                    extra_pages.extend(render_page(
                        &p,
                        &output.index,
                        &mut doctests,
                        &config,
                        &highlight_state,
                    ));
                }
            }

            let pages = Pages {
                index: index_html,
                extra: extra_pages,
            };

            if let Some(ref doctest_conf) = config.doctests {
                if doctest_conf.enable {
                    let bar = m.add(ProgressBar::new(doctests.len() as u64));

                    bar.set_style(
                        ProgressStyle::with_template("Running doctest {pos}/{len}").unwrap(),
                    );

                    if let None = doctest_conf.run {
                        report_error("Doctests enabled but no run option specified");
                        std::process::exit(1);
                    }

                    if let None = doctest_conf.compiler_invocation {
                        report_error("Doctests enabled but no compiler invocation specified");
                        std::process::exit(1);
                    }

                    for doc in doctests {
                        let out = doc.compile(doctest_conf);

                        if doctest_conf.run.unwrap() {
                            doc.run(out);
                        }

                        bar.inc(1);
                    }

                    bar.finish_and_clear();
                }
            }

            // Make directories
            std::fs::create_dir_all(&config.output.path)
                .map_err(|e| {
                    report_error(&format!("Error creating output directory: {}", e));
                    std::process::exit(1);
                })
                .unwrap();

            for page in &pages.extra {
                let path = Path::new(&config.output.path)
                    .join(page.path.parent().unwrap_or_else(|| &Path::new("")));
                std::fs::create_dir_all(path)
                    .map_err(|e| {
                        report_error(&format!("Error creating output directory: {}", e));
                        std::process::exit(1);
                    })
                    .unwrap();
            }

            let tera = templates::init(&output.index, &config);
            let mut context = tera::Context::new();

            context.insert("config", &config);
            context.insert("project", &config.project);
            context.insert("pages", &pages);
            context.insert("summary", &summary);

            let bar = m.add(ProgressBar::new_spinner());

            for page in &pages.extra {
                context.insert("content", &page.content);
                context.insert("title", &page.title);
                context.insert("page", &page);

                let mut out_path = Path::new(&config.output.path).join(&page.path);

                out_path.set_extension("md.html");

                bar.set_message(format!("Rendering page {}", page.path.to_str().unwrap()));

                std::fs::write(&out_path, tera.render("docpage", &context).unwrap())
                    .map_err(|e| {
                        report_error(&format!(
                            "Error writing extra page file {}: {}",
                            out_path.display(),
                            e
                        ));
                        std::process::exit(1);
                    })
                    .unwrap();
                bar.tick();
            }

            bar.finish_with_message("Pages rendered");

            context.insert("page", &pages.index);

            std::fs::write(
                format!("{}/search.html", config.output.path),
                tera.render("search", &context).unwrap(),
            )
            .map_err(|e| {
                report_error(&format!("Error writing search page file: {}", e));
                std::process::exit(1);
            })
            .unwrap();

            let bar = ProgressBar::new_spinner();
            bar.enable_steady_tick(Duration::from_millis(100));
            bar.set_message("Rendering root namespace");

            if let Err(e) = templates::output_namespace(
                root_namespace,
                &pages,
                &config,
                &output.index,
                &summary,
                &tera,
            ) {
                report_error(&format!("Could not render root namespace: {}", e));
            }

            bar.finish_and_clear();

            match std::fs::read_dir(&config.output.static_dir) {
                Ok(dir) => {
                    // Copy everything in the static directory to the output directory
                    for entry in dir {
                        let entry = entry.unwrap();
                        let path = entry.path();
                        let filename = path.file_name().unwrap();
                        let dest = format!("{}/{}", config.output.path, filename.to_str().unwrap());
                        std::fs::copy(&path, &dest).unwrap();
                    }
                }

                Err(e) => {
                    report_error(&format!("Could not copy static directory: {}", e));
                }
            }

            // Make a new, more searchable index
            let mut id: i32 = 0;
            let mut index = Vec::new();

            for item in &output.index {
                index.push(SearchIndex {
                    id,
                    name: item.0.clone().replace("\"", "&quot;"),
                    link: match item.1.as_str() {
                        "namespace" => {
                            format!(
                                "{}/index",
                                get_path_for_name(item.0, &output.index, None).unwrap_or_default()
                            )
                        }
                        _ => get_path_for_name(item.0, &output.index, None).unwrap_or_default(),
                    }
                    .replace("\"", "&quot;")
                    .to_string(),

                    kind: item.1.clone(),
                });

                id += 1;
            }

            // Add pages to the search index
            for page in &pages.extra {
                index.push(SearchIndex {
                    id,
                    name: page.title.clone(),
                    link: page.path.to_string_lossy().into_owned(),
                    kind: "page".to_string(),
                });

                id += 1;
            }

            let index_json = serde_json::to_string(&index).unwrap();

            std::fs::write(
                format!("{}/search_index.json", config.output.path),
                index_json,
            )
            .unwrap();

            // Generate syntax highlighting CSS file
            let css_file =
                std::fs::File::create(Path::new(&config.output.path).join("highlight.css"))
                    .unwrap();

            let mut css_dark_writer = BufWriter::new(&css_file);

            match css_for_theme_with_class_style(&theme, ClassStyle::Spaced) {
                Ok(s) => {
                    writeln!(css_dark_writer, "{}", s).unwrap();
                }

                Err(e) => {
                    report_error(&format!(
                        "Could not generate syntax highlighting file: {}",
                        e
                    ));

                    std::process::exit(1);
                }
            }

            println!("Documentation generated in {}", config.output.path);
        }

        Commands::Init => {
            init_new_project();
        }
    }
}
