use xtask::emitter::{emit, ir::Ir, parse, parse::to_pascal};

/// Build a one-component spec and return the formatted emitted Rust.
fn emit_one(name: &str, schema: serde_json::Value) -> String {
    let spec = serde_json::json!({
        "components": { "schemas": { name: schema } }
    });
    let ir: Ir = parse::parse(&spec);
    emit::emit(&ir)
}

#[test]
fn basic_struct() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object",
            "required": ["id", "name"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" }
            }
        }),
    );
    let expected = "\
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
}
";
    assert!(out.contains(expected), "got:\n{out}");
}

#[test]
fn field_order_is_deterministic() {
    // Properties written in reverse-alphabetical order in the JSON literal.
    // The emitted struct must list them alphabetically regardless of map backend.
    let out = emit_one(
        "Thing",
        serde_json::json!({
            "type": "object",
            "required": ["zebra", "alpha"],
            "properties": {
                "zebra": { "type": "string" },
                "alpha": { "type": "string" }
            }
        }),
    );
    let alpha_idx = out
        .find("pub alpha: String")
        .expect("alpha field not found");
    let zebra_idx = out
        .find("pub zebra: String")
        .expect("zebra field not found");
    assert!(
        alpha_idx < zebra_idx,
        "alpha must appear before zebra; got:\n{out}"
    );
}

#[test]
fn optional_field() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" }
            }
        }),
    );
    assert!(
        out.contains("#[serde(default, skip_serializing_if = \"Option::is_none\")]"),
        "got:\n{out}"
    );
    assert!(out.contains("pub name: Option<String>"), "got:\n{out}");
}

#[test]
fn ref_field() {
    let out = emit_one(
        "Order",
        serde_json::json!({
            "type": "object",
            "required": ["customer"],
            "properties": {
                "customer": { "$ref": "#/components/schemas/customer" }
            }
        }),
    );
    assert!(out.contains("pub customer: Customer"), "got:\n{out}");
}

#[test]
fn array_field() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object", "required": ["tags"],
            "properties": { "tags": { "type": "array", "items": { "type": "string" } } }
        }),
    );
    assert!(out.contains("pub tags: Vec<String>"), "got:\n{out}");
}

#[test]
fn typed_map_field() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object", "required": ["labels"],
            "properties": { "labels": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            } }
        }),
    );
    assert!(
        out.contains("std::collections::HashMap<String, String>"),
        "got:\n{out}"
    );
}

#[test]
fn metadata_field() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object", "required": ["metadata"],
            "properties": { "metadata": {
                "type": "object",
                "additionalProperties": { "anyOf": [ { "type": "string" }, { "type": "integer" } ] }
            } }
        }),
    );
    assert!(
        out.contains("pub metadata: polar_runtime::Metadata"),
        "got:\n{out}"
    );
}

#[test]
fn nullable_anyof() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "type": "object", "required": ["price"],
            "properties": {
                "price": { "anyOf": [ { "type": "integer" }, { "type": "null" } ] }
            }
        }),
    );
    assert!(out.contains("pub price: Option<i64>"), "got:\n{out}");
}

#[test]
fn nullable_ref() {
    let out = emit_one(
        "Subscription",
        serde_json::json!({
            "type": "object",
            "required": ["customer"],
            "properties": {
                "customer": {
                    "anyOf": [
                        { "$ref": "#/components/schemas/customer" },
                        { "type": "null" }
                    ]
                }
            }
        }),
    );
    assert!(
        out.contains("pub customer: Option<Customer>"),
        "got:\n{out}"
    );
}

#[test]
fn string_enum() {
    let out = emit_one(
        "OrderStatus",
        serde_json::json!({
            "type": "string", "enum": ["paid", "refunded"]
        }),
    );
    assert!(out.contains("pub enum OrderStatus"), "got:\n{out}");
    assert!(out.contains("#[serde(rename = \"paid\")]"), "got:\n{out}");
    assert!(out.contains("Paid,"), "got:\n{out}");
}

#[test]
fn variant_order_is_deterministic() {
    // Mapping written in reverse-alphabetical order; emitted enum must list variants alphabetically.
    let spec = serde_json::json!({
        "components": {
            "schemas": {
                "Shape": {
                    "oneOf": [
                        { "$ref": "#/components/schemas/zeta_shape" },
                        { "$ref": "#/components/schemas/alpha_shape" }
                    ],
                    "discriminator": {
                        "propertyName": "type",
                        "mapping": {
                            "zeta":  "#/components/schemas/zeta_shape",
                            "alpha": "#/components/schemas/alpha_shape"
                        }
                    }
                },
                "zeta_shape":  { "type": "object", "properties": {} },
                "alpha_shape": { "type": "object", "properties": {} }
            }
        }
    });
    let ir: xtask::emitter::ir::Ir = xtask::emitter::parse::parse(&spec);
    let out = xtask::emitter::emit::emit(&ir);
    let alpha_idx = out.find("Alpha(").expect("Alpha variant not found");
    let zeta_idx = out.find("Zeta(").expect("Zeta variant not found");
    assert!(
        alpha_idx < zeta_idx,
        "Alpha must appear before Zeta; got:\n{out}"
    );
}

