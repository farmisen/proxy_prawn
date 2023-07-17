use crate::api_client::ApiClient;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::schemas::Model;

#[derive(Debug, Serialize, Deserialize)]
struct ModelsResponse {
    object: String,
    data: Vec<Model>,
}

pub async fn get_models(State(client): State<ApiClient>) -> Json<Vec<Model>> {
    let response = client.get::<ModelsResponse>("/models".into()).await;

    Json(response.unwrap().data)
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use super::*;
    use serde_json::json;
    use tokio_test::block_on;

    use crate::schemas::{Model, ModelPermission};

    fn mock_client(mock_server: &mockito::ServerGuard) -> ApiClient {
        ApiClient::new(
            format!("http://{}/v1", mock_server.host_with_port()),
            "test-api-key".into(),
        )
    }

    #[test]
    fn test_get_models() {
        let mut mock_server = mockito::Server::new();
        let state = State(mock_client(&mock_server));

        let response_data = vec![Model {
            id: "babbage".into(),
            object: "model".into(),
            created: 1649358449_i64,
            owned_by: "openai".into(),
            permission: vec![ModelPermission {
                id: "modelperm-49FUp5v084tBB49tC4z8LPH5".into(),
                object: "model_permission".into(),
                created: 1669085501,
                allow_create_engine: false,
                allow_sampling: true,
                allow_logprobs: true,
                allow_search_indices: false,
                allow_view: true,
                allow_fine_tuning: false,
                organization: "*".into(),
                group: None,
                is_blocking: false,
            }],
            root: "babbage".into(),
            parent: None,
        }];
        let mock_response = json!({
            "object": "list",
            "data": response_data.clone(),
        });

        let mock_response_body = serde_json::to_string(&mock_response).unwrap();

        let mock = mock_server
            .mock("GET", "/v1/models")
            .match_header("Authorization", "Bearer test-api-key")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response_body.clone())
            .create();

        let result = block_on(get_models(state));

        assert_eq!(result.deref(), &response_data);
        mock.assert();
    }
}
