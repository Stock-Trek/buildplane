locals {
  protocol_type = "HTTP"
  stage_name    = "$default"
  auto_deploy   = true

  cors_count          = var.cors == null ? 0 : 1
  cors_allow_headers  = ["*"]
  cors_allow_methods  = ["*"]
  cors_expose_headers = []
  cors_max_age        = 86400
}
