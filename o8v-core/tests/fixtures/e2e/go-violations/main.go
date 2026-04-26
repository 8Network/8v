package main

import "fmt"

func main() {
	// go vet: printf format mismatch (%s with int)
	x := 42
	fmt.Printf("%s", x)

	// Use the helper from utils.go
	result := add(3, 4)
	fmt.Println(result)
}
