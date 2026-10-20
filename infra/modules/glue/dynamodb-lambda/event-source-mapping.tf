resource "aws_lambda_event_source_mapping" "dynamodb" {
  event_source_arn                   = var.stream_arn
  function_name                      = var.lambda_arn
  batch_size                         = var.batch_size
  maximum_batching_window_in_seconds = var.maximum_batching_window_in_seconds
  starting_position                  = var.starting_position
  bisect_batch_on_function_error     = var.bisect_batch_on_function_error
  maximum_retry_attempts             = var.maximum_retry_attempts
  maximum_record_age_in_seconds      = var.maximum_record_age_in_seconds
  enabled                            = true

  destination_config {
    on_failure {
      destination_arn = var.dlq_bucket_arn
    }
  }

  depends_on = [
    aws_iam_role_policy_attachment.consume,
    aws_iam_role_policy_attachment.dlq_write,
  ]
}
