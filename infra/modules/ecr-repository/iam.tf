resource "aws_iam_role" "update" {
  name               = var.role_update
  assume_role_policy = data.aws_iam_policy_document.github_assume_web_identity.json
  tags               = var.tags
}

resource "aws_iam_policy" "update_image" {
  name   = "${aws_iam_role.update.name}-update-image"
  policy = data.aws_iam_policy_document.push_image.json
}

resource "aws_iam_role_policy_attachment" "update_image" {
  role       = aws_iam_role.update.name
  policy_arn = aws_iam_policy.update_image.arn
}
