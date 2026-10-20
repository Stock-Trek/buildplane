locals {
  github_subject = "repo:${var.github_account}@${var.github_account_id}/${var.github_repo}@${var.github_repo_id}"

  image_count       = (var.package_type == "Image" ? 1 : 0)
  dummy_image_count = (var.package_type == "Image" && var.create_dummy_image ? 1 : 0)
  zip_count         = (var.package_type == "Zip" ? 1 : 0)
  public_count      = (var.public_access_cors_bare_domain == null ? 0 : 1)
  role_run          = "${var.name}-run"
  package_config = var.package_type == "Image" ? {
    image_uri        = "${var.image.repository_url}:${var.image.image_tag}"
    filename         = null
    handler          = null
    runtime          = null
    source_code_hash = null
    } : {
    image_uri        = null
    filename         = data.archive_file.dummy[0].output_path
    handler          = var.zip.handler
    runtime          = var.zip.runtime
    source_code_hash = data.archive_file.dummy[0].output_base64sha256
  }
}
