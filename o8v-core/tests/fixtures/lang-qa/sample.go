package main

import "fmt"

type MyStruct struct {
    Name string
}

func (m MyStruct) Method() string {
    return m.Name
}

func GenericFunc[T any](x T) T {
    return x
}

func main() {
    fmt.Println("hello")
}
