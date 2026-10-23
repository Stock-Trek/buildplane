use buildplane_errors::BpResult;
use buildplane_github::github_auth::GitHubAuth;
use buildplane_github::installation::GitHubInstallation;
use buildplane_messages::sqs_message::GitSource;

pub enum GitRemote {
    GitHub(GitHubInstallation),
}

impl GitRemote {
    pub fn from_source(github_auth: &GitHubAuth, source: &GitSource) -> BpResult<Self> {
        match source {
            GitSource::GitHub {
                host,
                installation_id,
                ..
            } => Ok(Self::GitHub(
                github_auth.installation(host, *installation_id)?,
            )),
        }
    }
}
