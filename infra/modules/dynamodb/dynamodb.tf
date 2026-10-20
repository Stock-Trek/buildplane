resource "aws_dynamodb_table" "table" {
  name             = var.name
  billing_mode     = var.billing.mode
  read_capacity    = var.billing.read_capacity
  write_capacity   = var.billing.write_capacity
  hash_key         = var.hash_key
  range_key        = var.range_key
  stream_enabled   = var.stream_view_type != null
  stream_view_type = var.stream_view_type

  dynamic "attribute" {
    for_each = var.attributes
    content {
      name = attribute.key
      type = attribute.value
    }
  }

  dynamic "global_secondary_index" {
    for_each = var.global_secondary_indexes
    content {
      name            = global_secondary_index.key
      projection_type = global_secondary_index.value.projection_type
      non_key_attributes = (
        global_secondary_index.value.projection_type == "INCLUDE"
      ) ? global_secondary_index.value.non_key_attributes : null
      read_capacity  = global_secondary_index.value.read_capacity
      write_capacity = global_secondary_index.value.write_capacity

      dynamic "key_schema" {
        for_each = concat(
          [{
            attribute_name = global_secondary_index.value.hash_key
            key_type       = "HASH"
          }],
          global_secondary_index.value.range_key == null ? [] : [{
            attribute_name = global_secondary_index.value.range_key
            key_type       = "RANGE"
          }]
        )
        content {
          attribute_name = key_schema.value.attribute_name
          key_type       = key_schema.value.key_type
        }
      }
    }
  }

  ttl {
    attribute_name = var.ttl_attribute
    enabled        = var.ttl_attribute != null
  }

  point_in_time_recovery {
    enabled = var.point_in_time_recovery
  }

  server_side_encryption {
    enabled     = true
    kms_key_arn = data.aws_kms_alias.dynamodb.target_key_arn
  }

  tags = var.tags
}
