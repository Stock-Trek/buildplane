module "bucket_build_agent_artifacts" {
  source = "../modules/s3-bucket"
  name   = local.build_agent_rust_bucket
  tags   = local.tags

  providers = {
    aws         = aws
    aws.replica = aws.us_east_1
  }
}

module "ecr_build_agent" {
  source                      = "../modules/ecr-repository"
  name                        = local.build_agent_rust_ecr_name
  mutability_exclusion_filter = local.build_agent_rust_ecr_image_tag
  github_account              = local.github_account
  github_account_id           = local.github_account_id
  github_repo                 = "build-agent"
  github_repo_id              = "1395007534"
  github_oidc_arn             = data.aws_iam_openid_connect_provider.github.arn
  role_update                 = local.build_agent_rust_update_role
  tags                        = local.tags
}

module "lambda_build_agent" {
  source            = "../modules/lambda"
  name              = local.build_agent_rust_lambda
  github_account    = local.github_account
  github_account_id = local.github_account_id
  github_repo       = "build-agent"
  github_repo_id    = "1395007534"
  github_oidc_arn   = data.aws_iam_openid_connect_provider.github.arn
  architecture      = "arm64"
  memory            = 4096
  timeout           = 900
  role_update       = module.ecr_build_agent.role_update_name
  package_type      = "Image"
  image = {
    ecr_repository_arn = module.ecr_build_agent.arn
    repository_url     = module.ecr_build_agent.repository_url
    image_tag          = local.build_agent_rust_ecr_image_tag
  }
  tags = local.tags
}

module "s3_put_build_agent" {
  source     = "../modules/glue/s3-put"
  role       = module.lambda_build_agent.role_run_name
  bucket_arn = module.bucket_build_agent_artifacts.arn
  prefix     = local.build_agent_rust_artifact_prefix
}
