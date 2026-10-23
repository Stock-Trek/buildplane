use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::BpResult;

pub struct GitHubAppConfig {
    pub app_id: u64,
    pub private_key: String,
}

impl GitHubAppConfig {
    pub fn from_env(config: &EnvVarConfig) -> BpResult<Self> {
        Ok(Self {
            app_id: config.github_app_id(),
            private_key: config.github_app_private_key(),
        })
    }
}
