data "aws_ecr_authorization_token" "dummy" {}

provider "aws" {
  region = "eu-west-1"

  default_tags {
    tags = local.tags
  }
}

provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"

  default_tags {
    tags = local.tags
  }
}

provider "docker" {
  registry_auth {
    address  = data.aws_ecr_authorization_token.dummy.proxy_endpoint
    username = data.aws_ecr_authorization_token.dummy.user_name
    password = data.aws_ecr_authorization_token.dummy.password
  }
}

terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~>6.40.0"
    }
    docker = {
      source  = "kreuzwerker/docker"
      version = "~> 4.2.0"
    }
  }
  backend "s3" {
    bucket       = "state.stock-trek.com"
    key          = "live/terraform.tfstate"
    region       = "eu-west-1"
    use_lockfile = true
    encrypt      = true
  }
}
