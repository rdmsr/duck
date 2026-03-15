use crate::book;
use crate::config::Config;
use crate::parser;
use crate::render;

use std::collections::HashMap;

use tera::Tera;

const MACROS: &str = include_str!("templates/macros.html");
const DOCPAGE_TEMPLATE: &str = include_str!("templates/docpage.html");
const PAGE_TEMPLATE: &str = include_str!("templates/page.html");
const INDEX_TEMPLATE: &str = include_str!("templates/index.html");
const RECORD_TEMPLATE: &str = include_str!("templates/record.html");
const NAMESPACE_TEMPLATE: &str = include_str!("templates/namespace.html");
const FUNCTION_TEMPLATE: &str = include_str!("templates/function.html");
const ENUM_TEMPLATE: &str = include_str!("templates/enum.html");
const SEARCH_TEMPLATE: &str = include_str!("templates/search.html");
const ALIAS_TEMPLATE: &str = include_str!("templates/alias.html");

fn cleanup_type(type_: &str) -> String {
    // Lmao

    let ret = type_
        .replace(
            "struct ",
            "</span><span class=\"storage type c\">struct </span><span class=\"storage type c\">",
        )
        .replace(
            "enum ",
            "</span><span class=\"storage type c\">enum </span><span class=\"storage type c\">",
        )
        .replace(
            "const ",
            "</span><span class=\"storage type c\">const </span><span class=\"storage type c\">",
        );

    return ret;
}

fn tera_output_template(index: HashMap<String, String>, config: Config) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let templ = args.get("template").unwrap().as_object().unwrap();
            let namespace = args.get("namespace").unwrap().as_str().unwrap();

            let mut prefix = String::new();
            prefix.push_str("<span class=\"storage type c\">template</span> &lt;");

            let params = templ.get("parameters").unwrap().as_array().unwrap();
            let params_length = params.len();

            for (i, param) in params.iter().enumerate() {
                let type_ = param.get("type").unwrap().as_str().unwrap();
                prefix.push_str(&format!(
                    "{} {}",
                    get_link_for_type(type_, namespace, &config, &index).unwrap_or(format!(
                        "<span class=\"storage type c\">{}</span>",
                        cleanup_type(type_)
                    )),
                    param.get("name").unwrap().as_str().unwrap()
                ));

                if i < params_length - 1 {
                    prefix.push_str(", ");
                }
            }

            prefix.push_str("&gt; ");

            Ok(tera::to_value(prefix).unwrap())
        },
    )
}

fn tera_get_link_for_namespace(index: HashMap<String, String>) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let namespace = args.get("namespace").unwrap().as_str().unwrap();

            let ret = render::get_path_for_name(namespace, &index, None);

            if ret.is_some() {
                let parts = namespace.split("::").collect::<Vec<_>>();
                let parts_count = parts.len();
                let mut link = String::new();
                let mut acc = String::new();

                for (i, part) in parts.iter().enumerate() {
                    acc.push_str(part);

                    if i != parts_count - 1 {
                        link.push_str(&format!(
                            "<a href=\"/{}/index.html\"><span class=\"storage type c\">{}</span></a>",
                            acc, part
                        ));
                    } else {
                        // Check if parent namespace is actually a record
                        if let Some(entry) = index.get(namespace) {
                            if entry == "record" {
                                link.push_str(&format!(
				    "<a href=\"{}/record.{}.html\"><span class=\"storage type c\">{}</span></a>",
				    if part.len() == acc.len() { "".to_string() } else {
					format!("/{}", &acc[0.. acc.len() - part.len()]).to_string()
				    },
				    part,
				    part
				));
                            } else if entry == "namespace" {
                                link.push_str(&format!(
                                    "<a href=\"/{}/index.html\"><span class=\"storage type c\">{}</span></a>",
                                    acc, part
                                ));
                            }
                        }
                    }

                    if i < parts_count - 1 {
                        link.push_str(" :: ");
                    }

                    acc.push('/');
                }

                Ok(tera::to_value(link).unwrap())
            } else {
                Ok(tera::to_value("".to_string()).unwrap())
            }
        },
    )
}

