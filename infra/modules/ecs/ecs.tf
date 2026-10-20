resource "aws_ecs_cluster" "cluster" {
  name = var.name

  setting {
    name  = "containerInsights"
    value = "enabled"
  }

  tags = var.tags
}

resource "aws_ecs_task_definition" "task" {
  family                   = var.name
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = tostring(var.cpu)
  memory                   = tostring(var.memory)
  execution_role_arn       = aws_iam_role.execution.arn
  task_role_arn            = aws_iam_role.task.arn

  runtime_platform {
    operating_system_family = "LINUX"
    cpu_architecture        = "ARM64"
  }

  container_definitions = jsonencode([
    {
      name                   = var.name
      image                  = "${var.image.repository_url}:${var.image.image_tag}"
      essential              = true
      user                   = "1000:1000"
      readonlyRootFilesystem = true
      environment = [
        for name, value in var.environment : {
          name  = name
          value = value
        }
      ]
      linuxParameters = {
        initProcessEnabled = true
        tmpfs = [
          {
            containerPath = "/tmp"
            size          = 512
            mountOptions  = ["rw", "noexec", "nosuid"]
          }
        ]
      }
      # logConfiguration = {
      #   logDriver = "awslogs"
      #   options = {
      #     "awslogs-group"         = aws_cloudwatch_log_group.logs.name
      #     "awslogs-region"        = data.aws_region.current.region
      #     "awslogs-stream-prefix" = var.name
      #   }
      # }
    }
  ])

  tags = var.tags
}
