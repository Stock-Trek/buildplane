module "bucket_market_data" {
  source = "../modules/s3-bucket"
  name   = local.market_data_bucket
  tags   = local.tags

  providers = {
    aws         = aws
    aws.replica = aws.us_east_1
  }
}

module "lambda_market_data" {
  source            = "../modules/lambda"
  name              = local.market_data_lambda
  github_account    = local.github_account
  github_account_id = local.github_account_id
  github_repo       = "market-data-updater"
  github_repo_id    = "1215979880"
  github_oidc_arn   = data.aws_iam_openid_connect_provider.github.arn
  memory            = 2048
  timeout           = 300
  role_update       = local.market_data_update_role
  package_type      = "Zip"
  tags              = local.tags
}

module "s3_access_market_data" {
  source      = "../modules/glue/s3-access"
  role        = module.lambda_market_data.role_run_name
  bucket_arns = [module.bucket_market_data.arn]
  depends_on  = [module.lambda_market_data]
}

# module "cron_update_market_data" {
#   source      = "../modules/glue/cron"
#   name        = local.market_data_event_rule
#   cron        = local.market_data_cron
#   lambda_arn  = module.lambda_market_data.arn
#   lambda_name = local.market_data_lambda
# }
