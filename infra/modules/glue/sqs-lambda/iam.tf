resource "aws_iam_policy" "consume" {
  name   = "${var.role}-sqs-consume"
  policy = data.aws_iam_policy_document.consume.json
}

resource "aws_iam_role_policy_attachment" "consume" {
  role       = var.role
  policy_arn = aws_iam_policy.consume.arn
}
