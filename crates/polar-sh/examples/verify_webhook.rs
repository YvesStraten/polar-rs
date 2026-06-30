// Demonstrates verifying a Standard Webhooks signature and parsing the payload.
// In production, payload and headers come from your HTTP request handler.

use std::collections::HashMap;

fn main() {
    // Replace with the raw request body bytes.
    let payload = br#"{"type":"product.created","timestamp":"2024-01-01T00:00:00Z","data":{"id":"prod_1","name":"Widget","organization_id":"org_1","created_at":"2024-01-01T00:00:00Z","is_archived":false,"is_recurring":false,"attached_custom_fields":[],"benefits":[],"medias":[],"metadata":{},"prices":[],"visibility":"public"}}"#;

    let secret = std::env::var("POLAR_WEBHOOK_SECRET")
        .unwrap_or_else(|_| "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw".to_string());

    // These headers come from the incoming HTTP request.
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert(
        "webhook-id".to_string(),
        "msg_p5jXN8AQM9LWM0D4loKWxJek".to_string(),
    );
    headers.insert("webhook-timestamp".to_string(), "1614265330".to_string());
    headers.insert(
        "webhook-signature".to_string(),
        "v1,placeholder_signature".to_string(),
    );

    if let Err(e) = polar_sh::webhooks::verify(payload, &headers, &secret) {
        eprintln!("signature verification failed: {e}");
        return;
    }

    let event: polar_sh::WebhookPayload = match serde_json::from_slice(payload) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("failed to parse payload: {e}");
            return;
        }
    };

    match event {
        polar_sh::WebhookPayload::ProductCreated(p) => {
            println!("product.created: {} ({})", p.data.name, p.data.id);
        }
        polar_sh::WebhookPayload::OrderCreated(o) => {
            println!("order.created: {}", o.data.id);
        }
        other => {
            println!("unhandled event: {other:?}");
        }
    }
}
