data "aws_iam_policy_document" "github_assume_web_identity" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [var.github_oidc_arn]
    }
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["${local.github_subject}:ref:refs/heads/${var.branch_name}"]
    }
  }
}

data "aws_iam_policy_document" "parameter_store_read" {
  statement {
    effect = "Allow"
    actions = [
      "ssm:GetParameter",
      "ssm:GetParameters",
      "ssm:GetParametersByPath"
    ]
    resources = [
      "arn:aws:ssm:*:*:parameter/${local.prefix}/*",
      "arn:aws:ssm:*:*:parameter/${local.prefix}"
    ]
  }
}

data "aws_iam_policy_document" "parameter_store_write" {
  statement {
    effect = "Allow"
    actions = [
      "ssm:PutParameter",
      "ssm:DeleteParameter",
      "ssm:DeleteParameters"
    ]
    resources = [
      "arn:aws:ssm:*:*:parameter/${local.prefix}/*",
      "arn:aws:ssm:*:*:parameter/${local.prefix}"
    ]
  }
}
