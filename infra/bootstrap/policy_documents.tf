# plan
data "aws_iam_policy_document" "bootstrap_plan_lock" {
  statement {
    sid    = "TerraformStateLock"
    effect = "Allow"
    actions = [
      "s3:DeleteObject",
      "s3:PutObject"
    ]
    resources = ["${module.bucket_bootstrap.arn}/*.tflock"]
  }
}

data "aws_iam_policy_document" "bootstrap_assume_role_plan" {
  statement {
    sid    = "AssumeRoleGitHub"
    effect = "Allow"
    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.bootstrap_github.arn]
    }
    actions = ["sts:AssumeRoleWithWebIdentity"]
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${local.github_account}@${local.github_account_id}/${local.github_repo}@${local.github_repo_id}:*"]
    }
  }
}


# apply
data "aws_iam_policy_document" "bootstrap_assume_role_apply" {
  statement {
    sid    = "AssumeRoleGitHub"
    effect = "Allow"
    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.bootstrap_github.arn]
    }
    actions = ["sts:AssumeRoleWithWebIdentity"]
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${local.github_account}@${local.github_account_id}/${local.github_repo}@${local.github_repo_id}:ref:refs/heads/${local.github_branch}"]
    }
  }
}
