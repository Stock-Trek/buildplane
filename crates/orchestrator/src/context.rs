use crate::aws_ops::aws::Aws;
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::BpResult;
use buildplane_github::github_auth::GitHubAuth;
use buildplane_sqs::sqs_dedup::SqsDedup;
use std::time::Duration;

pub struct RsOrchestratorContext {
    pub aws: Aws,
    pub github_auth: GitHubAuth,
    pub dedup: SqsDedup,
    pub sqs_process_concurrency: usize,
    pub deadline_buffer: Duration,
}

impl RsOrchestratorContext {
    pub async fn new(config: &EnvVarConfig) -> BpResult<Self> {
        let aws_timeouts = Aws::load_timeouts(config)?;
        let sdk_config = Aws::load_sdk_config(aws_timeouts).await;
        let aws = Aws::from_env(&sdk_config, aws_timeouts, config)?;
        let github_auth = GitHubAuth::from_env(config)?;
        let dedup = SqsDedup::from_env(config)?;
        let sqs_process_concurrency = config.sqs_process_concurrency().get();
        let deadline_buffer = config.deadline_buffer();
        Ok(Self {
            aws,
            github_auth,
            dedup,
            sqs_process_concurrency,
            deadline_buffer,
        })
    }
}
