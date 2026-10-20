output "api_ref_sync_url" {
  value = module.api_ref_sync.url
}

output "bucket_build_agent_artifacts_arn" {
  value = module.bucket_build_agent_artifacts.arn
}

output "bucket_market_data_arn" {
  value = module.bucket_market_data.arn
}

output "ecr_repository_build_agent_url" {
  value = module.ecr_build_agent.repository_url
}

output "ecr_repository_build_agent_update_role" {
  value = module.lambda_build_agent.role_update_name
}

output "lambda_build_agent_arn" {
  value = module.lambda_build_agent.arn
}

output "lambda_market_data_arn" {
  value = module.lambda_market_data.arn
}

output "lambda_market_data_update_role" {
  value = module.lambda_market_data.role_update_name
}

output "lambda_ref_sync_orchestrator_arn" {
  value = module.lambda_ref_sync_orchestrator.arn
}

output "lambda_ref_sync_orchestrator_update_role" {
  value = module.lambda_ref_sync_orchestrator.role_update_name
}

output "lambda_ref_sync_webhook_arn" {
  value = module.lambda_ref_sync_webhook.arn
}

output "lambda_ref_sync_webhook_update_role" {
  value = module.lambda_ref_sync_webhook.role_update_name
}

output "website_bucket" {
  value = module.website_stock_trek.bucket
}

output "website_bucket_update_role" {
  value = module.website_stock_trek.role
}

output "website_cloudfront_distribution_id" {
  value = module.website_stock_trek.cloudfront_distribution_id
}

output "website_url" {
  value = module.website_stock_trek.url
}
