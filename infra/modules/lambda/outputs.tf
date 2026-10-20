output "arn" {
  value = aws_lambda_function.lambda.arn
}

output "invoke_arn" {
  value = aws_lambda_function.lambda.invoke_arn
}

output "url" {
  value = (local.public_count == 1) ? aws_lambda_function_url.public_url[0].function_url : null
}

output "role_update_name" {
  value = var.role_update
}

output "dead_letter_arn" {
  value = aws_sqs_queue.dead_letter.arn
}

output "role_run_name" {
  value = aws_iam_role.run.name
}

output "role_run_arn" {
  value = aws_iam_role.run.arn
}
