resource "aws_lambda_function_url" "public_url" {
  count = local.public_count

  function_name      = aws_lambda_function.lambda.function_name
  authorization_type = "AWS_IAM"
  cors {
    allow_origins = [
      "https://${var.public_access_cors_bare_domain}",
      "https://www.${var.public_access_cors_bare_domain}"
    ]
    allow_headers  = ["*"]
    allow_methods  = ["POST", "GET"]
    expose_headers = []
    max_age        = 86400
  }
}
