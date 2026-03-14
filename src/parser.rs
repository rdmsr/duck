use crate::comment;
use crate::config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Comment {
    pub description: String,
    pub brief: String,
    #[serde(rename = "impl")]
    pub impl_: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NestedField {
    Record(Record),
    Enum(Enum),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub comment: Option<Comment>,

    #[serde(rename = "struct")]
    pub struct_: Option<NestedField>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Record {
    pub name: String,
    pub fields: Vec<Field>,
    pub comment: Option<Comment>,
    pub kind: String,
    pub namespace: Option<String>,
    pub ctor: Vec<Function>,
    pub methods: Vec<Function>,
    pub template: Option<Template>,
    pub nested: Option<Vec<NestedField>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnumValue {
    pub name: String,
    pub comment: Option<Comment>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub comment: Option<Comment>,
    pub namespace: Option<String>,
    pub values: Vec<EnumValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionProps {
    #[serde(rename = "const")]
    pub const_: bool,
    #[serde(rename = "static")]
    pub static_: bool,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TemplateParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Template {
    pub parameters: Vec<TemplateParameter>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Function {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<Field>,
    pub comment: Option<Comment>,
    pub props: FunctionProps,
    pub namespace: Option<String>,
    pub template: Option<Template>,
    pub overloads: Option<Vec<Function>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alias {
    pub namespace: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub comment: Option<Comment>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Namespace {
    pub name: String,
    pub comment: Option<Comment>,
    pub records: Vec<Record>,
    pub functions: Vec<Function>,
    pub namespaces: Vec<Namespace>,
    pub enums: Vec<Enum>,
    pub aliases: Vec<Alias>,
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Output {
    pub root: Namespace,
    pub index: HashMap<String, String>,
}

pub struct Parser<'a> {
    index: clang::Index<'a>,
}

impl Output {
    fn merge_namespace(ns: &mut Namespace, other: Namespace) {
        // Merge functions, records, enums, aliases directly
        ns.functions.extend(other.functions);
        ns.records.extend(other.records);
        ns.enums.extend(other.enums);
        ns.aliases.extend(other.aliases);

        // Merge nested namespaces
        for other_sub in other.namespaces {
            if let Some(existing_sub) = ns.namespaces.iter_mut().find(|n| n.name == other_sub.name)
            {
                Self::merge_namespace(existing_sub, other_sub);
            } else {
                ns.namespaces.push(other_sub);
            }
        }
    }

    pub fn merge(&mut self, other: Output) {
        Self::merge_namespace(&mut self.root, other.root);
        self.index.extend(other.index);
    }
}

impl<'a> Parser<'a> {
    pub fn new(clang: &'a clang::Clang) -> Self {
        let index = clang::Index::new(clang, false, false);
        Parser { index }
    }

    fn parse_template(&self, node: clang::Entity) -> Template {
        Template {
            parameters: node
                .get_children()
                .iter()
                .filter(|c| {
                    c.get_kind() == clang::EntityKind::TemplateTypeParameter
                        || c.get_kind() == clang::EntityKind::NonTypeTemplateParameter
                        || c.get_kind() == clang::EntityKind::TemplateTemplateParameter
                })
                .map(|c| TemplateParameter {
                    name: c.get_name().unwrap_or_default(),
                    type_: match c.get_kind() {
                        clang::EntityKind::TemplateTypeParameter => "typename".to_string(),
                        clang::EntityKind::NonTypeTemplateParameter => {
                            c.get_type().unwrap().get_display_name()
                        }
                        clang::EntityKind::TemplateTemplateParameter => "template".to_string(),
                        _ => unreachable!(),
                    },
                })
                .collect(),
        }
    }

    fn parse_function(&self, node: clang::Entity) -> Option<Function> {
        let mut ret = Function {
            name: node.get_name().unwrap(),
            return_type: node.get_result_type().unwrap().get_display_name(),
            parameters: Vec::new(),
            comment: None,
            props: FunctionProps {
                const_: node.is_const_method(),
                static_: node.is_static_method(),
                virtual_: node.is_virtual_method(),
            },
            namespace: None,
            template: None,
            overloads: None,
        };

        // Handle function names with quotes, like operator"", so that links don't fuck up
        ret.name = ret.name.replace("\"", "&quot");

        if let Some(c) = node.get_comment() {
            ret.comment = comment::parse_comment(c);
            if ret.comment.is_none() {
                return None;
            }
        }

        for c in node
            .get_children()
            .iter()
            .filter(|c| c.get_kind() == clang::EntityKind::ParmDecl)
        {
            let field = Field {
                name: c.get_name().unwrap_or_default(),
                type_: c.get_type().unwrap().get_display_name(),
                comment: None,
                struct_: None,
            };
            ret.parameters.push(field);
        }

        if node.get_kind() == clang::EntityKind::FunctionTemplate {
            ret.template = Some(self.parse_template(node));
        }

        Some(ret)
    }

    fn parse_record(&self, node: clang::Entity) -> Option<Record> {
        let mut ret = Record {
            name: node.get_name().unwrap(),
            fields: Vec::new(),
            comment: None,
            kind: match node.get_kind() {
                clang::EntityKind::StructDecl => "struct".to_string(),
                clang::EntityKind::ClassDecl => "class".to_string(),
                clang::EntityKind::ClassTemplate => "class".to_string(),
                clang::EntityKind::UnionDecl => "union".to_string(),
                _ => {
                    println!("Unexpected record child kind: {:?}", node.get_kind());
                    unreachable!()
                }
            },
            namespace: None,
            ctor: Vec::new(),
            methods: Vec::new(),
            template: None,
            nested: None,
        };

        if let Some(c) = node.get_comment() {
            ret.comment = comment::parse_comment(c);
            if ret.comment.is_none() {
                return None;
            }
        }

        if node.get_kind() == clang::EntityKind::ClassTemplate {
            ret.template = Some(self.parse_template(node));
        }

        if node.get_children().len() == 0 {
            return None;
        }

        for c in node.get_children().iter() {
            match c.get_kind() {
                clang::EntityKind::FieldDecl => {
                    if let Some(clang::Accessibility::Public) = c.get_accessibility() {
                        let comment = if let Some(comment) = c.get_comment() {
                            comment::parse_comment(comment)
                        } else {
                            None
                        };

                        let mut field = Field {
                            name: c.get_name().unwrap_or_default(),
                            type_: c.get_type().unwrap().get_display_name(),
                            comment,
                            struct_: None,
                        };

                        // NOTE: We assume that unnamed struct types always have "(unnamed struct" in their name
                        if field.type_.contains("(unnamed struct") {
                            let ret_struct = self.parse_record(
                                *c.get_children()
                                    .iter()
                                    .find(|c| c.get_kind() == clang::EntityKind::StructDecl)
                                    .unwrap(),
                            )?;

                            field.type_ = "struct".to_string();
                            field.struct_ = Some(NestedField::Record(ret_struct));
                        }

                        if field.type_.contains("(unnamed union") {
                            let ret_struct = self.parse_record(
                                *c.get_children()
                                    .iter()
                                    .find(|c| c.get_kind() == clang::EntityKind::UnionDecl)
                                    .unwrap(),
                            )?;

                            field.type_ = "union".to_string();
                            field.struct_ = Some(NestedField::Record(ret_struct));
                        }

                        if field.type_.contains("(unnamed enum") {
                            let ret_enum = self.parse_enum(
                                *c.get_children()
                                    .iter()
                                    .find(|c| c.get_kind() == clang::EntityKind::EnumDecl)
                                    .unwrap(),
                            )?;

                            field.type_ = "enum".to_string();
                            field.struct_ = Some(NestedField::Enum(ret_enum));
                        }

                        ret.fields.push(field);
                    }
                }

                clang::EntityKind::Constructor => {
                    let mut function = self.parse_function(*c)?;
                    function.return_type = "".to_string();

                    ret.ctor.push(function);
                }

                clang::EntityKind::Method | clang::EntityKind::FunctionTemplate => {
                    if let Some(clang::Accessibility::Public) = c.get_accessibility() {
                        let mut function = self.parse_function(*c)?;
                        function.namespace = Some(ret.name.clone());

                        ret.methods.push(function);
                    }
                }

                clang::EntityKind::StructDecl
                | clang::EntityKind::ClassDecl
                | clang::EntityKind::UnionDecl
                | clang::EntityKind::ClassTemplate => {
                    if let Some(mut record) = self.parse_record(*c) {
                        if !record.name.contains("(anonymous") && !record.name.contains("(unnamed")
                        {
                            record.namespace = Some(ret.name.clone());

                            if ret.nested.is_none() {
                                ret.nested = Some(Vec::new());
                            } else if let Some(nested) = ret.nested.as_mut() {
                                nested.push(NestedField::Record(record));
                            }
                        }
                    }
                }

                clang::EntityKind::EnumDecl => {
                    let mut enum_ = self.parse_enum(*c)?;

                    if !enum_.name.contains("(anonymous") && !enum_.name.contains("(unnamed") {
                        enum_.namespace = Some(ret.name.clone());

                        if ret.nested.is_none() {
                            ret.nested = Some(Vec::new());
                        }

                        if let Some(nested) = ret.nested.as_mut() {
                            nested.push(NestedField::Enum(enum_));
                        }
                    }
                }

                _ => {}
            }
        }

        Some(ret)
    }

    fn parse_enum(&self, node: clang::Entity) -> Option<Enum> {
        let mut ret = Enum {
            name: node.get_name().unwrap(),
            comment: None,
            namespace: None,
            values: Vec::new(),
        };

        if ret.name.contains("(unnamed") {
            return None;
        }

        if let Some(c) = node.get_comment() {
            ret.comment = comment::parse_comment(c);
            if ret.comment.is_none() {
                return None;
            }
        }

        for c in node.get_children().iter() {
            if c.get_kind() == clang::EntityKind::EnumConstantDecl {
                let comment = if let Some(comment) = c.get_comment() {
                    comment::parse_comment(comment)
                } else {
                    None
                };

                let value = EnumValue {
                    name: c.get_name().unwrap_or_default(),
                    comment,
                };

                ret.values.push(value);
            }
        }

        Some(ret)
    }

    fn get_name_for_namespace(name: &str, namespace_name: &str, ns_name_full: &str) -> String {
        if !ns_name_full.is_empty() {
            return format!("{}::{}", ns_name_full, name);
        }

        if !namespace_name.is_empty() {
            format!("{}::{}", namespace_name, name)
        } else {
            name.to_string()
        }
    }

    fn parse_node(
        &self,
        node: clang::Entity,
        ns: &mut Namespace,
        index: &mut HashMap<String, String>,
        current_namespace_name: &str,
    ) {
        let absolute_name = Self::get_name_for_namespace(
            node.get_name().unwrap_or_default().as_str(),
            ns.name.as_str(),
            current_namespace_name,
        );

        match node.get_kind() {
            clang::EntityKind::FunctionDecl | clang::EntityKind::FunctionTemplate => {
                let Some(mut function) = self.parse_function(node) else {
                    return;
                };

                function.namespace = Some(current_namespace_name.to_string());

                if let Some(existing) = ns.functions.iter_mut().find(|f| f.name == function.name) {
                    if existing.overloads.is_none() {
                        existing.overloads = Some(Vec::new());
                    }

                    existing.overloads.as_mut().unwrap().push(function);
                    return;
                }

                if function.name.contains("deduction guide") {
                    return;
                }

                index.insert(absolute_name, "function".to_string());

                ns.functions.push(function);
            }

            clang::EntityKind::StructDecl
            | clang::EntityKind::ClassDecl
            | clang::EntityKind::UnionDecl
            | clang::EntityKind::ClassTemplate => {
                let Some(mut record) = self.parse_record(node) else {
                    return;
                };

                record.namespace = Some(current_namespace_name.to_string());

                // If a record already exists, it must be some kind of template specialization/overloading,
                // We don't really support template specialization/overloading, so we just ignore it and merge all methods.
                if let Some(existing) = ns.records.iter_mut().find(|r| r.name == record.name) {
                    existing.methods.append(&mut record.methods);
                    return;
                }

                if let Some(nest) = &mut record.nested {
                    for nested in nest {
                        match nested {
                            NestedField::Record(r) => {
                                let current_namespace_name = if current_namespace_name.is_empty() {
                                    record.name.clone()
                                } else {
                                    format!("{}::{}", current_namespace_name, record.name)
                                };

                                r.namespace = Some(current_namespace_name.to_string());

                                index.insert(
                                    Self::get_name_for_namespace(
                                        r.name.as_str(),
                                        record.name.as_str(),
                                        &current_namespace_name,
                                    ),
                                    "record".to_string(),
                                );
                            }
                            NestedField::Enum(e) => {
                                let current_namespace_name = if current_namespace_name.is_empty() {
                                    record.name.clone()
                                } else {
                                    format!("{}::{}", current_namespace_name, record.name)
                                };

                                e.namespace = Some(current_namespace_name.to_string());

                                index.insert(
                                    Self::get_name_for_namespace(
                                        e.name.as_str(),
                                        record.name.as_str(),
                                        &current_namespace_name,
                                    ),
                                    "enum".to_string(),
                                );
                            }
                        }
                    }
                }

                index.insert(absolute_name, "record".to_string());
                ns.records.push(record);
            }

            clang::EntityKind::EnumDecl => {
                let Some(mut enum_) = self.parse_enum(node) else {
                    return;
                };

                enum_.namespace = Some(current_namespace_name.to_string());

                index.insert(absolute_name, "enum".to_string());
                ns.enums.push(enum_);
            }

            clang::EntityKind::Namespace => {
                if let Some(name) = node.get_name() {
                    let comment = if let Some(comment) = node.get_comment() {
                        comment::parse_comment(comment)
                    } else {
                        None
                    };

                    let mut real_ns = Namespace {
                        name: node.get_name().unwrap(),
                        comment,
                        records: Vec::new(),
                        functions: Vec::new(),
                        namespaces: Vec::new(),
                        enums: Vec::new(),
                        aliases: Vec::new(),
                        namespace: Some(current_namespace_name.to_string()),
                    };

                    let mut already_exists = false;

                    let new_ns = if let Some(existing_ns) =
                        ns.namespaces.iter_mut().find(|n| n.name == name)
                    {
                        already_exists = true;
                        existing_ns
                    } else {
                        &mut real_ns
                    };

                    index.insert(absolute_name, "namespace".to_string());

                    for cursor in node.get_children() {
                        if !current_namespace_name.is_empty() {
                            self.parse_node(
                                cursor,
                                new_ns,
                                index,
                                format!("{}::{}", current_namespace_name, name.as_str()).as_str(),
                            );
                        } else {
                            self.parse_node(cursor, new_ns, index, name.as_str());
                        }
                    }

                    if !already_exists {
                        ns.namespaces.push(real_ns);
                    }
                }
            }

            clang::EntityKind::TypedefDecl | clang::EntityKind::TypeAliasDecl => {
                let mut type_ = String::new();
                let mut templated = false;

                let children = node.get_children();
                let child_count = children.len();

                let is_function_pointer = children
                    .iter()
                    .any(|c| c.get_kind() == clang::EntityKind::ParmDecl);

                if !is_function_pointer {
                    // Wow this sucks
                    for (i, c) in children.iter().enumerate() {
                        if c.get_kind() == clang::EntityKind::TypeRef {
                            if let Some(t) = c.get_typedef_underlying_type() {
                                type_.push_str(&t.get_display_name());
                            } else {
                                let display_name = c.get_display_name().unwrap();
                                let display_name = display_name.trim_start_matches("struct ");
                                type_.push_str(display_name);
                            }
                        } else if c.get_kind() == clang::EntityKind::TemplateRef {
                            templated = true;
                            type_ = c.get_display_name().unwrap();
                            type_.push('<');
                        }

                        if templated && i < child_count - 1 && i != 0 {
                            type_.push(',');
                        }
                    }

                    if templated {
                        type_.push('>');
                    }
                }

                if type_.is_empty() || is_function_pointer {
                    type_ = node
                        .get_typedef_underlying_type()
                        .unwrap()
                        .get_display_name();
                }

                let comment = if let Some(comment) = node.get_comment() {
                    comment::parse_comment(comment)
                } else {
                    None
                };

                let alias = Alias {
                    namespace: Some(current_namespace_name.to_string()),
                    name: node.get_name().unwrap(),
                    type_,
                    comment,
                };

                index.insert(absolute_name, "alias".to_string());
                ns.aliases.push(alias);
            }

            clang::EntityKind::ConceptDecl => {
                println!("warning: concepts are not supported");
            }

            _ => {}
        }
    }

    pub fn parse(&mut self, config: &config::Config, file: &str, out: &mut Output) {
        let tu = self
            .index
            .parser(file)
            .arguments(&config.input.compiler_arguments)
            .parse()
            .unwrap();

        let main_file = tu.get_file(file).unwrap();

        for cursor in tu.get_entity().get_children() {
            let location = cursor.get_location();

            if let Some(loc) = location {
                // Check both the spelling location (normal) and expansion location (macros)
                let in_main = loc
                    .get_file_location()
                    .file
                    .as_ref()
                    .map(|f| *f == main_file)
                    .unwrap_or(false);

                let in_main_expansion = loc
                    .get_expansion_location()
                    .file
                    .as_ref()
                    .map(|f| *f == main_file)
                    .unwrap_or(false);

                if in_main || in_main_expansion {
                    self.parse_node(cursor, &mut out.root, &mut out.index, "");
                }
            }
        }
    }
}
