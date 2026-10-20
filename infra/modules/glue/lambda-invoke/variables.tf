variable "role" {
  description = "Role to attach Lambda invoke permission to"
  type        = string
}

variable "function_arn" {
  description = "ARN of the Lambda function to allow invoking"
  type        = string
}
