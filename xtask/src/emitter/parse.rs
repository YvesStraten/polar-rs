use crate::emitter::ir::*;
use serde_json::Value;

pub fn parse(spec: &Value) -> Ir {
    let mut types = Vec::new();
    if let Some(schemas) = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
    {
        for (name, schema) in schemas {
            types.push(parse_named(name, schema));
        }
    }
    Ir {
        types,
        operations: parse_operations(spec),
        webhook_events: parse_webhook_events(spec),
    }
}

fn parse_webhook_events(spec: &Value) -> Vec<WebhookVariant> {
    let Some(schemas) = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut variants: Vec<WebhookVariant> = Vec::new();
    for (name, schema) in schemas {
        if !(name.starts_with("Webhook") && name.ends_with("Payload")) {
            continue;
        }
        let wire_type = schema
            .pointer("/properties/type/const")
            .and_then(Value::as_str)
            .or_else(|| {
                schema
                    .pointer("/properties/type/enum/0")
                    .and_then(Value::as_str)
            });
        let Some(wire_type) = wire_type else {
            continue;
        };
        let middle = name
            .strip_prefix("Webhook")
            .unwrap()
            .strip_suffix("Payload")
            .unwrap();
        variants.push(WebhookVariant {
            wire_type: wire_type.to_string(),
            variant: to_pascal(middle),
            payload: to_pascal(name),
        });
    }
    variants.sort_by(|a, b| a.wire_type.cmp(&b.wire_type));
    variants
}

fn parse_operations(spec: &Value) -> Vec<Operation> {
    let mut items: Vec<(Operation, Vec<String>)> = Vec::new();
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return Vec::new();
    };
    const METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];
    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for m in METHODS {
            let Some(op) = item.get(m).and_then(Value::as_object) else {
                continue;
            };
            let tag_raw = op
                .get("tags")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("default");
            let op_id = op.get("operationId").and_then(Value::as_str).unwrap_or("");
            let segments: Vec<String> = op_id
                .split([':', '.'])
                .filter(|s| !s.is_empty())
                .map(to_snake)
                .collect();
            let method_name = segments.last().cloned().unwrap_or_else(|| to_snake(op_id));
            let (mut path_params, mut query_params) = (Vec::new(), Vec::new());
            if let Some(params) = op.get("parameters").and_then(Value::as_array) {
                for p in params {
                    let loc = p.get("in").and_then(Value::as_str).unwrap_or("");
                    let wire = p
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let required = p.get("required").and_then(Value::as_bool).unwrap_or(false);
                    let ty = p.get("schema").map(parse_type).unwrap_or(RustType::String);
                    let param = Param {
                        name: to_snake(&wire),
                        wire,
                        ty,
                        required,
                    };
                    match loc {
                        "path" => path_params.push(param),
                        "query" => query_params.push(param),
                        _ => {}
                    }
                }
            }
            path_params.sort_by(|a, b| a.name.cmp(&b.name));
            query_params.sort_by(|a, b| a.name.cmp(&b.name));
            let request_body = op
                .get("requestBody")
                .and_then(|b| b.pointer("/content/application~1json/schema"))
                .map(parse_type);
            let response = op
                .get("responses")
                .and_then(Value::as_object)
                .and_then(|r| {
                    let mut keys: Vec<&String> = r.keys().filter(|k| k.starts_with('2')).collect();
                    keys.sort();
                    keys.first().and_then(|k| {
                        r[*k]
                            .pointer("/content/application~1json/schema")
                            .map(parse_type)
                    })
                })
                .unwrap_or(RustType::Value);
            items.push((
                Operation {
                    tag: to_pascal(tag_raw),
                    accessor: to_snake(tag_raw),
                    method_name,
                    http_method: m.to_uppercase(),
                    path: path.clone(),
                    path_params,
                    query_params,
                    request_body,
                    response,
                    doc: op
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                segments,
            ));
        }
    }
    // Deterministic processing order before disambiguation (tag, full operationId).
    items.sort_by(|a, b| (a.0.tag.as_str(), a.1.join("_")).cmp(&(b.0.tag.as_str(), b.1.join("_"))));
    disambiguate_method_names(&mut items);
    let mut ops: Vec<Operation> = items.into_iter().map(|(op, _)| op).collect();
    ops.sort_by(|a, b| {
        (a.tag.as_str(), a.method_name.as_str()).cmp(&(b.tag.as_str(), b.method_name.as_str()))
    });
    ops
}

