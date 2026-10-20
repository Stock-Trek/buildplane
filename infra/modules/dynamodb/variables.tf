variable "name" {
  description = "Table name"
  type        = string
}

variable "attributes" {
  description = "Map of table attribute names to types. Each type must be one of [S, N, B]"
  type        = map(string)

  validation {
    condition = alltrue([
      for type in values(var.attributes) :
      contains(["S", "N", "B"], type)
    ])
    error_message = "Each attribute type must be one of [S, N, B]."
  }
}

variable "hash_key" {
  description = "Partition key attribute name. Must be one of the attribute keys"
  type        = string

  validation {
    condition     = contains(keys(var.attributes), var.hash_key)
    error_message = "hash-key must be one of the attribute keys."
  }
}

variable "range_key" {
  description = "Optional sort key attribute name. Must be one of the attribute keys when set"
  type        = string
  nullable    = true
  default     = null

  validation {
    condition = (
      var.range_key == null ||
      try(contains(keys(var.attributes), var.range_key), false)
    )
    error_message = "range-key must be null or one of the attribute keys."
  }
}

variable "global_secondary_indexes" {
  description = "Map of global secondary index names to their configuration"
  type = map(object({
    hash_key           = string
    range_key          = optional(string)
    projection_type    = string
    non_key_attributes = optional(list(string), [])
    read_capacity      = optional(number)
    write_capacity     = optional(number)
  }))
  default = {}

  validation {
    condition = alltrue([
      for index in values(var.global_secondary_indexes) :
      contains(keys(var.attributes), index.hash_key)
    ])
    error_message = "Each global secondary index hash-key must be one of the attribute keys."
  }

  validation {
    condition = alltrue([
      for index in values(var.global_secondary_indexes) :
      index.range_key == null || try(contains(keys(var.attributes), index.range_key), false)
    ])
    error_message = "Each global secondary index range-key must be null or one of the attribute keys."
  }

  validation {
    condition = alltrue([
      for index in values(var.global_secondary_indexes) :
      contains(["ALL", "INCLUDE", "KEYS_ONLY"], index.projection_type)
    ])
    error_message = "Each global secondary index projection-type must be one of [ALL, INCLUDE, KEYS_ONLY]."
  }

  validation {
    condition = alltrue([
      for index in values(var.global_secondary_indexes) :
      index.projection_type != "INCLUDE" || length(index.non_key_attributes) > 0
    ])
    error_message = "Each global secondary index with projection-type INCLUDE must set at least one non-key-attribute."
  }
}

variable "billing" {
  description = "Billing configuration. read-capacity and write-capacity are only valid when mode is PROVISIONED"
  type = object({
    mode           = string
    read_capacity  = optional(number)
    write_capacity = optional(number)
  })
  default = { mode = "PAY_PER_REQUEST" }

  validation {
    condition     = contains(["PAY_PER_REQUEST", "PROVISIONED"], var.billing.mode)
    error_message = "mode must be one of ['PAY_PER_REQUEST', 'PROVISIONED']."
  }
}

variable "stream_view_type" {
  description = "Stream view type. Streams are enabled when set. One of [KEYS_ONLY, NEW_IMAGE, OLD_IMAGE, NEW_AND_OLD_IMAGES]"
  type        = string
  nullable    = true
  default     = null

  validation {
    condition = var.stream_view_type == null || try(contains([
      "KEYS_ONLY",
      "NEW_IMAGE",
      "OLD_IMAGE",
      "NEW_AND_OLD_IMAGES"
    ], var.stream_view_type), false)
    error_message = "stream-view-type must be one of [null, KEYS_ONLY, NEW_IMAGE, OLD_IMAGE, NEW_AND_OLD_IMAGES]."
  }
}

variable "ttl_attribute" {
  description = "Optional attribute name to use for time to live"
  type        = string
  nullable    = true
  default     = null
}

variable "point_in_time_recovery" {
  description = "Whether to enable point in time recovery"
  type        = bool
  default     = true
}

variable "tags" {
  description = "Tags"
  type        = map(string)
  nullable    = true
  default     = {}
}
