use reqwest::{Client, Error};
use serde::de::DeserializeOwned;

// The ApiClient struct holds information necessary for making HTTP requests to a specified API.
#[derive(Clone, Debug)]
pub struct ApiClient {
    api_key: String,      // The API key for authenticating requests.
    api_base_url: String, // The base URL of the API.
    client: Client,       // The `reqwest` HTTP client for making the actual requests.
}

impl ApiClient {
    pub fn new(api_base_url: String, api_key: String) -> Self {
        Self {
            api_base_url,
            api_key,
            client: Client::new(),
        }
    }

    // The get function makes an HTTP GET request to the specified path.
    // It takes the path as a parameter and returns the json deserialized response.
    // The type parameter T specifies the expected return type of the deserialized response.
    pub async fn get<T: DeserializeOwned>(&self, path: String) -> Result<T, Error> {
        self.client
            .get(format!("{}{}", self.api_base_url, path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await?
            .json::<T>()
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::ApiClient;
    use tokio_test::block_on;

    #[test]
    fn test_get_set_correct_headers() {
        let mut mock_server = mockito::Server::new();
        let mock = mock_server
            .mock("GET", "/")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .create();

        let api_client = ApiClient::new(mock_server.url(), "test-api-key".into());
        let _ = block_on(api_client.get::<()>("/".into()));
        mock.assert();
    }

    #[test]
    fn test_get_returns_deserialized_json() {
        let mut mock_server = mockito::Server::new();
        let mock = mock_server
            .mock("GET", "/")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[\"foo\", \"bar\", \"baz\"]")
            .create();

        let api_client = ApiClient::new(mock_server.url(), "test-api-key".into());
        let result = block_on(api_client.get::<Vec<String>>("/".into()));
        mock.assert();
        assert_eq!(
            result.unwrap(),
            vec!["foo".to_string(), "bar".to_string(), "baz".to_string()]
        );
    }
}