fn tera_output_struct(index: HashMap<String, String>, config: Config) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let struct_ = args.get("struct").unwrap().as_object().unwrap();
            let namespace = args.get("namespace").unwrap().as_str().unwrap();
            let type_ = args.get("type").unwrap().as_str().unwrap();

            if type_ == "struct" {
                let mut listing = "<span class=\"storage type c\">struct</span> {\n".to_string();

                let fields = struct_
                    .get("Record")
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .get("fields")
                    .unwrap()
                    .as_array()
                    .unwrap();

                let fields_count = fields.len();

                for (i, field) in fields.iter().enumerate() {
                    let field = field.as_object().unwrap();
                    let type_ = field.get("type").unwrap().as_str().unwrap();
                    let name = field.get("name").unwrap().as_str().unwrap();

                    listing.push_str("  ");
                    listing.push_str(&format!(
                        "{} {};",
                        get_link_for_type(type_, namespace, &config, &index).unwrap_or(format!(
                            "<span class=\"storage type c\">{}</span>",
                            cleanup_type(type_)
                        )),
                        name
                    ));

                    if i < fields_count - 1 {
                        listing.push('\n');
                    }
                }

                if fields_count != 0 {
                    listing.push('\n');
                }

                listing.push('}');

                Ok(tera::to_value(listing).unwrap())
            } else if type_ == "enum" {
                let mut listing = "<span class=\"storage type c\">enum</span> {\n".to_string();

                let values = struct_
                    .get("Enum")
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .get("values")
                    .unwrap()
                    .as_array()
                    .unwrap();
                let fields_count = values.len();

                for (i, field) in values.iter().enumerate() {
                    let field = field.as_object().unwrap();
                    let name = field.get("name").unwrap().as_str().unwrap();

                    listing.push_str("  ");
                    listing.push_str(&format!("{};", name));

                    if i < fields_count - 1 {
                        listing.push('\n');
                    }
                }

                if fields_count != 0 {
                    listing.push('\n');
                }

                listing.push('}');

                Ok(tera::to_value(listing).unwrap())
            } else {
                Ok(tera::to_value("".to_string()).unwrap())
            }
        },
    )
}

pub fn init(index: &HashMap<String, String>, config: &Config) -> Tera {
    let mut tera = Tera::default();
    tera.add_raw_templates(vec![
        ("macros", MACROS),
        ("docpage", DOCPAGE_TEMPLATE),
        ("page", PAGE_TEMPLATE),
        ("index", INDEX_TEMPLATE),
        ("record", RECORD_TEMPLATE),
        ("namespace", NAMESPACE_TEMPLATE),
        ("function", FUNCTION_TEMPLATE),
        ("enum", ENUM_TEMPLATE),
        ("search", SEARCH_TEMPLATE),
        ("alias", ALIAS_TEMPLATE),
    ])
    .unwrap();

    tera.register_function(
        "link_for_type",
        tera_get_url_for(index.clone(), config.clone()),
    );
    tera.register_function(
        "output_template",
        tera_output_template(index.clone(), config.clone()),
    );
    tera.register_function(
        "output_struct",
        tera_output_struct(index.clone(), config.clone()),
    );
    tera.register_function(
        "get_link_for_namespace",
        tera_get_link_for_namespace(index.clone()),
    );

    tera
}

pub fn output_function(
    function: &parser::Function,
    pages: &crate::Pages,
    config: &Config,
    summary: &book::Summary,
    tera: &Tera,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = match function.namespace {
        Some(ref ns) => {
            if ns.is_empty() {
                "".to_string()
            } else {
                format!("/{}", render::get_namespace_path(ns))
            }
        }
        None => "".to_string(),
    };

    let mut context = tera::Context::new();

    context.insert("function", function);
    context.insert("pages", &pages);
    context.insert("project", &config.project);
    context.insert("config", &config);
    context.insert("summary", &summary);
    context.insert("page", &pages.index);

    let path = format!(
        "{}/{}/function.{}.html",
        config.output.path,
        path,
        function.name.replace("/", "slash")
    );

    let output = tera.render("function", &context)?;

    std::fs::write(&path, output)?;

    Ok(())
}

