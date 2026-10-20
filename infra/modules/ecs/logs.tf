# resource "aws_cloudwatch_log_group" "logs" {
#   name              = "/ecs/${var.name}"
#   retention_in_days = var.log_retention_days
#   kms_key_id        = local.aws_logs_kms_arn # data.aws_kms_alias.logs.target_key_arn
#   tags              = var.tags
# }