#[test]
fn tagged_union() {
    let out = emit_one(
        "Benefit",
        serde_json::json!({
            "oneOf": [
                { "$ref": "#/components/schemas/benefit_custom" },
                { "$ref": "#/components/schemas/benefit_license_keys" }
            ],
            "discriminator": {
                "propertyName": "type",
                "mapping": {
                    "custom": "#/components/schemas/benefit_custom",
                    "license_keys": "#/components/schemas/benefit_license_keys"
                }
            }
        }),
    );
    assert!(out.contains("#[serde(tag = \"type\")]"), "got:\n{out}");
    assert!(out.contains("pub enum Benefit"), "got:\n{out}");
    assert!(out.contains("#[serde(rename = \"custom\")]"), "got:\n{out}");
    assert!(out.contains("Custom(BenefitCustom)"), "got:\n{out}");
    assert!(
        out.contains("LicenseKeys(BenefitLicenseKeys)"),
        "got:\n{out}"
    );
}

#[test]
fn self_reference_is_boxed() {
    let out = emit_one(
        "Category",
        serde_json::json!({
            "type": "object", "required": ["id"],
            "properties": {
                "id": { "type": "string" },
                "parent": { "$ref": "#/components/schemas/category" }
            }
        }),
    );
    // `parent` is optional (not required) and self-referential -> Option<Box<Category>>
    assert!(
        out.contains("pub parent: Option<Box<Category>>"),
        "got:\n{out}"
    );
}

#[test]
fn timestamp_and_docs() {
    let out = emit_one(
        "Order",
        serde_json::json!({
            "type": "object", "description": "An order.", "required": ["created_at"],
            "properties": {
                "created_at": { "type": "string", "format": "date-time" }
            }
        }),
    );
    assert!(
        out.contains("pub created_at: polar_runtime::Timestamp"),
        "got:\n{out}"
    );
    assert!(out.contains("/// An order."), "got:\n{out}");
}

#[test]
fn allof_merge() {
    let out = emit_one(
        "Product",
        serde_json::json!({
            "allOf": [
                { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } },
                { "type": "object", "properties": { "name": { "type": "string" } } }
            ]
        }),
    );
    assert!(out.contains("pub id: String"), "got:\n{out}");
    assert!(out.contains("pub name: Option<String>"), "got:\n{out}");
}

#[test]
fn pascal_non_alphanumeric() {
    assert_eq!(to_pascal("user:read"), "UserRead");
    assert_eq!(to_pascal("Africa/Abidjan"), "AfricaAbidjan");
    assert_eq!(to_pascal("foo_bar"), "FooBar");
    assert_eq!(to_pascal("hello-world"), "HelloWorld");
}

#[test]
fn emits_webhook_payload_enum() {
    let spec = serde_json::json!({
        "components": { "schemas": {
            "WebhookBenefitCreatedPayload": { "type": "object", "properties": {
                "type": { "type": "string", "const": "benefit.created" },
                "data": { "type": "object" }
            } },
            "WebhookOrderCreatedPayload": { "type": "object", "properties": {
                "type": { "type": "string", "const": "order.created" },
                "data": { "type": "object" }
            } }
        } }
    });
    let ir = parse::parse(&spec);
    let out = emit::emit(&ir);
    assert!(out.contains("#[serde(tag = \"type\")]"), "got:\n{out}");
    // The synthesized union is `WebhookPayload`; `WebhookEvent` is a real schema.
    assert!(out.contains("pub enum WebhookPayload"), "got:\n{out}");
    assert!(
        out.contains("#[serde(rename = \"benefit.created\")]"),
        "got:\n{out}"
    );
    assert!(
        out.contains("BenefitCreated(WebhookBenefitCreatedPayload)"),
        "got:\n{out}"
    );
    assert!(
        out.contains("OrderCreated(WebhookOrderCreatedPayload)"),
        "got:\n{out}"
    );
    // Each payload's own `type` discriminator defaults on read and is skipped on
    // write (the enum tag re-emits it), so internal tagging round-trips.
    assert!(
        out.contains("#[serde(default, skip_serializing)]"),
        "got:\n{out}"
    );
}

#[test]
fn descending_sort_enum_variant() {
    let out = emit_one(
        "SortOrder",
        serde_json::json!({
            "type": "string",
            "enum": ["created_at", "-created_at"]
        }),
    );
    assert!(out.contains("pub enum SortOrder"), "got:\n{out}");
    assert!(
        out.contains("#[serde(rename = \"created_at\")]"),
        "got:\n{out}"
    );
    assert!(
        out.contains("#[serde(rename = \"-created_at\")]"),
        "got:\n{out}"
    );
    // The two Rust idents must be distinct.
    let asc_pos = out.find("CreatedAt").expect("CreatedAt not found");
    let desc_pos = out.find("DescCreatedAt").expect("DescCreatedAt not found");
    assert_ne!(asc_pos, desc_pos, "idents must differ; got:\n{out}");
}
