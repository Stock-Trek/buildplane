data "aws_iam_policy_document" "consume" {
  statement {
    effect = "Allow"
    actions = [
      "dynamodb:DescribeStream",
      "dynamodb:GetRecords",
      "dynamodb:GetShardIterator",
      "dynamodb:ListStreams"
    ]
    resources = [
      var.stream_arn,
      "${var.stream_arn}/*"
    ]
  }
}

data "aws_iam_policy_document" "dlq_write" {

  statement {
    effect    = "Allow"
    actions   = ["s3:ListBucket"]
    resources = [var.dlq_bucket_arn]
  }
  statement {
    effect    = "Allow"
    actions   = ["s3:PutObject"]
    resources = ["${var.dlq_bucket_arn}/*"]
  }
}
