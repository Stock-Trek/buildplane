variable "name" {
  description = "ECS cluster, task definition and log group name"
  type        = string
}

variable "cpu" {
  description = "Fargate task CPU units"
  type        = number
  default     = 2048
}

variable "memory" {
  description = "Fargate task memory (MB)"
  type        = number
  default     = 4096
}

variable "image" {
  description = "Container image configuration"
  type = object({
    repository_url = string
    image_tag      = string
  })
}

variable "environment" {
  description = "Environment variables to pass to the container"
  type        = map(string)
  default     = {}
}

variable "tags" {
  description = "Tags"
  type        = map(string)
  nullable    = true
  default     = {}
}
