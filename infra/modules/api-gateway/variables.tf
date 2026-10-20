variable "name" {
  description = "API Gateway name"
  type        = string
}

variable "tags" {
  description = "Tags"
  type        = map(string)
  nullable    = true
  default     = {}
}

variable "cors" {
  description = "Optional CORS configuration, no CORS configuration is applied when null"
  type = object({
    allow_origins = list(string)
  })
  nullable = true
  default  = null
}

# variable "log_retention_days" {
#   description = "CloudWatch access log retention in days"
#   type        = number
#   default     = 365
# }
