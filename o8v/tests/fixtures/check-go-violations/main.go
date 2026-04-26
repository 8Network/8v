package main

import "fmt"

func main() {
	// go vet: too few arguments for format string
	fmt.Printf("%d %d", 1)
}
