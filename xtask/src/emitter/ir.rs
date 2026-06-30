#[derive(Debug, Clone, PartialEq)]
pub struct Ir {
    pub types: Vec<TypeDef>,
    pub operations: Vec<Operation>,
    pub webhook_events: Vec<WebhookVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WebhookVariant {
    pub wire_type: String,
    pub variant: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub tag: String,
    pub accessor: String,
    pub method_name: String,
    pub http_method: String,
    pub path: String,
    pub path_params: Vec<Param>,
    pub query_params: Vec<Param>,
    pub request_body: Option<RustType>,
    pub response: RustType,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub wire: String,
    pub ty: RustType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    pub name: String,
    pub doc: Option<String>,
    pub kind: TypeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Struct(Vec<Field>),
    StringEnum(Vec<EnumVariant>),
    TaggedUnion {
        tag: String,
        variants: Vec<UnionVariant>,
    },
    Alias(RustType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub rename: Option<String>,
    pub ty: RustType,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub rename: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionVariant {
    pub name: String,
    pub rename: Option<String>,
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    String,
    Bool,
    I64,
    F64,
    Timestamp,
    Value,
    Metadata,
    Named(String),
    Option(Box<RustType>),
    Vec(Box<RustType>),
    Map(Box<RustType>),
    Box(Box<RustType>),
}
