package main

import "fmt"

// add returns the sum of a and b.
func add(a, b int) int {
	return a + b
}

// printStatus prints a status message. Has unreachable code.
func printStatus(ok bool) {
	if ok {
		fmt.Println("ok")
		return
	} else {
		// go vet: SA4017 / unreachable: the else branch after a return is never reached
		// but this form also works for staticcheck ST1003 (stuttering)
		fmt.Println("not ok")
		return
	}
	fmt.Println("this line is unreachable") // go vet: unreachable
}

// Person has a malformed struct tag (missing colon in json tag).
type Person struct {
	Name string `json:"name"`
	Age  int    `json "age"` // go vet: struct tag — missing colon
}

// double uses an unnecessary conversion (staticcheck S1000-style).
// The len(s) > 0 check can be replaced by s != "" (staticcheck S1003).
func isEmpty(s string) bool {
	if len(s) == 0 {
		return true
	}
	return false // staticcheck S1008: could simplify to single return
}
