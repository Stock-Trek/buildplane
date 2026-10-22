//! The payload sent from the orchestrator to the build agent Lambda.
//!
//! The payload is the Lambda invocation event, so it contains everything that
//! changes per build. Process-wide settings remain in the agent's environment.

use crate::sqs_message::SqsRefType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRequest {
    pub git_host: String,
    pub git_provider: String,
    pub repo_id: String,
    pub ref_name: String,
    pub ref_type: SqsRefType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    pub signed_get_url: String,
    pub signed_post: BuildRequestSignedPost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRequestSignedPost {
    pub url: String,
    pub fields: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_artifact_size_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> BuildRequest {
        BuildRequest {
            git_host: "github.com".to_string(),
            git_provider: "github".to_string(),
            repo_id: "42".to_string(),
            ref_name: "main".to_string(),
            ref_type: SqsRefType::Branch,
            commit_hash: Some("0123456789abcdef".to_string()),
            signed_get_url: "https://example.com/source.zip".to_string(),
            signed_post: BuildRequestSignedPost {
                url: "https://example.com/upload".to_string(),
                fields: BTreeMap::from([
                    ("key".to_string(), "artifacts/${filename}".to_string()),
                    ("policy".to_string(), "policy".to_string()),
                ]),
                max_artifact_size_bytes: Some(1024),
            },
        }
    }

    #[test]
    fn round_trips_through_the_lambda_payload() {
        let request = request();
        let payload = serde_json::to_vec(&request).unwrap();

        assert_eq!(
            serde_json::from_slice::<BuildRequest>(&payload).unwrap(),
            request
        );
    }

    #[test]
    fn uses_plain_json_field_names_and_a_nested_signed_post() {
        let payload = serde_json::to_value(request()).unwrap();

        assert_eq!(payload["ref_type"], "branch");
        assert_eq!(payload["signed_get_url"], "https://example.com/source.zip");
        assert_eq!(payload["signed_post"]["url"], "https://example.com/upload");
        assert_eq!(
            payload["signed_post"]["fields"]["key"],
            "artifacts/${filename}"
        );
        assert_eq!(payload["signed_post"]["max_artifact_size_bytes"], 1024);
        assert!(payload.get("SIGNED_GET_URL").is_none());
    }

    #[test]
    fn optional_fields_may_be_absent() {
        let payload = json!({
            "git_host": "github.com",
            "git_provider": "github",
            "repo_id": "42",
            "ref_name": "main",
            "ref_type": "tag",
            "signed_get_url": "https://example.com/source.zip",
            "signed_post": {
                "url": "https://example.com/upload",
                "fields": { "key": "artifacts/${filename}" },
            },
        });
        let request: BuildRequest = serde_json::from_value(payload).unwrap();

        assert_eq!(request.commit_hash, None);
        assert_eq!(request.signed_post.max_artifact_size_bytes, None);
    }

    #[test]
    fn omits_absent_optional_fields_when_serialising() {
        let mut request = request();
        request.commit_hash = None;
        request.signed_post.max_artifact_size_bytes = None;
        let payload = serde_json::to_value(request).unwrap();

        assert!(payload.get("commit_hash").is_none());
        assert!(
            payload["signed_post"]
                .get("max_artifact_size_bytes")
                .is_none()
        );
    }
}
