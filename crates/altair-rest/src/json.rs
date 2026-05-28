//! JSON request/response helpers on [`Client`].

use crate::client::Client;
use crate::error::{Error, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

impl Client {
    /// GET the URL and decode the response body as JSON.
    ///
    /// Calls `response.error_for_status()` before decoding — 4xx/5xx responses
    /// surface as [`Error::Http`], not as a misleading [`Error::Decode`] on
    /// an HTML error page.
    pub async fn get_json<T>(&self, url: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self.get(url).send().await.map_err(Error::from)?;
        let response = response.error_for_status().map_err(Error::from)?;
        let bytes = response.bytes().await.map_err(Error::from)?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }

    /// POST a JSON body and decode the response body as JSON.
    pub async fn post_json<T, R>(&self, url: &str, body: &R) -> Result<T>
    where
        T: DeserializeOwned,
        R: Serialize + ?Sized,
    {
        let serialized = serde_json::to_vec(body)?;
        let response = self
            .post(url)
            .header("content-type", "application/json")
            .body(serialized)
            .send()
            .await
            .map_err(Error::from)?;
        let response = response.error_for_status().map_err(Error::from)?;
        let bytes = response.bytes().await.map_err(Error::from)?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }
}
