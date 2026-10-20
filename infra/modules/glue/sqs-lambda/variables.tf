variable "queue_arn" {
  description = "SQS queue arn to consume messages from"
  type        = string
}

variable "lambda_name" {
  description = "Name of the Lambda function to invoke with SQS messages"
  type        = string
}

variable "role" {
  description = "Role to attach SQS consume permission to"
  type        = string
}

variable "batch_size" {
  description = "Maximum number of messages to retrieve per batch"
  type        = number
  default     = 10
}

variable "maximum_batching_window_in_seconds" {
  description = "Maximum amount of time in seconds to gather records before invoking the function"
  type        = number
  nullable    = true
  default     = null
}

variable "enabled" {
  description = "Whether the event source mapping is enabled"
  type        = bool
  default     = true
}
