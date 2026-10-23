use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::{BpError, BpResult};

pub struct GitHubAppConfig {
    pub app_id: u64,
    pub private_key: String,
}

impl GitHubAppConfig {
    pub fn from_env(config: &EnvVarConfig) -> BpResult<Self> {
        Ok(Self {
            app_id: config
                .github_app_id
                .ok_or_else(|| BpError::config_missing("GITHUB_APP_ID"))?,
            private_key: config
                .github_app_private_key
                .clone()
                .ok_or_else(|| BpError::config_missing("GITHUB_APP_PRIVATE_KEY"))?,
        })
    }
}
