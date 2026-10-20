module "api_ref_sync" {
  source = "../modules/api-gateway"
  name   = local.ref_sync_api_name
  tags   = local.tags
}

module "api_ref_sync_route_github" {
  source             = "../modules/glue/api-routes"
  api_id             = module.api_ref_sync.id
  api_execution_arn  = module.api_ref_sync.execution_arn
  lambda_name        = local.ref_sync_lambda_webhook
  lambda_invoke_arn  = module.lambda_ref_sync_webhook.invoke_arn
  authorization_type = "NONE"
  routes = [{
    method = "POST"
    path   = "/github"
  }]
  depends_on = [module.api_ref_sync, module.lambda_ref_sync_webhook]
}

module "lambda_invoke_ref_sync" {
  source       = "../modules/glue/lambda-invoke"
  role         = module.lambda_ref_sync_orchestrator.role_run_name
  function_arn = module.lambda_build_agent.arn
  depends_on   = [module.lambda_ref_sync_orchestrator, module.lambda_build_agent]
}

module "lambda_ref_sync_webhook" {
  source            = "../modules/lambda"
  name              = local.ref_sync_lambda_webhook
  github_account    = local.github_account
  github_account_id = local.github_account_id
  github_repo       = "ref-sync"
  github_repo_id    = "1413785530"
  github_oidc_arn   = data.aws_iam_openid_connect_provider.github.arn
  memory            = 1024
  timeout           = 30
  role_update       = local.ref_sync_webhook_update_role
  package_type      = "Zip"
  env_vars = {
    INFISICAL_BASE_URL    = "https://us.infisical.com"
    INFISICAL_IDENTITY_ID = "7bbc17ad-dedb-4297-a89b-07f266374a38"
    INFISICAL_PROJECT_ID  = "a6b3a1c2-b598-4d17-80ac-da8f47a953f2"
    INFISICAL_ENVIRONMENT = "prod"
    SQS_URL               = module.sqs_ref_sync.url
  }
  tags = local.tags
}

module "lambda_ref_sync_orchestrator" {
  source            = "../modules/lambda"
  name              = local.ref_sync_lambda_orchestrator
  github_account    = local.github_account
  github_account_id = local.github_account_id
  github_repo       = "ref-sync"
  github_repo_id    = "1413785530"
  github_oidc_arn   = data.aws_iam_openid_connect_provider.github.arn
  memory            = 1024
  timeout           = 30
  role_update       = local.ref_sync_orchestrator_update_role
  package_type      = "Zip"
  env_vars = {
    BUILD_AGENT_FUNCTION_ARN = module.lambda_build_agent.arn
  }
  tags = local.tags
}

module "sqs_lambda_ref_sync" {
  source      = "../modules/glue/sqs-lambda"
  queue_arn   = module.sqs_ref_sync.arn
  lambda_name = local.ref_sync_lambda_orchestrator
  role        = module.lambda_ref_sync_orchestrator.role_run_name
  depends_on  = [module.lambda_ref_sync_orchestrator, module.sqs_ref_sync]
}

module "sqs_ref_sync" {
  source             = "../modules/sqs"
  fifo               = false
  name               = local.ref_sync_sqs_name
  role               = module.lambda_ref_sync_webhook.role_run_name
  visibility_timeout = 300
  dead_letter = {
    max_receive_count = 3
  }
  tags = local.tags
}
