# plan
resource "aws_iam_role" "bootstrap_plan" {
  name               = local.iam_role_plan
  description        = "Read-only access via GitHub for Terraform plan"
  assume_role_policy = data.aws_iam_policy_document.bootstrap_assume_role_plan.json
}

resource "aws_iam_role_policy_attachment" "bootstrap_plan" {
  role       = aws_iam_role.bootstrap_plan.name
  policy_arn = "arn:aws:iam::aws:policy/ReadOnlyAccess"
}

resource "aws_iam_policy" "bootstrap_plan_lock" {
  name   = "${aws_iam_role.bootstrap_plan.name}-lock"
  policy = data.aws_iam_policy_document.bootstrap_plan_lock.json
}

resource "aws_iam_role_policy_attachment" "bootstrap_plan_lock" {
  role       = aws_iam_role.bootstrap_plan.name
  policy_arn = aws_iam_policy.bootstrap_plan_lock.arn
}


# apply
resource "aws_iam_role" "bootstrap_apply" {
  name               = local.iam_role_apply
  description        = "Apply access via GitHub for Terraform apply"
  assume_role_policy = data.aws_iam_policy_document.bootstrap_assume_role_apply.json
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_acm" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AWSCertificateManagerFullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_api_gateway" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonAPIGatewayAdministrator"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_cloudwatch" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchFullAccessV2"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_cloudfront" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/CloudFrontFullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_dynamodb" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonDynamoDBFullAccess_v2"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_ecr" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryFullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_ecs" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonECS_FullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_iam" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/IAMFullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_lambda" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AWSLambda_FullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_route53" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonRoute53FullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_s3" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonS3FullAccess"
}

resource "aws_iam_role_policy_attachment" "bootstrap_apply_sqs" {
  role       = aws_iam_role.bootstrap_apply.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSQSFullAccess"
}

