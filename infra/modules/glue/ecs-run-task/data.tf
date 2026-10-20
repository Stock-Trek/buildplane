data "aws_iam_policy_document" "ecs_run_task" {
  statement {
    effect    = "Allow"
    actions   = ["ecs:RunTask"]
    resources = ["${local.task_definition_family_arn}:*"]
  }

  statement {
    effect    = "Allow"
    actions   = ["ecs:DescribeTasks"]
    resources = ["${local.cluster_task_arn_prefix}/*"]
  }

  statement {
    effect  = "Allow"
    actions = ["iam:PassRole"]
    resources = [
      var.execution_role_arn,
      var.task_role_arn
    ]
    condition {
      test     = "StringEquals"
      variable = "iam:PassedToService"
      values   = ["ecs-tasks.amazonaws.com"]
    }
  }
}
