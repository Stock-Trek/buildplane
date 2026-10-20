output "cluster_arn" {
  value = aws_ecs_cluster.cluster.arn
}

output "cluster_name" {
  value = aws_ecs_cluster.cluster.name
}

output "container_name" {
  value = var.name
}

output "execution_role_arn" {
  value = aws_iam_role.execution.arn
}

output "task_definition_arn" {
  value = aws_ecs_task_definition.task.arn
}

output "task_role_arn" {
  value = aws_iam_role.task.arn
}

output "task_role_name" {
  value = aws_iam_role.task.name
}
