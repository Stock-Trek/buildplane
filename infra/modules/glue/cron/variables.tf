variable "name" {
  description = "Event rule name"
  type        = string
}

variable "lambda_arn" {
  description = "Lambda arn to run"
  type        = string
}

variable "lambda_name" {
  description = "Lambda function name"
  type        = string
}

variable "cron" {
  description = "Cron expression to run on"
  type        = string
}

variable "retries" {
  description = "Max retries"
  type        = number
  default     = 1
}

variable "retry_expiry_seconds" {
  description = "When to stop retrying in seconds"
  type        = number
  default     = 60
}

variable "tags" {
  description = "Tags"
  type        = map(string)
  nullable    = true
  default     = {}
}
