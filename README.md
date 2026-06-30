# Polar

[![ci](https://github.com/franzos/polar-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/franzos/polar-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/polar-sh.svg)](https://crates.io/crates/polar-sh)
[![Documentation](https://docs.rs/polar-sh/badge.svg)](https://docs.rs/polar-sh)

Rust SDK for the [Polar](https://polar.sh) API. The typed models and the full endpoint surface (185 operations across 27 resources) are generated from Polar's published OpenAPI document by an in-repo emitter; a small hand-written runtime provides the client, pagination, and webhook verification. Async, built on `reqwest` and `serde`.

## Usage

```toml
[dependencies]
polar-sh = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### Create a client

Authenticate with a Personal or Organization Access Token. Production by default, with a sandbox constructor and a builder for a custom base URL or your own `reqwest::Client`.

```rust
use polar_sh::Polar;

let polar = Polar::new("polar_oat_...");           // production
let polar = Polar::new_sandbox("polar_oat_...");   // sandbox

let polar = Polar::builder("polar_oat_...")
    .base_url("https://api.polar.sh")
    .http_client(reqwest::Client::new())           // bring your own (middleware, retries, ...)
    .build();
```

### Make a request

Resource accessors live on the `Resources` trait; bring it into scope and call typed methods that return generated model types.

```rust
use polar_sh::{Polar, Resources};
use polar_sh::operations::ProductsListParams;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let polar = Polar::new(std::env::var("POLAR_ACCESS_TOKEN")?);

    let params = ProductsListParams { limit: Some(10), ..Default::default() };
    let page = polar.products().list(&params).await?;

    println!("{} of {} product(s):", page.items.len(), page.pagination.total_count);
    for product in &page.items {
        println!("  {} — {}", product.id, product.name);
    }

    // Fetch one by id.
    let product = polar.products().get(&page.items[0].id).await?;
    println!("{} (org {})", product.name, product.organization_id);

    Ok(())
}
```

Every resource follows the same pattern: import `polar_sh::Resources`, then call `polar.<resource>().<method>(...)`.

### Pagination

Every page-based list endpoint gets a `<method>_stream` companion that walks all pages and yields each item as a `Stream`.

```rust
use polar_sh::{Polar, Resources, TryStreamExt};
use polar_sh::operations::OrdersListParams;

async fn all_orders(polar: &Polar) -> Result<(), polar_sh::Error> {
    let mut orders = polar.orders().list_stream(&OrdersListParams::default());
    while let Some(order) = orders.try_next().await? {
        println!("{}", order.id);
    }
    Ok(())
}
```

`TryStreamExt::try_collect` drives a stream to a `Vec` instead. Cursor-based endpoints (e.g. events) are not streamed; call them page by page.

### Creating resources

Request bodies are accepted as typed values. A few polymorphic bodies (like `ProductCreate`) are `serde_json::Value` and built with `serde_json::json!`, while the response stays fully typed.

```rust
use polar_sh::{Polar, Resources};

async fn create_product(polar: &Polar) -> Result<(), polar_sh::Error> {
    let body: polar_sh::types::ProductCreate = serde_json::json!({
        "name": "Pro plan",
        "recurring_interval": "month",
        "prices": [
            { "amount_type": "fixed", "price_amount": 2000, "price_currency": "usd" }
        ]
    });

    let product = polar.products().create(&body).await?;
    println!("created {} — {}", product.id, product.name);
    Ok(())
}
```

### Webhook verification

`webhooks::verify` implements the [Standard Webhooks](https://www.standardwebhooks.com) HMAC scheme Polar uses. Pass the raw request body, the request headers, and your endpoint secret; on success, deserialize the body into the typed `WebhookPayload` union.

```rust
use std::collections::HashMap;
use polar_sh::{webhooks, WebhookPayload};

fn handle(raw_body: &[u8], headers: HashMap<String, String>, secret: &str) {
    if let Err(e) = webhooks::verify(raw_body, &headers, secret) {
        eprintln!("rejected: {e}");
        return;
    }

    match serde_json::from_slice::<WebhookPayload>(raw_body) {
        Ok(WebhookPayload::OrderCreated(p)) => println!("order created: {:?}", p.data),
        Ok(WebhookPayload::CheckoutUpdated(p)) => println!("checkout updated: {:?}", p.data),
        Ok(other) => println!("other event: {other:?}"),
        Err(e) => eprintln!("decode failed: {e}"),
    }
}
```

`headers` keys are matched case-insensitively; the scheme reads `webhook-id`, `webhook-timestamp`, and `webhook-signature`. Verification is signature-only — timestamp/replay-window enforcement is left to the caller.

### Error handling

Every call returns `Result<T, polar_sh::Error>`. Non-success HTTP responses become `Error::Api` carrying the status and raw body.

```rust
use polar_sh::Error;

match polar.products().get("does-not-exist").await {
    Ok(product) => println!("{}", product.name),
    Err(Error::Api { status, body }) => eprintln!("API {status}: {body}"),
    Err(Error::Transport(e)) => eprintln!("transport: {e}"),
    Err(Error::Decode(e)) => eprintln!("decode: {e}"),
}
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `webhooks` | yes | Standard Webhooks verification and the `WebhookPayload` event union |

Disable default features to drop the webhook surface:

```toml
polar-sh = { version = "0.1", default-features = false }
```

## How the SDK is generated

The models and endpoints are not hand-written. An in-repo `xtask` emitter parses Polar's OpenAPI document into committed Rust under `crates/polar-api/` (`types.rs`, `operations.rs`). The vendored spec lives at `spec/openapi.json`, and `cargo build` only ever compiles the committed output — generation is an explicit step.

```sh
cargo run -p xtask -- update-spec   # fetch the latest spec into spec/openapi.json
cargo run -p xtask -- generate      # regenerate the committed Rust
```

A drift guard (`cargo test -p xtask --test drift`) asserts the committed code is byte-identical to a fresh generation, so the checked-in SDK can never silently diverge from the spec. The hand-written runtime (`polar-runtime`) and the `Resources` trait are the stable, semver-relevant surface.

## Limitations

- **Cursor pagination**: cursor-based endpoints (e.g. events) are returned as single pages; only page-based lists get a `_stream` companion.
- **Idempotency keys**: the Polar API has no idempotency-key mechanism, so none is exposed; inject a configured `reqwest::Client` if you need custom headers or retries.
- **Unresolved inline objects**: a number of deeply-nested or polymorphic request bodies (such as `ProductCreate`) are typed as `serde_json::Value` rather than fully modeled.
- **Per-resource crates**: all operations live in a single `polar-api` crate; splitting into per-resource crates is deferred.

## Testing

```sh
cargo test
```

The suite covers the emitter (one test per generation rule), the drift guard, the runtime (HTTP, pagination, and the canonical Standard Webhooks signature vector), and end-to-end round-trips against a mock HTTP server. No live API calls run in the test suite.

## License

Licensed under Apache 2.0.
