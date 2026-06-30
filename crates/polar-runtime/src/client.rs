use crate::error::Error;
use std::sync::Arc;

pub const PRODUCTION_URL: &str = "https://api.polar.sh";
pub const SANDBOX_URL: &str = "https://sandbox-api.polar.sh";

#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl Client {
    pub fn new(token: impl Into<String>) -> Client {
        Client::builder(token).build()
    }
    pub fn new_sandbox(token: impl Into<String>) -> Client {
        Client::builder(token).base_url(SANDBOX_URL).build()
    }
    pub fn builder(token: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            token: token.into(),
            base_url: PRODUCTION_URL.to_string(),
            http: None,
        }
    }

    pub async fn request<B, R>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(String, String)],
        body: Option<&B>,
    ) -> Result<R, Error>
    where
        B: serde::Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.inner.base_url, path);
        let mut req = self
            .inner
            .http
            .request(method, &url)
            .bearer_auth(&self.inner.token);
        if !query.is_empty() {
            req = req.query(query);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(Error::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        let text = if text.is_empty() {
            "null".to_string()
        } else {
            text
        };
        Ok(serde_json::from_str(&text)?)
    }
}

pub struct ClientBuilder {
    token: String,
    base_url: String,
    http: Option<reqwest::Client>,
}

impl ClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http = Some(client);
        self
    }
    pub fn build(self) -> Client {
        Client {
            inner: Arc::new(Inner {
                http: self.http.unwrap_or_default(),
                base_url: self.base_url,
                token: self.token,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_sends_bearer_and_decodes_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/ping"))
            .and(header("authorization", "Bearer tok_123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;
        let client = Client::builder("tok_123").base_url(server.uri()).build();
        let v: serde_json::Value = client
            .request(reqwest::Method::GET, "/v1/ping", &[], None::<&()>)
            .await
            .unwrap();
        assert_eq!(v, serde_json::json!({"ok": true}));
    }

    #[tokio::test]
    async fn non_2xx_returns_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/boom"))
            .respond_with(ResponseTemplate::new(404).set_body_string("nope"))
            .mount(&server)
            .await;
        let client = Client::builder("t").base_url(server.uri()).build();
        let err = client
            .request::<(), serde_json::Value>(reqwest::Method::GET, "/v1/boom", &[], None)
            .await
            .unwrap_err();
        match err {
            Error::Api { status, body } => {
                assert_eq!(status, 404);
                assert_eq!(body, "nope");
            }
            _ => panic!("wrong error: {err}"),
        }
    }
}
