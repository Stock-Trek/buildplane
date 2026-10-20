resource "docker_image" "dummy" {
  count = local.dummy_image_count

  name         = "alpine:latest"
  platform     = var.architecture == "arm64" ? "linux/arm64" : "linux/amd64"
  keep_locally = false

  lifecycle {
    ignore_changes = all
  }
}

resource "docker_tag" "dummy" {
  count = local.dummy_image_count

  source_image = docker_image.dummy[0].name
  target_image = local.package_config.image_uri

  lifecycle {
    ignore_changes = all
  }
}

resource "docker_registry_image" "dummy" {
  count = local.dummy_image_count

  name = docker_tag.dummy[0].target_image

  lifecycle {
    ignore_changes = all
  }
}
