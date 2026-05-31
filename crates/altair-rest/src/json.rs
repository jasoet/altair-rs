//! JSON request/response helpers on [`Client`].

use crate::client::Client;
use crate::error::{Error, Result};
use reqwest::Response;
use reqwest_middleware::RequestBuilder;
use serde::Serialize;
use serde::de::DeserializeOwned;

impl Client {
    /// GET the URL and decode the response body as JSON.
    ///
    /// Calls `response.error_for_status()` before decoding — 4xx/5xx
    /// responses surface as [`Error::Http`], not as a misleading
    /// [`Error::Decode`] on an HTML error page. Bodies exceeding the
    /// configured [`crate::ClientBuilder::response_body_limit`] are
    /// rejected with [`Error::ResponseTooLarge`].
    pub async fn get_json<T>(&self, url: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self.get(url).send().await.map_err(Error::from)?;
        decode_json(response, self.response_body_limit).await
    }

    /// POST a JSON body and decode the response body as JSON. Sets
    /// `content-type: application/json` automatically.
    pub async fn post_json<T, R>(&self, url: &str, body: &R) -> Result<T>
    where
        T: DeserializeOwned,
        R: Serialize + ?Sized,
    {
        send_json(self.post(url), body, self.response_body_limit).await
    }

    /// PUT a JSON body and decode the response body as JSON. Sets
    /// `content-type: application/json` automatically.
    pub async fn put_json<T, R>(&self, url: &str, body: &R) -> Result<T>
    where
        T: DeserializeOwned,
        R: Serialize + ?Sized,
    {
        send_json(self.put(url), body, self.response_body_limit).await
    }

    /// PATCH a JSON body and decode the response body as JSON. Sets
    /// `content-type: application/json` automatically.
    pub async fn patch_json<T, R>(&self, url: &str, body: &R) -> Result<T>
    where
        T: DeserializeOwned,
        R: Serialize + ?Sized,
    {
        send_json(self.patch(url), body, self.response_body_limit).await
    }
}

async fn send_json<T, R>(builder: RequestBuilder, body: &R, body_limit: u64) -> Result<T>
where
    T: DeserializeOwned,
    R: Serialize + ?Sized,
{
    let serialized = serde_json::to_vec(body)?;
    let response = builder
        .header("content-type", "application/json")
        .body(serialized)
        .send()
        .await
        .map_err(Error::from)?;
    decode_json(response, body_limit).await
}

async fn decode_json<T>(response: Response, body_limit: u64) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = response.error_for_status().map_err(Error::from)?;

    // Fast-path: if Content-Length is present and over the cap, reject
    // without buffering at all.
    if let Some(len) = response.content_length()
        && len > body_limit
    {
        return Err(Error::ResponseTooLarge {
            received: len,
            limit: body_limit,
        });
    }

    let bytes = response.bytes().await.map_err(Error::from)?;
    let received = bytes.len() as u64;
    if received > body_limit {
        return Err(Error::ResponseTooLarge {
            received,
            limit: body_limit,
        });
    }
    let value = serde_json::from_slice(&bytes)?;
    Ok(value)
}
