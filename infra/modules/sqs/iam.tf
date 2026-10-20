resource "aws_iam_policy" "send_message" {
  name   = "${var.role}-sqs-send-message"
  policy = data.aws_iam_policy_document.send_message.json
}

resource "aws_iam_role_policy_attachment" "send_message" {
  role       = var.role
  policy_arn = aws_iam_policy.send_message.arn
}
