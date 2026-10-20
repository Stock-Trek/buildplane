locals {
  prefix = var.prefix == null ? "" : "${trim(var.prefix, "/")}/"
}
