resource "aws_sqs_queue" "dead_letter" {
  name                      = "${var.name}-dead-letter"
  message_retention_seconds = 1209600
  sqs_managed_sse_enabled   = true
  tags                      = var.tags
}
