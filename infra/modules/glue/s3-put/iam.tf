resource "aws_iam_policy" "s3_put" {
  name   = "${var.role}-s3-put"
  policy = data.aws_iam_policy_document.s3_put.json
}

resource "aws_iam_role_policy_attachment" "s3_put" {
  role       = var.role
  policy_arn = aws_iam_policy.s3_put.arn
}
