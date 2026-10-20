# resource "aws_signer_signing_profile" "lambda" {
#   name        = replace(var.name, "-", "_")
#   platform_id = "AWSLambda-SHA384-ECDSA"
#   tags        = var.tags
# }

# resource "aws_lambda_code_signing_config" "lambda" {
#   allowed_publishers {
#     signing_profile_version_arns = [aws_signer_signing_profile.lambda.version_arn]
#   }

#   policies {
#     untrusted_artifact_on_deployment = "Warn" # TODO Use "Enforce" when code signing is in effect
#   }

#   tags = var.tags
# }
