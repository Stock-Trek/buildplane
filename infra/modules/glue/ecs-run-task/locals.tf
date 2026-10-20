locals {
  task_definition_family_arn = replace(var.task_definition_arn, "/:[0-9]+$/", "")
  cluster_task_arn_prefix    = replace(var.cluster_arn, ":cluster/", ":task/")
}