fn get_link_for_type(
    name: &str,
    curr_namespace: &str,
    config: &Config,
    index: &HashMap<String, String>,
) -> Option<String> {
    let explicit_kind = if name.contains("struct ") {
        Some("record")
    } else if name.contains("enum ") {
        Some("enum")
    } else {
        None
    };

    let cleaned_name = name
        .trim_start_matches("const ")
        .trim_start_matches("struct ")
        .trim_start_matches("enum ");
    let name_without_suffix = name.trim_matches(|c| c == '&' || c == ' ' || c == '*');
    let suffix = name.trim_start_matches(name_without_suffix).trim();
    let cleaned_name = cleaned_name.trim_matches(|c| c == '&' || c == ' ' || c == '*');

    if name.contains('<') {
        let mut type_name = name.split('<').next().unwrap();
        type_name = type_name.trim();

        let mut ret = String::new();

        let mut type_params = Vec::new();
        let mut depth = 0;
        let mut start = 0;

        for (i, c) in name.chars().enumerate() {
            if c == '<' {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }

            if c == ',' && depth == 1 {
                type_params.push(&name[start..i]);
                start = i + 1;
            } else if c == '>' {
                depth -= 1;
                if depth == 0 {
                    type_params.push(&name[start..i]);
                }
            }
        }

        let mut suffix = name.split('>').last().unwrap_or_default();
        suffix = suffix.trim();

        for (i, param) in type_params.iter().enumerate() {
            let param = param.trim();
            let link = get_link_for_type(param, curr_namespace, config, index);

            if let Some(link) = link {
                ret.push_str(&link);
            } else {
                ret.push_str("<span class=\"storage type c\">");
                ret.push_str(cleanup_type(param).as_str());
                ret.push_str("</span>");
            }

            if i < type_params.len() - 1 {
                ret.push_str(", ");
            }
        }

        return Some(format!(
            "{}&lt;{}&gt;{}",
            get_link_for_type(type_name, curr_namespace, config, index).unwrap_or_else(|| format!(
                "<span class=\"storage type c\">{}</span>",
                cleanup_type(type_name)
            )),
            ret,
            suffix
        ));
    }

    let base_url = config.output.base_url.clone().unwrap_or("".to_string());

    // if name starts with '::', then we must use the global namespace
    if cleaned_name.starts_with("::") {
        let cleaned_name = cleaned_name.trim_start_matches("::");
        let ret = render::get_path_for_name(cleaned_name, index, explicit_kind);

        if let Some(ret) = ret {
            return Some(format!(
                "<a href=\"{}/{}.html\"><span class=\"storage type c\">{}</span></a>{}",
                base_url, ret, name_without_suffix, suffix
            ));
        }
    }

    // First try name in current namespace
    let ret = render::get_path_for_name(
        &format!("{}::{}", curr_namespace, cleaned_name),
        index,
        explicit_kind,
    );

    let name_without_suffix = name_without_suffix
        .replace(
            "struct ",
            "</span><span class=\"storage type c\">struct </span><span class=\"storage type c\">",
        )
        .replace(
            "enum ",
            "</span><span class=\"storage type c\">enum </span><span class=\"storage type c\">",
        )
        .replace(
            "const ",
            "</span><span class=\"storage type c\">const </span><span class=\"storage type c\">",
        );

    if let Some(ret) = ret {
        return Some(format!(
            "<a href=\"{}/{}.html\"><span class=\"storage type c\">{}</span></a>{}",
            base_url, ret, name_without_suffix, suffix
        ));
    }

    // Then try name in global namespace
    let ret = render::get_path_for_name(cleaned_name, index, explicit_kind);

    if let Some(ret) = ret {
        return Some(format!(
            "<a href=\"{}/{}.html\"><span class=\"storage type c\">{}</span></a>{}",
            base_url, ret, name_without_suffix, suffix
        ));
    }

    // If still not found, then try in all parents namespaces
    let mut parts = curr_namespace.split("::").collect::<Vec<_>>();

    while !parts.is_empty() {
        let ret = render::get_path_for_name(
            &format!("{}::{}", parts.join("::"), cleaned_name),
            index,
            explicit_kind,
        );

        if let Some(ret) = ret {
            return Some(format!(
                "<a href=\"{}/{}.html\"><span class=\"storage type c\">{}</span></a>{}",
                base_url, ret, name_without_suffix, suffix
            ));
        }

        parts.pop();
    }

    None
}

