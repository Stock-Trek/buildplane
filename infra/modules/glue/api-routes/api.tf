resource "aws_apigatewayv2_integration" "lambda" {
  api_id                 = var.api_id
  integration_type       = local.integration_type
  integration_uri        = var.lambda_invoke_arn
  payload_format_version = local.payload_format_version
}

resource "aws_apigatewayv2_route" "route" {
  for_each = toset(local.route_keys)

  api_id             = var.api_id
  route_key          = each.value
  target             = "integrations/${aws_apigatewayv2_integration.lambda.id}"
  authorization_type = var.authorization_type
}

resource "aws_lambda_permission" "allow_api" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = var.lambda_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${var.api_execution_arn}/*/*"
}
