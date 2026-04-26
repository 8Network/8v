terraform {
  required_version = ">= 1.0"
}

# Deprecated interpolation — should use var.name instead of "${var.name}"
variable "name" {
  type    = string
  default = "test"
}

variable "unused_var" {
  type        = string
  description = "This variable is declared but never used"
  default     = "unused"
}

# Naming convention violation — camelCase instead of snake_case
variable "myAppName" {
  type    = string
  default = "app"
}

resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"

  # Deprecated interpolation syntax — using "${var.name}" instead of var.name
  tags = {
    Name = "${var.name}-instance"
  }
}
