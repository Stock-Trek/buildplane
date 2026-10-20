resource "aws_iam_policy" "ecs_run_task" {
  name   = "${var.role}-ecs-run-task"
  policy = data.aws_iam_policy_document.ecs_run_task.json
}

resource "aws_iam_role_policy_attachment" "ecs_run_task" {
  role       = var.role
  policy_arn = aws_iam_policy.ecs_run_task.arn
}
