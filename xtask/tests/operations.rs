use xtask::emitter::{emit, ir::Ir, parse};

fn parse_ops(spec: serde_json::Value) -> Ir {
    parse::parse(&spec)
}

#[test]
fn emits_tag_struct_and_method() {
    let spec = serde_json::json!({
        "paths": { "/v1/products/{id}": { "get": {
            "tags": ["products"], "operationId": "products:get",
            "parameters": [ { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } } ],
            "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Product" } } } } }
        } } },
        "components": { "schemas": { "Product": { "type": "object", "properties": { "id": { "type": "string" } } } } }
    });
    let ir = parse::parse(&spec);
    let out = emit::emit_operations(&ir);
    assert!(out.contains("pub struct Products<'a>"), "got:\n{out}");
    assert!(out.contains("pub async fn get"), "got:\n{out}");
    assert!(out.contains("self.client.request"), "got:\n{out}");
    assert!(out.contains("trait Resources"), "got:\n{out}");
    assert!(
        out.contains("fn products(&self) -> Products"),
        "got:\n{out}"
    );
}

#[test]
fn parses_get_with_path_param_and_response() {
    let spec = serde_json::json!({
        "paths": {
            "/v1/products/{id}": {
                "get": {
                    "tags": ["products"],
                    "operationId": "products:get",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Product" } } } }
                    }
                }
            }
        },
        "components": { "schemas": { "Product": { "type": "object", "properties": { "id": { "type": "string" } } } } }
    });
    let ir = parse_ops(spec);
    let op = ir
        .operations
        .iter()
        .find(|o| o.method_name == "get")
        .expect("get op");
    assert_eq!(op.tag, "Products");
    assert_eq!(op.accessor, "products");
    assert_eq!(op.http_method, "GET");
    assert_eq!(op.path, "/v1/products/{id}");
    assert_eq!(op.path_params.len(), 1);
    assert_eq!(op.path_params[0].name, "id");
    assert!(matches!(op.response, xtask::emitter::ir::RustType::Named(ref n) if n == "Product"));
}

#[test]
fn emits_query_params_struct() {
    let spec = serde_json::json!({
        "paths": { "/v1/products": { "get": {
            "tags": ["products"], "operationId": "products:list",
            "parameters": [ { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer" } } ],
            "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Product" } } } } }
        } } },
        "components": { "schemas": { "Product": { "type": "object", "properties": { "id": { "type": "string" } } } } }
    });
    let ir = parse_ops(spec);
    let out = emit::emit_operations(&ir);
    assert!(
        out.contains("pub struct ProductsListParams"),
        "missing struct declaration; got:\n{out}"
    );
    assert!(
        out.contains("pub fn to_pairs(&self)"),
        "missing to_pairs method; got:\n{out}"
    );
    assert!(
        out.contains("params: &ProductsListParams"),
        "method signature missing Params ref; got:\n{out}"
    );
}

#[test]
fn emits_stream_for_page_based_list() {
    let spec = serde_json::json!({
        "paths": { "/v1/things": { "get": {
            "tags": ["things"], "operationId": "things:list",
            "parameters": [
                { "name": "page", "in": "query", "required": false, "schema": { "type": "integer" } }
            ],
            "responses": { "200": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ListResourceThing" } } } } }
        } } },
        "components": { "schemas": {
            "Thing": { "type": "object", "properties": { "id": { "type": "string" } } },
            "Pagination": { "type": "object", "required": ["max_page"], "properties": { "max_page": { "type": "integer" } } },
            "ListResourceThing": { "type": "object", "required": ["items", "pagination"], "properties": {
                "items": { "type": "array", "items": { "$ref": "#/components/schemas/Thing" } },
                "pagination": { "$ref": "#/components/schemas/Pagination" }
            } }
        } }
    });
    let ir = parse::parse(&spec);
    let out = emit::emit_operations(&ir);
    assert!(out.contains("pub fn list_stream"), "got:\n{out}");
    assert!(out.contains("polar_runtime::paginate"), "got:\n{out}");
    assert!(out.contains("types::Thing"), "got:\n{out}");
}
