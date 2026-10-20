locals {
  github_subject = "repo:${var.github_account}@${var.github_account_id}/${var.github_repo}@${var.github_repo_id}"

  remove_untagged = {
    rules = [
      {
        rulePriority = 1
        description  = "Keep at most 1 untagged image"
        selection = {
          tagStatus   = "untagged"
          countType   = "imageCountMoreThan"
          countNumber = 1
        }
        action = {
          type = "expire"
        }
      }
    ]
  }
}
