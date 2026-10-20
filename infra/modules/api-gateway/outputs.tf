output "id" {
  value = aws_apigatewayv2_api.api.id
}

output "arn" {
  value = aws_apigatewayv2_api.api.arn
}

output "execution_arn" {
  value = aws_apigatewayv2_api.api.execution_arn
}

output "url" {
  value = aws_apigatewayv2_stage.stage.invoke_url
}