/// Resolve duplicate `method_name`s within a single tag deterministically.
///
/// The default method name is the last `:`/`.` segment of the operationId (e.g.
/// `customer_portal:benefit-grants:list` -> `list`). When several operations on
/// one tag collapse to the same name, the colliding ones are widened by dropping
/// any leading segment equal to the tag accessor and joining the rest (e.g.
/// `benefit_grants_list`). A numeric suffix is appended only if a collision still
/// remains, keeping every name stable and unique.
fn disambiguate_method_names(items: &mut [(Operation, Vec<String>)]) {
    use std::collections::HashMap;
    let mut by_tag: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, (op, _)) in items.iter().enumerate() {
        by_tag.entry(op.tag.clone()).or_default().push(i);
    }
    let mut tags: Vec<&String> = by_tag.keys().collect();
    tags.sort();
    for tag in tags {
        let group = &by_tag[tag];
        let mut counts: HashMap<String, usize> = HashMap::new();
        for &i in group {
            *counts.entry(items[i].0.method_name.clone()).or_insert(0) += 1;
        }
        for &i in group {
            if counts[&items[i].0.method_name] > 1 {
                let accessor = items[i].0.accessor.clone();
                let segs = &items[i].1;
                let widened: Vec<&String> = match segs.split_first() {
                    Some((head, rest)) if *head == accessor && !rest.is_empty() => {
                        rest.iter().collect()
                    }
                    _ => segs.iter().collect(),
                };
                if !widened.is_empty() {
                    items[i].0.method_name = widened
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join("_");
                }
            }
        }
        // Final safety net: append a deterministic counter to any residual dups.
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut ordered = group.clone();
        ordered.sort_by(|&a, &b| items[a].1.join("_").cmp(&items[b].1.join("_")));
        for i in ordered {
            let n = seen.entry(items[i].0.method_name.clone()).or_insert(0);
            if *n > 0 {
                items[i].0.method_name = format!("{}_{}", items[i].0.method_name, n);
            }
            *n += 1;
        }
    }
}

fn merge_all_of(schema: &Value) -> Option<Value> {
    let members = schema.get("allOf")?.as_array()?;
    let mut props = serde_json::Map::new();
    let mut required: Vec<Value> = Vec::new();
    // Each member is assumed to be an inline object schema; refs are unresolved at this stage.
    for m in members {
        if let Some(ref_val) = m.get("$ref").and_then(Value::as_str) {
            debug_assert!(
                false,
                "allOf $ref member not yet supported (schema member: {ref_val}); \
                 inline-only merge would silently drop fields"
            );
        }
        if let Some(p) = m.get("properties").and_then(Value::as_object) {
            for (k, v) in p {
                props.insert(k.clone(), v.clone());
            }
        }
        if let Some(r) = m.get("required").and_then(Value::as_array) {
            required.extend(r.iter().cloned());
        }
    }
    Some(serde_json::json!({ "type": "object", "properties": props, "required": required }))
}

fn parse_named(name: &str, schema: &Value) -> TypeDef {
    let doc = schema
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(merged) = merge_all_of(schema) {
        let kind = parse_struct(&merged);
        return TypeDef {
            name: to_pascal(name),
            doc,
            kind,
        };
    }
    let kind = if let Some(disc) = schema.get("discriminator") {
        let tag = disc
            .get("propertyName")
            .and_then(Value::as_str)
            .unwrap_or("type")
            .to_string();
        let mut variants = Vec::new();
        if let Some(map) = disc.get("mapping").and_then(Value::as_object) {
            for (wire, reference) in map {
                let ty_name = reference
                    .as_str()
                    .unwrap_or("")
                    .rsplit('/')
                    .next()
                    .unwrap_or("");
                variants.push(UnionVariant {
                    name: to_pascal(wire),
                    rename: Some(wire.clone()),
                    ty: to_pascal(ty_name),
                });
            }
        }
        variants.sort_by(|a, b| a.name.cmp(&b.name));
        TypeKind::TaggedUnion { tag, variants }
    } else if let Some(values) = schema
        .get("enum")
        .and_then(Value::as_array)
        .filter(|vs| vs.iter().any(Value::is_string))
    {
        let variants = values
            .iter()
            .filter_map(Value::as_str)
            .map(|v| {
                // Leading '-' denotes a descending sort key; disambiguate from the
                // ascending variant (`-created_at` vs `created_at`) with a `Desc` prefix.
                let name = match v.strip_prefix('-') {
                    Some(rest) => format!("Desc{}", to_pascal(rest)),
                    None => to_pascal(v),
                };
                EnumVariant {
                    name,
                    rename: Some(v.to_string()),
                }
            })
            .collect();
        TypeKind::StringEnum(variants)
    } else if schema.get("type").and_then(Value::as_str) == Some("object")
        || schema.get("properties").is_some()
    {
        parse_struct(schema)
    } else {
        TypeKind::Alias(parse_type(schema))
    };
    TypeDef {
        name: to_pascal(name),
        doc,
        kind,
    }
}

