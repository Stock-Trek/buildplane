use crate::{
    env_var_config::EnvVarConfig, error::BaResult, handler::Handler, request::BuildRequest,
    tracing_setup::TracingSetup,
};
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use serde_json::{Value, json};

mod artifacts;
mod env_var_config;
mod error;
mod handler;
mod request;
mod s3;
mod source;
mod tracing_setup;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config = EnvVarConfig::from_env()?;
    TracingSetup::setup(config.log_filter())?;
    let handler = Handler::new(
        config.build_timeout(),
        config.source_dir().to_path_buf(),
        config.build_cache_dir().to_path_buf(),
    );
    run(service_fn(move |event| {
        let handler = handler.clone();
        async move { BuildAgent::handle(handler, event).await }
    }))
    .await
}

struct BuildAgent;

impl BuildAgent {
    async fn handle(handler: Handler, event: LambdaEvent<BuildRequest>) -> BaResult<Value> {
        handler.handle(event.payload).await?;
        Ok(json!({ "status": "ok" }))
    }
}
