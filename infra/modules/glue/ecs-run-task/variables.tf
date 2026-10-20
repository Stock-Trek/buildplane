variable "role" {
  description = "Role to attach ECS run task permission to"
  type        = string
}

variable "task_definition_arn" {
  description = "Task definition arn to allow running"
  type        = string
}

variable "cluster_arn" {
  description = "Cluster arn containing the tasks to allow describing"
  type        = string
}

variable "execution_role_arn" {
  description = "Task execution role arn to pass to ECS"
  type        = string
}

variable "task_role_arn" {
  description = "Task role arn to pass to ECS"
  type        = string
}
