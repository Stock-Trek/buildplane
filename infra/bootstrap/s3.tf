module "bucket_bootstrap" {
  source = "../modules/s3-bucket"
  name   = "state.${local.bare_domain}"
  tags   = {}

  providers = {
    aws         = aws
    aws.replica = aws.replica
  }
}
