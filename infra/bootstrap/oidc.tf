data "aws_iam_openid_connect_provider" "bootstrap_github" {
  url = "https://token.actions.githubusercontent.com"
}
