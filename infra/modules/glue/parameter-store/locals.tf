locals {
  prefix         = trim(var.prefix, "/")
  github_subject = "repo:${var.github_account}@${var.github_account_id}/${var.github_repo}@${var.github_repo_id}"
}