fn parse_struct(schema: &Value) -> TypeKind {
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut fields = Vec::new();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (pname, pschema) in props {
            let mut ty = parse_type(pschema);
            let already_opt = matches!(ty, RustType::Option(_));
            if !required.contains(&pname.as_str()) && !already_opt {
                ty = RustType::Option(Box::new(ty));
            }
            let snake = to_snake(pname);
            let rename = if &snake != pname {
                Some(pname.clone())
            } else {
                None
            };
            fields.push(Field {
                name: snake,
                rename,
                ty,
                doc: pschema
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    TypeKind::Struct(fields)
}

fn parse_type(schema: &Value) -> RustType {
    // type array with "null"
    if let Some(arr) = schema.get("type").and_then(Value::as_array) {
        let non_null: Vec<&str> = arr
            .iter()
            .filter_map(Value::as_str)
            .filter(|t| *t != "null")
            .collect();
        let has_null = arr.iter().any(|t| t.as_str() == Some("null"));
        if let Some(single) = non_null.first().filter(|_| non_null.len() == 1) {
            let mut inner = schema.clone();
            inner["type"] = Value::String((*single).to_string());
            let parsed = parse_type(&inner);
            return if has_null {
                RustType::Option(Box::new(parsed))
            } else {
                parsed
            };
        }
    }
    // anyOf with a null member
    if let Some(arr) = schema.get("anyOf").and_then(Value::as_array) {
        let has_null = arr
            .iter()
            .any(|m| m.get("type").and_then(Value::as_str) == Some("null"));
        let rest: Vec<&Value> = arr
            .iter()
            .filter(|m| m.get("type").and_then(Value::as_str) != Some("null"))
            .collect();
        if rest.len() == 1 {
            let parsed = parse_type(rest[0]);
            return if has_null {
                RustType::Option(Box::new(parsed))
            } else {
                parsed
            };
        }
        // multi-member anyOf without a clean single type: fall back to Value.
        return RustType::Value;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        return RustType::Named(to_pascal(name));
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => {
            if schema.get("format").and_then(Value::as_str) == Some("date-time") {
                RustType::Timestamp
            } else {
                RustType::String
            }
        }
        Some("boolean") => RustType::Bool,
        Some("integer") => RustType::I64,
        Some("number") => RustType::F64,
        Some("array") => {
            let items = schema
                .get("items")
                .map(parse_type)
                .unwrap_or(RustType::Value);
            RustType::Vec(Box::new(items))
        }
        Some("object") => {
            match schema.get("additionalProperties") {
                Some(Value::Object(ap)) => {
                    if ap.contains_key("anyOf") {
                        RustType::Metadata
                    } else {
                        RustType::Map(Box::new(parse_type(&Value::Object(ap.clone()))))
                    }
                }
                Some(Value::Bool(true)) => RustType::Metadata,
                // Inline object with explicit properties: promote to a struct
                // in a later plan; for now treat as opaque JSON.
                _ => RustType::Value,
            }
        }
        _ => RustType::Value,
    }
}

pub fn to_pascal(s: &str) -> String {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_uppercase() && !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            // Map any separator (`-`, space, `.`, `[`, etc.) to a single `_`.
            out.push('_');
        }
    }
    let trimmed = out.trim_end_matches('_');
    let trimmed = trimmed.trim_start_matches('_');
    out = trimmed.to_string();
    if out.is_empty() || out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}
