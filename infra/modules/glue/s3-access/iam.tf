resource "aws_iam_policy" "s3_access" {
  name   = "${var.role}-s3-access"
  policy = data.aws_iam_policy_document.s3_access.json
}

resource "aws_iam_role_policy_attachment" "s3_access" {
  role       = var.role
  policy_arn = aws_iam_policy.s3_access.arn
}
