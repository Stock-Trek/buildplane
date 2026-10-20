terraform {
  required_version = ">= 1.10"

  required_providers {
    archive = {
      source  = "hashicorp/archive"
      version = "~> 2.7.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.40.0"
    }
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 4.2.0"
    }
    null = {
      source  = "hashicorp/null"
      version = "~> 3.2.0"
    }
  }
}
