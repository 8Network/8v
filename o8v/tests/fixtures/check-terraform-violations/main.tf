terraform {
  required_version = ">= 1.0"
}
variable "region" {
  type    = string
  default = "us-east-1"
}
variable "env" {
  type    = string
  default = "prod"
}
