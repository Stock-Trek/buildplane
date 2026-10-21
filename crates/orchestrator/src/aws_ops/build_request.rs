use crate::{aws_ops::s3::PresignedPost, errors::error::RsOrchestratorResult};
use buildplane_shared::sqs_message::{GitSource, SqsRefType};
use std::collections::BTreeMap;

const COMMIT_HASH: &str = "COMMIT_HASH";
const FORCED: &str = "FORCED";
const GIT_HOST: &str = "GIT_HOST";
const GIT_PROVIDER: &str = "GIT_PROVIDER";
const REPO_ID: &str = "REPO_ID";
const REF_NAME: &str = "REF_NAME";
const REF_TYPE: &str = "REF_TYPE";
const SIGNED_GET_URL: &str = "SIGNED_GET_URL";
const SIGNED_POST_FIELDS: &str = "SIGNED_POST_FIELDS";
const SIGNED_POST_URL: &str = "SIGNED_POST_URL";

pub struct BuildRequest {
    pub source: GitSource,
    pub repo_id: String,
    pub ref_name: String,
    pub ref_type: SqsRefType,
    pub commit_hash: Option<String>,
    pub forced: bool,
    pub signed_get_url: String,
    pub signed_post: PresignedPost,
}

impl BuildRequest {
    /// The environment passed to the build agent for this invocation.
    ///
    /// Lambda does not support per-invocation environment variables, so the
    /// values are sent in the invocation payload instead.
    pub fn environment(&self) -> RsOrchestratorResult<BTreeMap<&'static str, String>> {
        let mut environment = BTreeMap::from([
            (GIT_HOST, self.source.host().to_string()),
            (GIT_PROVIDER, self.source.name().to_string()),
            (REPO_ID, self.repo_id.clone()),
            (REF_NAME, self.ref_name.clone()),
            (REF_TYPE, self.ref_type.to_string()),
            (SIGNED_GET_URL, self.signed_get_url.clone()),
            (SIGNED_POST_URL, self.signed_post.url.clone()),
            (SIGNED_POST_FIELDS, self.signed_post.fields_json()?),
        ]);
        if let Some(commit_hash) = self.commit_hash.as_deref() {
            environment.insert(COMMIT_HASH, commit_hash.to_string());
            environment.insert(FORCED, self.forced.to_string());
        }
        Ok(environment)
    }

    pub fn payload(&self) -> RsOrchestratorResult<Vec<u8>> {
        Ok(serde_json::to_vec(&self.environment()?)?)
    }
}
