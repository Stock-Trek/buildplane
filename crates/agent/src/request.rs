//! The build request sent by the orchestrator.
//!
//! The orchestrator serialises the request as a JSON object whose
//! keys are the uppercase names of the former ECS container overrides. The
//! agent receives that object as the payload of the Lambda invocation and
//! reads the per-build values (signed URLs, repository metadata, ...) from it,
//! rather than requiring them as process environment variables.

use crate::s3::SignedPost;
use buildplane_shared::errors::ConfigError;
use buildplane_shared::errors::{BpError, BpResult};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug, Clone, Deserialize)]
pub struct BuildRequest {
    #[serde(rename = "GIT_HOST", default)]
    pub git_host: String,
    #[serde(rename = "GIT_PROVIDER", default)]
    pub git_provider: String,
    #[serde(rename = "REPO_ID", default)]
    pub repo_id: String,
    #[serde(rename = "REF_NAME", default)]
    pub ref_name: String,
    #[serde(rename = "REF_TYPE", default)]
    pub ref_type: String,
    #[serde(rename = "COMMIT_HASH", default)]
    pub commit_hash: Option<String>,
    #[serde(rename = "SIGNED_GET_URL")]
    pub signed_get_url: String,
    #[serde(rename = "SIGNED_POST_URL")]
    pub signed_post_url: String,
    #[serde(rename = "SIGNED_POST_FIELDS")]
    pub signed_post_fields: String,
}

impl BuildRequest {
    pub fn signed_post(&self, timeout: Duration) -> BpResult<SignedPost> {
        SignedPost::new(
            self.signed_post_url.clone(),
            Self::parse_signed_post_fields(&self.signed_post_fields)?,
            timeout,
        )
    }

    fn parse_signed_post_fields(json: &str) -> BpResult<BTreeMap<String, String>> {
        let value: Value = serde_json::from_str(json).map_err(|source| {
            BpError::Config(ConfigError::Build(format!(
                "SIGNED_POST_FIELDS is not valid JSON: {source}"
            )))
        })?;
        let object = value.as_object().ok_or_else(|| {
            BpError::Config(ConfigError::Build(
                "SIGNED_POST_FIELDS must be a JSON object".to_string(),
            ))
        })?;

        let mut fields = BTreeMap::new();
        for (name, value) in object {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(boolean) => boolean.to_string(),
                _ => {
                    return Err(BpError::Config(ConfigError::Build(format!(
                        "SIGNED_POST_FIELDS.{name} must be a string"
                    ))));
                }
            };
            fields.insert(name.clone(), text);
        }
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> serde_json::Value {
        json!({
            "COMMIT_HASH": "0123456789abcdef",
            "GIT_HOST": "github.com",
            "GIT_PROVIDER": "github",
            "REPO_ID": "42",
            "REF_NAME": "main",
            "REF_TYPE": "branch",
            "SIGNED_GET_URL": "https://example.com/source.zip",
            "SIGNED_POST_FIELDS": "{\"key\":\"artifacts/${filename}\",\"policy\":\"policy\"}",
            "SIGNED_POST_URL": "https://example.com/upload",
        })
    }

    #[test]
    fn deserializes_orchestrator_payload() {
        let request: BuildRequest = serde_json::from_value(payload()).unwrap();

        assert_eq!(request.git_host, "github.com");
        assert_eq!(request.git_provider, "github");
        assert_eq!(request.repo_id, "42");
        assert_eq!(request.ref_name, "main");
        assert_eq!(request.ref_type, "branch");
        assert_eq!(request.commit_hash.as_deref(), Some("0123456789abcdef"));
        assert_eq!(request.signed_get_url, "https://example.com/source.zip");
        assert_eq!(request.signed_post_url, "https://example.com/upload");
    }

    #[test]
    fn parses_signed_post_fields() {
        let fields = BuildRequest::parse_signed_post_fields(
            "{\"key\":\"artifacts/${filename}\",\"policy\":\"policy\",\"x-amz-date\":123}",
        )
        .unwrap();

        assert_eq!(fields["key"], "artifacts/${filename}");
        assert_eq!(fields["policy"], "policy");
        assert_eq!(fields["x-amz-date"], "123");
    }

    #[test]
    fn rejects_non_object_signed_post_fields() {
        let error = BuildRequest::parse_signed_post_fields("[]").unwrap_err();

        assert!(error.to_string().contains("must be a JSON object"));
    }
}
