locals {
  replication_name = "${var.name}-replication"

  replica_buckets = {
    replica      = aws_s3_bucket.replica.id
    logs_replica = aws_s3_bucket.logs_replica.id
  }
}
