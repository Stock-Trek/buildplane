data "aws_iam_policy_document" "push_image" {
  count = local.image_count

  statement {
    effect    = "Allow"
    actions   = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }
  statement {
    effect = "Allow"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:BatchGetImage",
      "ecr:CompleteLayerUpload",
      "ecr:GetDownloadUrlForLayer",
      "ecr:InitiateLayerUpload",
      "ecr:PutImage",
      "ecr:UploadLayerPart"
    ]
    resources = [var.image.ecr_repository_arn]
  }
}

data "aws_iam_policy_document" "pull_image" {
  count = local.image_count

  statement {
    effect = "Allow"
    actions = [
      "ecr:GetAuthorizationToken",
      "ecr:BatchGetImage",
      "ecr:GetDownloadUrlForLayer"
    ]
    resources = [var.image.ecr_repository_arn]
  }
}

resource "aws_iam_policy" "push_image" {
  count = local.image_count

  name   = "${var.role_update}-push-image"
  policy = data.aws_iam_policy_document.push_image[0].json
}

resource "aws_iam_role_policy_attachment" "push_image" {
  count = local.image_count

  role       = var.role_update
  policy_arn = aws_iam_policy.push_image[0].arn
}

resource "aws_iam_policy" "pull_image" {
  count = local.image_count

  name   = "${aws_iam_role.run.name}-pull-image"
  policy = data.aws_iam_policy_document.pull_image[0].json
}

resource "aws_iam_role_policy_attachment" "pull_image" {
  count = local.image_count

  role       = aws_iam_role.run.name
  policy_arn = aws_iam_policy.pull_image[0].arn
}
