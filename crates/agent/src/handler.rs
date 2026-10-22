use crate::{artifacts::Artifacts, s3::SignedPost};
use buildplane_shared::build_request::BuildRequest;
use buildplane_shared::errors::TimeoutError;
use buildplane_shared::errors::{BpError, BpResult};
use std::{path::PathBuf, time::Duration};
use tokio::time::timeout;

#[derive(Clone)]
pub struct Handler {
    build_timeout: Duration,
    source_dir: PathBuf,
    build_cache_dir: PathBuf,
}

impl Handler {
    pub fn new(build_timeout: Duration, source_dir: PathBuf, build_cache_dir: PathBuf) -> Self {
        Self {
            build_timeout,
            source_dir,
            build_cache_dir,
        }
    }

    pub async fn handle(&self, request: BuildRequest) -> BpResult<()> {
        tracing::info!(
            git_host = %request.git_host,
            git_provider = %request.git_provider,
            repo_id = %request.repo_id,
            ref_name = %request.ref_name,
            ref_type = %request.ref_type,
            commit_hash = request.commit_hash.as_deref().unwrap_or("unknown"),
            "starting build"
        );
        let signed_post = SignedPost::from_build_request(&request.signed_post, self.build_timeout)?;
        let artifacts = Artifacts::new(
            signed_post,
            request.signed_get_url,
            self.source_dir.clone(),
            self.build_cache_dir.clone(),
        )?;
        timeout(self.build_timeout, async {
            let cwasm = artifacts.compile().await?;
            artifacts.publish(&cwasm).await
        })
        .await
        .unwrap_or(Err(BpError::Timeout(TimeoutError::BuildTimeout(
            self.build_timeout,
        ))))
    }
}
