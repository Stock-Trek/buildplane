variable "role" {
  description = "Role to attach S3 put permission to"
  type        = string
}

variable "bucket_arn" {
  description = "Bucket arn to grant put access to"
  type        = string
}

variable "prefix" {
  description = "Key prefix within the bucket the role can write to"
  type        = string
  nullable    = true
  default     = null
}
