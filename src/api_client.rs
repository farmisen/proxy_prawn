use reqwest::{Client, Error};
use serde::de::DeserializeOwned;

#[derive(Clone, Debug)]
pub struct ApiClient {
    api_key: String,
    api_base_url: String,
    client: Client,
}

impl ApiClient {
    pub fn new(api_base_url: String, api_key: String) -> Self {
        Self {
            api_base_url,
            api_key,
            client: Client::new(),
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: String) -> Result<T, Error> {
        self.client
            .get(format!("{}{}", self.api_base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .unwrap()
            .json::<T>()
            .await
    }
}
