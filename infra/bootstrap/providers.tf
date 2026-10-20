provider "aws" {
  region = local.aws_region
}

provider "aws" {
  alias  = "replica"
  region = "us-east-1"
}

terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~>6.40.0"
    }
  }
  backend "s3" {
    bucket       = "state.stock-trek.com"
    key          = "bootstrap/terraform.tfstate"
    region       = "eu-west-1"
    use_lockfile = true
    encrypt      = true
  }
}
