module "website_stock_trek" {
  source                      = "github.com/NathanJAdams/aws_s3_website.git?ref=b2209adba84594e48714e2b2bc273b55017bbef8"
  bare_domain                 = local.website_bare_domain
  error_file                  = null
  oidc_role_name              = "StockTrekUpdateWebsite"
  oidc_connector              = "GitHub"
  oidc_github_account_name    = "${local.github_account}@${local.github_account_id}"
  oidc_github_repository_name = local.github_repository_name_website
  tags                        = local.tags
  providers = {
    aws           = aws
    aws.us_east_1 = aws.us_east_1
  }
}
