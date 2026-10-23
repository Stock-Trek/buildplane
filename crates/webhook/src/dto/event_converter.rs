use crate::{
    dto::github::github_event_converter::GitHubEventConverter, event_response::EventResponse,
    infisical::secrets::Secrets,
};
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::BpResult;
use lambda_http::Request;

pub trait EventConverter: Send + Sync {
    fn new(secrets: &Secrets, config: &EnvVarConfig) -> BpResult<Self>
    where
        Self: Sized;

    fn event_response(&self, event: &Request) -> BpResult<EventResponse>;
}

pub struct EventConverters;

impl EventConverters {
    pub fn all(secrets: &Secrets, config: &EnvVarConfig) -> BpResult<Vec<Box<dyn EventConverter>>> {
        Ok(vec![Box::new(GitHubEventConverter::new(secrets, config)?)])
    }
}
