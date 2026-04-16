variable "name" {
  default = "app"
}

resource "null_resource" "example" {
  triggers = {
    name = "${var.name}"
  }
}
