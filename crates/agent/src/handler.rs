use crate::{
    artifacts::Artifacts,
    error::{BaError, BaResult},
    request::BuildRequest,
};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Clone)]
pub struct Handler {
    build_timeout: Duration,
}

impl Handler {
    pub fn new(build_timeout: Duration) -> Self {
        Self { build_timeout }
    }

    pub async fn handle(&self, request: BuildRequest) -> BaResult<()> {
        tracing::info!(
            git_host = %request.git_host,
            git_provider = %request.git_provider,
            repo_id = %request.repo_id,
            ref_name = %request.ref_name,
            ref_type = %request.ref_type,
            commit_hash = request.commit_hash.as_deref().unwrap_or("unknown"),
            forced = request.forced.as_deref().unwrap_or("false"),
            "starting build"
        );
        let signed_post = request.signed_post(self.build_timeout)?;
        let artifacts = Artifacts::new(signed_post, request.signed_get_url)?;
        timeout(self.build_timeout, async {
            let cwasm = artifacts.compile().await?;
            artifacts.publish(&cwasm).await
        })
        .await
        .unwrap_or(Err(BaError::Timeout(self.build_timeout)))
    }
}
