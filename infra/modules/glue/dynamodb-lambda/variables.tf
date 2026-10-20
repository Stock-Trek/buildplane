variable "stream_arn" {
  description = "DynamoDB stream arn to consume records from"
  type        = string
}

variable "lambda_arn" {
  description = "ARN of the Lambda function to invoke with DynamoDB stream records"
  type        = string
}

variable "role_name" {
  description = "Name of the role to attach DynamoDB stream consume and DLQ write permissions to"
  type        = string
}

variable "dlq_bucket_arn" {
  description = "Bucket arn to use as the on-failure destination for unprocessed records"
  type        = string
}

variable "batch_size" {
  description = "Maximum number of records to retrieve per batch"
  type        = number
  default     = 100
}

variable "maximum_batching_window_in_seconds" {
  description = "Maximum amount of time in seconds to gather records before invoking the function"
  type        = number
  nullable    = true
  default     = null
}

variable "starting_position" {
  description = "Position in the stream where Lambda starts reading. One of [TRIM_HORIZON, LATEST]"
  type        = string
  default     = "LATEST"

  validation {
    condition     = contains(["TRIM_HORIZON", "LATEST"], var.starting_position)
    error_message = "starting-position must be one of [TRIM_HORIZON, LATEST]."
  }
}

variable "bisect_batch_on_function_error" {
  description = "Whether to split the batch in two and retry when the function returns an error"
  type        = bool
  default     = false
}

variable "maximum_retry_attempts" {
  description = "Maximum number of times to retry a failed batch before sending records to the on-failure destination"
  type        = number
  nullable    = true
  default     = null
}

variable "maximum_record_age_in_seconds" {
  description = "Maximum age in seconds of a record before it is sent to the on-failure destination"
  type        = number
  nullable    = true
  default     = null
}
