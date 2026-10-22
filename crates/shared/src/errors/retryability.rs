use aws_credential_types::provider::error::CredentialsError;
use aws_sdk_lambda::{
    error::ProvideErrorMetadata, operation::invoke::InvokeError as SdkInvokeError,
};

const HTTP_FORBIDDEN: u16 = 403;
const HTTP_REQUEST_TIMEOUT: u16 = 408;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const HTTP_INTERNAL_SERVER_ERROR: u16 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retryability {
    Retryable,
    Permanent,
}

impl Retryability {
    pub fn sqs_batch_failure_retryability(code: &str) -> Retryability {
        if Self::is_permanent_sqs_error_code(code) {
            Retryability::Permanent
        } else {
            Retryability::Retryable
        }
    }

    pub fn from_http_status(status: u16) -> Retryability {
        match status {
            HTTP_REQUEST_TIMEOUT | HTTP_TOO_MANY_REQUESTS => Retryability::Retryable,
            status if status >= HTTP_INTERNAL_SERVER_ERROR => Retryability::Retryable,
            status if (400..500).contains(&status) => Retryability::Permanent,
            _ => Retryability::Retryable,
        }
    }

    pub fn from_binance_code(code: i64) -> Retryability {
        match code {
            -1001 | -1003 | -1006 | -1007 | -1015 | -1016 | -1021 => Retryability::Retryable,
            _ => Retryability::Permanent,
        }
    }

    pub fn from_binance_code_and_status(code: i64, status: u16) -> Retryability {
        match code {
            -1 => Self::from_http_status(status),
            code if (100..=599).contains(&code) => Self::from_http_status(code as u16),
            code => Self::from_binance_code(code),
        }
    }

    pub fn from_credentials(error: &CredentialsError) -> Retryability {
        match error {
            CredentialsError::ProviderTimedOut(_) | CredentialsError::ProviderError(_) => {
                Retryability::Retryable
            }
            _ => Retryability::Permanent,
        }
    }

    pub fn from_reqwest(error: &reqwest::Error) -> Retryability {
        match error.status().map(|status| status.as_u16()) {
            Some(status) => Self::from_http_status(status),
            None => Retryability::Retryable,
        }
    }

    pub fn from_octocrab(error: &octocrab::Error) -> Retryability {
        match error {
            octocrab::Error::GitHub { source, .. } => Self::from_github_response(
                source.status_code.as_u16(),
                &source.message,
                source.documentation_url.as_deref(),
            ),
            octocrab::Error::Installation { .. }
            | octocrab::Error::InstallationTokenInvalidAuth { .. }
            | octocrab::Error::InvalidHeaderValue { .. }
            | octocrab::Error::InvalidUtf8 { .. }
            | octocrab::Error::Json { .. }
            | octocrab::Error::JWT { .. }
            | octocrab::Error::Serde { .. }
            | octocrab::Error::SerdeUrlEncoded { .. }
            | octocrab::Error::Uri { .. }
            | octocrab::Error::UriParse { .. } => Retryability::Permanent,
            _ => Retryability::Retryable,
        }
    }

    pub fn from_lambda_invoke(error: &SdkInvokeError) -> Retryability {
        match error.code() {
            Some(
                "InvalidParameterValueException"
                | "InvalidRequestContentException"
                | "RecursiveInvocationException"
                | "ResourceConflictException"
                | "ResourceNotFoundException"
                | "UnsupportedMediaTypeException",
            ) => Retryability::Permanent,
            _ => Retryability::Retryable,
        }
    }

    fn from_github_response(
        status: u16,
        message: &str,
        documentation_url: Option<&str>,
    ) -> Retryability {
        match status {
            HTTP_FORBIDDEN if is_rate_limit_error(message, documentation_url) => {
                Retryability::Retryable
            }
            _ => Self::from_http_status(status),
        }
    }

    fn is_permanent_sqs_error_code(code: &str) -> bool {
        matches!(
            code,
            "AccessDenied"
                | "BatchEntryIdsNotDistinct"
                | "BatchRequestTooLong"
                | "BpError"
                | "EmptyBatchRequest"
                | "InvalidAddress"
                | "InvalidBatchEntryId"
                | "InvalidMessageContents"
                | "InvalidSecurity"
                | "KmsAccessDenied"
                | "KmsDisabled"
                | "KmsInvalidKeyUsage"
                | "KmsInvalidState"
                | "KmsNotFound"
                | "KmsOptInRequired"
                | "QueueDoesNotExist"
                | "TooManyEntriesInBatchRequest"
                | "UnsupportedOperation"
        )
    }
}

fn is_rate_limit_error(message: &str, documentation_url: Option<&str>) -> bool {
    mentions_rate_limit(message) || documentation_url.is_some_and(mentions_rate_limit)
}

fn mentions_rate_limit(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("rate limit")
        || value.contains("rate-limit")
        || value.contains("rate_limit")
        || value.contains("ratelimit")
}
