data "aws_iam_policy_document" "lambda_invoke" {
  statement {
    effect    = "Allow"
    actions   = ["lambda:InvokeFunction"]
    resources = [var.function_arn]
  }
}
