resource "null_resource" "dummy" {
  count = local.zip_count
}

data "archive_file" "dummy" {
  count = local.zip_count

  type        = "zip"
  output_path = "${path.module}/files/dummy.zip"
  source {
    content  = "#!/bin/bash\nexit 0"
    filename = "bootstrap"
  }
  depends_on = [null_resource.dummy[0]]
}
