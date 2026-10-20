resource "aws_iam_role" "parameter_store_write" {
  name               = var.writer_role_name
  assume_role_policy = data.aws_iam_policy_document.github_assume_web_identity.json
}


resource "aws_iam_policy" "parameter_store_read" {
  name   = "parameter-store-read-${replace(local.prefix, "/", "-")}"
  policy = data.aws_iam_policy_document.parameter_store_read.json
}

resource "aws_iam_policy" "parameter_store_write" {
  name   = "parameter-store-write-${replace(local.prefix, "/", "-")}"
  policy = data.aws_iam_policy_document.parameter_store_write.json
}


resource "aws_iam_role_policy_attachment" "parameter_store_read" {
  role       = var.lambda_role_name
  policy_arn = aws_iam_policy.parameter_store_read.arn
}

resource "aws_iam_role_policy_attachment" "parameter_store_write" {
  role       = var.writer_role_name
  policy_arn = aws_iam_policy.parameter_store_write.arn
}
