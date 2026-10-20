resource "aws_s3_bucket" "bucket" {
  bucket = var.name
  tags   = var.tags
}

resource "aws_s3_bucket" "logs" {
  bucket = "${var.name}-logs"
  tags   = var.tags
}

locals {
  primary_buckets = {
    bucket = aws_s3_bucket.bucket.id
    logs   = aws_s3_bucket.logs.id
  }
}

resource "aws_s3_bucket_notification" "primary" {
  for_each    = local.primary_buckets
  bucket      = each.value
  eventbridge = true
}

resource "aws_s3_bucket_lifecycle_configuration" "primary" {
  for_each = local.primary_buckets
  bucket   = each.value

  rule {
    id     = "abort-incomplete-multipart-uploads"
    status = "Enabled"

    filter {}

    abort_incomplete_multipart_upload {
      days_after_initiation = 1
    }
  }
}

resource "aws_s3_bucket_versioning" "primary" {
  for_each = local.primary_buckets
  bucket   = each.value
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_public_access_block" "primary" {
  for_each                = local.primary_buckets
  bucket                  = each.value
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "primary" {
  for_each = local.primary_buckets
  bucket   = each.value
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
  depends_on = [aws_s3_bucket_public_access_block.primary]
}

resource "aws_s3_bucket_server_side_encryption_configuration" "primary" {
  for_each = local.primary_buckets
  bucket   = each.value
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = data.aws_kms_alias.s3.target_key_arn
    }
    blocked_encryption_types = ["SSE-C"]
    bucket_key_enabled       = true
  }
}

resource "aws_s3_bucket_logging" "access_logging" {
  bucket        = aws_s3_bucket.bucket.id
  target_bucket = aws_s3_bucket.logs.id
  target_prefix = "logs/"
}

resource "aws_s3_bucket_policy" "secure_transport_policy" {
  bucket = aws_s3_bucket.bucket.id
  policy = data.aws_iam_policy_document.secure_transport.json
}

resource "aws_s3_bucket_policy" "logs" {
  bucket = aws_s3_bucket.logs.id
  policy = data.aws_iam_policy_document.logs.json
}
