# update
# Image lambdas have their update role created by the ecr-repository module.
resource "aws_iam_role" "update" {
  count = local.zip_count

  name               = var.role_update
  assume_role_policy = data.aws_iam_policy_document.github_assume_web_identity.json
  tags               = var.tags
}

resource "aws_iam_policy" "update_lambda" {
  name   = "${var.role_update}-update-lambda"
  policy = data.aws_iam_policy_document.update_lambda.json
}

resource "aws_iam_role_policy_attachment" "update_lambda" {
  role       = var.role_update
  policy_arn = aws_iam_policy.update_lambda.arn
}

# run
resource "aws_iam_role" "run" {
  name               = local.role_run
  assume_role_policy = data.aws_iam_policy_document.assume_role.json
  tags               = var.tags
}

resource "aws_iam_policy" "write_logs" {
  name   = "${aws_iam_role.run.name}-write-logs"
  policy = data.aws_iam_policy_document.write_logs.json
}

resource "aws_iam_role_policy_attachment" "write_logs" {
  role       = aws_iam_role.run.name
  policy_arn = aws_iam_policy.write_logs.arn
}

resource "aws_iam_policy" "dead_letter_write" {
  name   = "${aws_iam_role.run.name}-dead-letter-write"
  policy = data.aws_iam_policy_document.dead_letter_write.json
}

resource "aws_iam_role_policy_attachment" "dead_letter_write" {
  role       = aws_iam_role.run.name
  policy_arn = aws_iam_policy.dead_letter_write.arn
}

resource "aws_iam_policy" "xray_write" {
  name   = "${aws_iam_role.run.name}-xray-write"
  policy = data.aws_iam_policy_document.xray_write.json
}

resource "aws_iam_role_policy_attachment" "xray_write" {
  role       = aws_iam_role.run.name
  policy_arn = aws_iam_policy.xray_write.arn
}
