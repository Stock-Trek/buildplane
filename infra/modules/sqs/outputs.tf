output "arn" {
  value = aws_sqs_queue.queue.arn
}

output "url" {
  value = aws_sqs_queue.queue.url
}

output "name" {
  value = aws_sqs_queue.queue.name
}

output "dead_letter_arn" {
  value = try(aws_sqs_queue.dead_letter[0].arn, null)
}

output "dead_letter_url" {
  value = try(aws_sqs_queue.dead_letter[0].url, null)
}