fn tera_get_url_for(index: HashMap<String, String>, config: Config) -> impl tera::Function {
    Box::new(
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let the_type = args.get("type").unwrap();
            let namespace = args.get("namespace").unwrap();

            if let Some(parent) = args.get("parent") {
                if let Some(parent) = parent.as_object() {
                    let the_type = the_type.as_str().unwrap();

                    let parent_ns = parent.get("namespace").unwrap().as_str().unwrap();
                    let parent_name = parent.get("name").unwrap().as_str().unwrap();

                    let parent_ns = if parent_ns.is_empty() {
                        "".to_string()
                    } else {
                        format!("{}::", parent_ns)
                    };

                    let namespace = namespace.as_str().unwrap();
                    let namespace = if namespace.is_empty() {
                        "".to_string()
                    } else {
                        format!("{}::", namespace)
                    };

                    let namespace = format!("{}{}{}", namespace, parent_ns, parent_name);

                    // Prioritize parent namespace
                    if let Some(ret) = get_link_for_type(the_type, &namespace, &config, &index) {
                        return Ok(tera::to_value(ret).unwrap());
                    }
                }
            }

            match get_link_for_type(
                the_type.as_str().unwrap(),
                namespace.as_str().unwrap(),
                &config,
                &index,
            ) {
                Some(x) => Ok(tera::to_value(x).unwrap()),
                None => Ok(tera::to_value(format!(
                    "<span class=\"storage type c\">{}</span>",
                    cleanup_type(the_type.as_str().unwrap())
                ))
                .unwrap()),
            }
        },
    )
}

