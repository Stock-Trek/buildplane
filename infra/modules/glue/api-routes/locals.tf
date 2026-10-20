locals {
  integration_type       = "AWS_PROXY"
  payload_format_version = "2.0"
  route_keys = var.routes == null ? tolist(["$default"]) : [
    for r in var.routes : "${r.method} ${r.path}"
  ]
}
