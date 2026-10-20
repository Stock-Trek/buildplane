resource "aws_iam_policy" "consume" {
  name   = "${var.role_name}-dynamodb-stream-consume"
  policy = data.aws_iam_policy_document.consume.json
}

resource "aws_iam_role_policy_attachment" "consume" {
  role       = var.role_name
  policy_arn = aws_iam_policy.consume.arn
}

resource "aws_iam_policy" "dlq_write" {
  name   = "${var.role_name}-dlq-write"
  policy = data.aws_iam_policy_document.dlq_write.json
}

resource "aws_iam_role_policy_attachment" "dlq_write" {
  role       = var.role_name
  policy_arn = aws_iam_policy.dlq_write.arn
}