pub fn output_record(
    record: &parser::Record,
    pages: &crate::Pages,
    config: &Config,
    index: &HashMap<String, String>,
    summary: &book::Summary,
    tera: &Tera,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = tera::Context::new();

    let mut prefix = String::new();

    if record.name.starts_with("(unnamed struct") || record.name.starts_with("struct (unnamed") {
        return Ok(());
    }

    if let Some(templ) = &record.template {
        prefix.push_str("<span class=\"storage type c\">template</span> &lt;");

        let params_length = templ.parameters.len();

        for (i, param) in templ.parameters.iter().enumerate() {
            prefix.push_str(&format!(
                "{} {}",
                get_link_for_type(
                    &param.type_,
                    record.namespace.clone().unwrap_or_default().as_str(),
                    config,
                    index
                )
                .unwrap_or(format!(
                    "<span class=\"storage type c\">{}</span>",
                    cleanup_type(&param.type_)
                )),
                param.name
            ));

            if i < params_length - 1 {
                prefix.push_str(", ");
            }
        }

        prefix.push_str("&gt; ");
    }

    let mut listing = format!(
        "{}<span class=\"storage type c\">{}</span> {} {{",
        prefix, record.kind, record.name
    );
    let ns_name = record.namespace.clone().unwrap_or_default();

    if !record.fields.is_empty() {
        listing.push('\n');
    }

    for field in &record.fields {
        if let Some(nested) = &field.struct_ {
            if let parser::NestedField::Record(struct_) = nested {
                listing.push_str("  <span class=\"storage type c\">struct</span> {\n");
                for struct_field in struct_.fields.iter() {
                    listing.push_str("  ");
                    listing.push_str(&format!(
                        "  {} {};\n",
                        get_link_for_type(
                            struct_field.type_.as_str(),
                            ns_name.as_str(),
                            config,
                            index
                        )
                        .unwrap_or(format!(
                            "<span class=\"storage type c\">{}</span>",
                            cleanup_type(&struct_field.type_)
                        )),
                        struct_field.name
                    ));
                }
                listing.push_str(&format!("  }} {};\n", field.name));
                continue;
            } else if let parser::NestedField::Enum(enm) = nested {
                listing.push_str("  <span class=\"storage type c\">enum</span> {\n");
                for enum_field in enm.values.iter() {
                    listing.push_str("  ");
                    listing.push_str(&format!("  {};\n", enum_field.name));
                }
                listing.push_str(&format!("  }} {};\n", field.name));
                continue;
            }
        }
        listing.push_str(&format!(
            "  {} {};\n",
            get_link_for_type(field.type_.as_str(), ns_name.as_str(), config, index).unwrap_or(
                format!(
                    "<span class=\"storage type c\">{}</span>",
                    cleanup_type(&field.type_)
                )
            ),
            field.name
        ));
    }

    if let Some(nested) = &record.nested {
        if !nested.is_empty() {
            // Create a directory to represent nested types
            let path = format!(
                "{}/{}/{}",
                config.output.path,
                render::get_namespace_path(&ns_name),
                record.name
            );

            std::fs::create_dir_all(&path)?;
        }

        for nested_field in nested {
            if let parser::NestedField::Record(rec) = nested_field {
                let mut rec = rec.clone();

                rec.namespace = if ns_name.is_empty() {
                    Some(record.name.clone())
                } else {
                    Some(format!("{}::{}", ns_name, record.name))
                };

                output_record(&rec, pages, config, index, summary, tera)?;
            } else if let parser::NestedField::Enum(enm) = nested_field {
                let mut enm = enm.clone();
                enm.namespace = if ns_name.is_empty() {
                    Some(record.name.clone())
                } else {
                    Some(format!("{}::{}", ns_name, record.name))
                };

                output_enum(&enm, pages, config, summary, tera)?;
            }
        }
    }

    listing.push_str("<span class=\"comment\">  /* Full declaration omitted */ </span>");
    if !record.fields.is_empty() {
        listing.push('\n');
    }
    listing.push('}');

    let listing = format!(
        "<div class=\"code highlight\"><pre><code>{}</code></pre></div>",
        listing
    );

    let path = match record.namespace {
        Some(ref ns) => {
            if ns.is_empty() {
                "".to_string()
            } else {
                format!("/{}", render::get_namespace_path(ns))
            }
        }
        _ => "".to_string(),
    };

    context.insert("record", record);
    context.insert("pages", &pages);
    context.insert("config", &config);
    context.insert("project", &config.project);
    context.insert("listing", &listing);
    context.insert("summary", &summary);
    context.insert("page", &pages.index);

    let output = tera.render("record", &context)?;

    let path = format!(
        "{}/{}/record.{}.html",
        config.output.path, path, record.name
    );

    std::fs::write(&path, output)?;

    Ok(())
}

fn output_alias(
    alias: &parser::Alias,
    pages: &crate::Pages,
    config: &Config,
    index: &HashMap<String, String>,
    summary: &book::Summary,
    tera: &Tera,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = tera::Context::new();

    let ns_name = alias.namespace.clone().unwrap_or_default();

    println!("{:?}", cleanup_type(&alias.type_));

    let listing = format!(
        "<span class=\"storage type c\">using</span> {} = {}",
        alias.name,
        get_link_for_type(alias.type_.as_str(), &ns_name, config, index).unwrap_or(format!(
            "<span class=\"storage type c\">{}</span>",
            cleanup_type(&alias.type_)
        )),
    );

    let listing = format!(
        "<div class=\"code highlight\"><pre><code>{}</code></pre></div>",
        listing
    );

    let path = match alias.namespace {
        Some(ref ns) => {
            if ns.is_empty() {
                "".to_string()
            } else {
                format!("/{}", render::get_namespace_path(ns))
            }
        }
        None => "".to_string(),
    };

    context.insert("alias", alias);
    context.insert("pages", &pages);
    context.insert("config", &config);
    context.insert("project", &config.project);
    context.insert("listing", &listing);
    context.insert("summary", &summary);
    context.insert("page", &pages.index);

    let output = tera.render("alias", &context)?;

    let path = format!("{}/{}/alias.{}.html", config.output.path, path, alias.name);

    std::fs::write(&path, output)?;

    Ok(())
}

