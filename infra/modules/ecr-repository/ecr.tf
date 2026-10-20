resource "aws_ecr_repository" "repository" {
  name                 = var.name
  image_tag_mutability = var.mutability_exclusion_filter != null ? "IMMUTABLE_WITH_EXCLUSION" : "IMMUTABLE"
  dynamic "image_tag_mutability_exclusion_filter" {
    for_each = var.mutability_exclusion_filter != null ? [var.mutability_exclusion_filter] : []
    content {
      filter_type = "WILDCARD"
      filter      = image_tag_mutability_exclusion_filter.value
    }
  }
  encryption_configuration {
    encryption_type = "KMS"
  }
  image_scanning_configuration {
    scan_on_push = true
  }
  tags = var.tags
}

resource "aws_ecr_lifecycle_policy" "remove_untagged" {
  repository = aws_ecr_repository.repository.name
  policy     = jsonencode(local.remove_untagged)
}
