resource "aws_sqs_queue" "queue" {
  name                        = local.queue_name
  fifo_queue                  = var.fifo
  content_based_deduplication = local.content_based_deduplication
  deduplication_scope         = local.deduplication_scope
  receive_wait_time_seconds   = local.receive_wait
  visibility_timeout_seconds  = var.visibility_timeout
  sqs_managed_sse_enabled     = true
  redrive_policy = var.dead_letter == null ? null : jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dead_letter[0].arn
    maxReceiveCount     = var.dead_letter.max_receive_count
  })
  tags = var.tags
}

resource "aws_sqs_queue" "dead_letter" {
  count = local.dead_letter_count

  name                      = local.dead_letter_name
  fifo_queue                = var.fifo
  receive_wait_time_seconds = local.receive_wait
  sqs_managed_sse_enabled   = true
  tags                      = var.tags
}

resource "aws_sqs_queue_redrive_allow_policy" "dead_letter" {
  count = local.dead_letter_count

  queue_url = aws_sqs_queue.dead_letter[0].id
  redrive_allow_policy = jsonencode({
    redrivePermission = "byQueue"
    sourceQueueArns   = [aws_sqs_queue.queue.arn]
  })
}
