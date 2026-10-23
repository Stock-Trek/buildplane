use crate::{context::RsOrchestratorContext, tasks::task::Task};
use aws_lambda_events::event::sqs::{SqsBatchResponse, SqsEvent, SqsMessage as SqsRecord};
use buildplane_errors::SqsError;
use buildplane_errors::{BpError, BpResult};
use buildplane_messages::sqs_message::SqsMessage;
use buildplane_metrics::route::RouteMetrics;
use buildplane_metrics::sqs::SqsMetrics;
use buildplane_time::deadline::Deadline;
use buildplane_time::request_context::RequestContext;
use futures::{StreamExt, stream};
use lambda_runtime::LambdaEvent;

pub struct SqsHandler;

impl SqsHandler {
    pub async fn handle(
        &self,
        context: &RsOrchestratorContext,
        event: LambdaEvent<SqsEvent>,
    ) -> BpResult<SqsBatchResponse> {
        let request_context = RequestContext {
            context,
            deadline: Deadline::new(event.context.deadline(), context.deadline_buffer),
        };
        let process_concurrency = context.sqs_process_concurrency;
        let results = stream::iter(event.payload.records)
            .map(|record| {
                let request_context = request_context.clone();
                async move {
                    let result = self.handle_sqs_record(request_context, &record).await;
                    (record, result)
                }
            })
            .buffer_unordered(process_concurrency)
            .collect::<Vec<_>>()
            .await;
        let mut response = SqsBatchResponse::default();
        for (record, result) in results {
            if let Err(error) = result {
                if !error.is_retryable() {
                    tracing::error!(
                        message_id = record.message_id.as_deref().unwrap_or("unknown"),
                        %error,
                        "Discarding SQS message with a permanent error",
                    );
                    continue;
                }
                let message_id = record
                    .message_id
                    .ok_or_else(|| BpError::Sqs(SqsError::MissingMessageId))?;
                tracing::error!(
                    message_id = %message_id,
                    %error,
                    "Failed to process SQS message, reporting batch item failure",
                );
                response.add_failure(message_id);
            }
        }
        Ok(response)
    }
}

impl SqsHandler {
    async fn handle_sqs_record(
        &self,
        request_context: RequestContext<&RsOrchestratorContext>,
        record: &SqsRecord,
    ) -> BpResult<()> {
        let body = record
            .body
            .as_deref()
            .ok_or_else(|| BpError::Sqs(SqsError::MissingMessageBody))?;
        let message: SqsMessage = serde_json::from_str(body)?;
        let dedup = request_context.context.dedup.clone();
        if dedup.is_duplicate(&message) {
            tracing::info!(
                message_id = record.message_id.as_deref().unwrap_or("unknown"),
                "Ignoring duplicate SQS message",
            );
            SqsMetrics::record_duplicate_message();
            return Ok(());
        }
        let task = Task::from(message.clone());
        let metrics = RouteMetrics::start(task.route());
        let result = task.handle(request_context).await;
        match &result {
            Ok(()) => metrics.record_success(),
            Err(error) => {
                dedup.forget(&message);
                metrics.record_error(error.error_type());
            }
        }
        result
    }
}
