output "arn" {
  value = aws_ecr_repository.repository.arn
}

output "repository_url" {
  value = aws_ecr_repository.repository.repository_url
}

output "role_update_name" {
  value = aws_iam_role.update.name
}
