resource "aws_lambda_function" "lambda" {
  function_name = var.name
  package_type  = var.package_type

  # Image
  image_uri = try(local.package_config.image_uri, null)

  # Zip
  filename         = try(local.package_config.filename, null)
  handler          = try(local.package_config.handler, null)
  runtime          = try(local.package_config.runtime, null)
  source_code_hash = try(local.package_config.source_code_hash, null)

  role                           = aws_iam_role.run.arn
  architectures                  = [var.architecture]
  memory_size                    = var.memory
  timeout                        = var.timeout
  reserved_concurrent_executions = var.reserved_concurrent_executions
  # code_signing_config_arn        = aws_lambda_code_signing_config.lambda.arn
  kms_key_arn = data.aws_kms_alias.lambda.target_key_arn
  tracing_config {
    mode = "Active"
  }
  environment {
    variables = var.env_vars
  }
  dead_letter_config {
    target_arn = aws_sqs_queue.dead_letter.arn
  }
  tags = var.tags
  depends_on = [
    aws_iam_role_policy_attachment.write_logs,
    aws_iam_role_policy_attachment.xray_write,
    aws_iam_role_policy_attachment.dead_letter_write,
    aws_iam_role_policy_attachment.pull_image,
    docker_registry_image.dummy,
    data.archive_file.dummy,
  ]
  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
      image_uri,
    ]
  }
}
