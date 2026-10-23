use crate::git_ops::github::{github_auth::GitHubAuth, installation::GitHubInstallation};
use buildplane_errors::BpResult;
use buildplane_messages::sqs_message::GitSource;
use buildplane_time::deadline::Deadline;

pub enum GitRemote {
    GitHub(GitHubInstallation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFile {
    pub path: String,
    pub contents: Vec<u8>,
}

#[allow(async_fn_in_trait)]
pub trait GitHost<InstallationId, RepoId>: Send + Sync {
    fn provider(&self) -> &'static str;
    fn host(&self) -> &str;
    fn installation_id(&self) -> InstallationId;
    fn parse_repo_id(&self, repo_id: &str) -> BpResult<RepoId>;
    async fn repo_ids(&self, deadline: Deadline) -> BpResult<Vec<RepoId>>;
    async fn repo_default_branch(&self, repo_id: RepoId, deadline: Deadline) -> BpResult<String>;
    async fn ref_commit(
        &self,
        repo_id: RepoId,
        ref_name: &str,
        deadline: Deadline,
    ) -> BpResult<String>;
    async fn directory_files(
        &self,
        repo_id: RepoId,
        ref_name: &str,
        directory: &str,
        deadline: Deadline,
    ) -> BpResult<Vec<GitFile>>;
}

impl GitRemote {
    pub fn from_source(github_auth: &GitHubAuth, source: &GitSource) -> BpResult<Self> {
        match source {
            GitSource::GitHub {
                host,
                installation_id,
                ..
            } => Ok(GitRemote::GitHub(
                github_auth.installation(host, *installation_id)?,
            )),
        }
    }
}
