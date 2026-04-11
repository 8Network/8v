package main

import "fmt"

func main() {
	fmt.Printf("%d", "not a number")
	return
	fmt.Println("unreachable")
}