fn output_enum(
    enum_: &parser::Enum,
    pages: &crate::Pages,
    config: &Config,
    summary: &book::Summary,
    tera: &Tera,
) -> Result<(), Box<dyn std::error::Error>> {
    if enum_.name.starts_with("(unnamed enum") || enum_.name.starts_with("enum (unnamed") {
        return Ok(());
    }

    let mut context = tera::Context::new();

    let mut listing = format!(
        "<span class=\"storage type c\">enum</span> {} {{",
        enum_.name
    );

    let value_cnt = enum_.values.len();

    if value_cnt != 0 {
        listing.push('\n');
    }

    for (i, value) in enum_.values.iter().enumerate() {
        listing.push_str("  ");
        listing.push_str(&value.name);

        if i < value_cnt - 1 {
            listing.push_str(",\n");
        }
    }
    if value_cnt != 0 {
        listing.push('\n');
    }
    listing.push_str("};");

    let listing = format!(
        "<div class=\"code highlight\"><pre><code>{}</code></pre></div>",
        listing
    );

    let path = match enum_.namespace {
        Some(ref ns) => {
            if ns.is_empty() {
                "".to_string()
            } else {
                format!("/{}", render::get_namespace_path(ns))
            }
        }
        None => "".to_string(),
    };

    context.insert("enum", enum_);
    context.insert("pages", &pages);
    context.insert("config", &config);
    context.insert("project", &config.project);
    context.insert("listing", &listing);
    context.insert("summary", &summary);
    context.insert("page", &pages.index);

    let output = tera.render("enum", &context)?;

    let path = format!("{}/{}/enum.{}.html", config.output.path, path, enum_.name);
    std::fs::write(&path, output)?;

    Ok(())
}

pub fn output_namespace(
    namespace: &parser::Namespace,
    pages: &crate::Pages,
    config: &Config,
    index: &HashMap<String, String>,
    summary: &book::Summary,
    tera: &Tera,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = tera::Context::new();

    let mut path = match namespace.namespace {
        Some(ref ns) => {
            if ns.is_empty() {
                "".to_string()
            } else {
                format!("/{}", render::get_namespace_path(ns))
            }
        }
        _ => "".to_string(),
    };

    context.insert("namespace", namespace);
    context.insert("config", &config);
    context.insert("project", &config.project);
    context.insert("pages", &pages);
    context.insert("summary", &summary);
    context.insert("page", &pages.index);

    let index_ns_name = config.output.root_namespace.as_deref().unwrap_or_default();
    let is_root = namespace.name.is_empty() || namespace.name == index_ns_name;

    if is_root {
        context.insert("content", &pages.index.content);
        path = "".to_string();
    }

    let output = tera.render(if is_root { "index" } else { "namespace" }, &context)?;

    let path = format!("{}/{}", config.output.path, path);

    std::fs::create_dir_all(format!("{}/{}", path, namespace.name))?;

    let path = format!(
        "{}/{}/index.html",
        path,
        if is_root {
            "".to_string()
        } else {
            namespace.name.to_string()
        }
    );

    std::fs::write(&path, output)?;

    for record in &namespace.records {
        output_record(record, pages, config, index, summary, tera)?;
    }

    for function in &namespace.functions {
        output_function(function, pages, config, summary, tera)?;
    }

    for enm in &namespace.enums {
        output_enum(enm, pages, config, summary, tera)?;
    }

    for alias in &namespace.aliases {
        output_alias(alias, pages, config, index, summary, tera)?;
    }

    for ns in &namespace.namespaces {
        output_namespace(ns, pages, config, index, summary, tera)?;
    }

    Ok(())
}
