variable "api_id" {
  description = "API Gateway id to integrate with"
  type        = string
}

variable "api_execution_arn" {
  description = "API Gateway execution ARN to grant invocation from"
  type        = string
}

variable "lambda_name" {
  description = "Name of the Lambda function to proxy requests to"
  type        = string
}

variable "lambda_invoke_arn" {
  description = "Invoke ARN of the Lambda function to proxy requests to"
  type        = string
}

variable "authorization_type" {
  description = "Authorization type for the routes"
  type        = string
  default     = "AWS_IAM"

  validation {
    condition = contains(
      ["AWS_IAM", "CUSTOM", "JWT", "NONE"],
      var.authorization_type
    )
    error_message = "authorization_type must be one of AWS_IAM, CUSTOM, JWT or NONE."
  }
}

variable "routes" {
  description = "Route keys to match requests against"
  type = list(object({
    method = string
    path   = string
  }))
  default = null

  validation {
    condition = var.routes == null || alltrue([
      for r in var.routes :
      contains(["ANY", "HEAD", "OPTIONS", "GET", "POST", "PUT", "PATCH", "DELETE"], r.method)
      && startswith(r.path, "/")
    ])
    error_message = "Each route must have a valid method (ANY, HEAD, OPTIONS, GET, POST, PUT, PATCH, DELETE) and a path beginning with '/'."
  }
}
