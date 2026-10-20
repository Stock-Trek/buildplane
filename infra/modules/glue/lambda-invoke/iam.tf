resource "aws_iam_policy" "lambda_invoke" {
  name   = "${var.role}-lambda-invoke"
  policy = data.aws_iam_policy_document.lambda_invoke.json
}

resource "aws_iam_role_policy_attachment" "lambda_invoke" {
  role       = var.role
  policy_arn = aws_iam_policy.lambda_invoke.arn
}
