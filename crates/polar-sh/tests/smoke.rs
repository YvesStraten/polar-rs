use polar_sh::types::{WebhookEndpointCreate, WebhookEventType, WebhookFormat};
use polar_sh::Resources;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn product_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "Widget",
        "organization_id": "org_1",
        "created_at": "2024-01-01T00:00:00Z",
        "is_archived": false,
        "is_recurring": false,
        "attached_custom_fields": [],
        "benefits": [],
        "medias": [],
        "metadata": {},
        "prices": [],
        "visibility": "public"
    })
}

#[tokio::test]
async fn products_get_roundtrips() {
    let server = MockServer::start().await;

    // Minimal JSON that satisfies all required fields of types::Product.
    // Optional fields are omitted; empty vecs cover attached_custom_fields,
    // benefits, medias, and prices.
    let body = serde_json::json!({
        "id": "prod_1",
        "name": "Widget",
        "organization_id": "org_1",
        "created_at": "2024-01-01T00:00:00Z",
        "is_archived": false,
        "is_recurring": false,
        "attached_custom_fields": [],
        "benefits": [],
        "medias": [],
        "metadata": {},
        "prices": [],
        "visibility": "public"
    });

    Mock::given(method("GET"))
        .and(path("/v1/products/prod_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let polar = polar_sh::Polar::builder("tok")
        .base_url(server.uri())
        .build();

    let product = polar.products().get("prod_1").await.unwrap();

    assert_eq!(product.id, "prod_1");
    assert_eq!(product.name, "Widget");
    assert!(!product.is_archived);
}

#[tokio::test]
async fn products_list_stream_paginates() {
    use polar_sh::TryStreamExt;

    let server = MockServer::start().await;

    // Two pages, one Product each, max_page = 2. The stream must walk both.
    let page1 = serde_json::json!({
        "items": [product_json("prod_1")],
        "pagination": { "max_page": 2, "total_count": 2 }
    });
    let page2 = serde_json::json!({
        "items": [product_json("prod_2")],
        "pagination": { "max_page": 2, "total_count": 2 }
    });

    Mock::given(method("GET"))
        .and(path("/v1/products/"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page1))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/products/"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page2))
        .mount(&server)
        .await;

    let polar = polar_sh::Polar::builder("t").base_url(server.uri()).build();

    let items: Vec<_> = polar
        .products()
        .list_stream(&Default::default())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "prod_1");
    assert_eq!(items[1].id, "prod_2");
}

#[tokio::test]
async fn create_sends_body_and_decodes_response() {
    let server = MockServer::start().await;

    let response_body = serde_json::json!({
        "id": "whe_1",
        "url": "https://example.com/hook",
        "organization_id": "org_1",
        "secret": "whsec_abc",
        "enabled": true,
        "format": "raw",
        "events": ["checkout.created"],
        "created_at": "2024-01-01T00:00:00Z"
    });

    Mock::given(method("POST"))
        .and(path("/v1/webhooks/endpoints"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&server)
        .await;

    let polar = polar_sh::Polar::builder("tok")
        .base_url(server.uri())
        .build();

    let body = WebhookEndpointCreate {
        events: vec![WebhookEventType::CheckoutCreated],
        format: WebhookFormat::Raw,
        url: "https://example.com/hook".to_string(),
        name: None,
        organization_id: None,
    };

    let endpoint = polar
        .webhooks()
        .create_webhook_endpoint(&body)
        .await
        .unwrap();

    assert_eq!(endpoint.id, "whe_1");
    assert_eq!(endpoint.url, "https://example.com/hook");
    assert!(endpoint.enabled);
}

#[tokio::test]
async fn api_error_surfaces_through_generated_method() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/webhooks/endpoints"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"detail":"Not found"}"#))
        .mount(&server)
        .await;

    let polar = polar_sh::Polar::builder("tok")
        .base_url(server.uri())
        .build();

    let body = WebhookEndpointCreate {
        events: vec![WebhookEventType::CheckoutCreated],
        format: WebhookFormat::Raw,
        url: "https://example.com/hook".to_string(),
        name: None,
        organization_id: None,
    };

    let err = polar
        .webhooks()
        .create_webhook_endpoint(&body)
        .await
        .unwrap_err();

    assert!(
        matches!(err, polar_sh::Error::Api { status: 404, .. }),
        "expected Api 404, got {err:?}",
    );
}

#[cfg(feature = "webhooks")]
#[test]
fn webhook_verify_and_parse_roundtrips() {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::collections::HashMap;

    // A `product.created` event: the payload's `data` is a full Product, reusing
    // the minimal-but-valid Product JSON above.
    let payload = serde_json::json!({
        "type": "product.created",
        "timestamp": "2024-01-01T00:00:00Z",
        "data": product_json("prod_1"),
    })
    .to_string();

    let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
    let id = "msg_test";
    let timestamp = "1700000000";

    let key = base64::engine::general_purpose::STANDARD
        .decode(secret.strip_prefix("whsec_").unwrap())
        .unwrap();
    let signed = format!("{id}.{timestamp}.{payload}");
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
    mac.update(signed.as_bytes());
    let sig = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    let mut headers = HashMap::new();
    headers.insert("webhook-id".to_string(), id.to_string());
    headers.insert("webhook-timestamp".to_string(), timestamp.to_string());
    headers.insert("webhook-signature".to_string(), format!("v1,{sig}"));

    polar_sh::webhooks::verify(payload.as_bytes(), &headers, secret).unwrap();

    let event: polar_sh::WebhookPayload = serde_json::from_slice(payload.as_bytes()).unwrap();
    assert!(matches!(event, polar_sh::WebhookPayload::ProductCreated(_)));
}
