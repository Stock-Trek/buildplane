data "aws_iam_policy_document" "s3_put" {
  statement {
    effect    = "Allow"
    actions   = ["s3:PutObject"]
    resources = ["${var.bucket_arn}/${local.prefix}*"]
  }
}
