import Foundation

let str: String = "Hello, World!"

// swiftlint: force_cast violation
let forcedValue = str as! NSString

// swiftlint: line_length violation (long line)
let veryLongLineWithLotsOfText = "This is an extremely long line of code that exceeds the recommended line length limit and should trigger a line_length violation"

// swiftlint: trailing_whitespace
func printMessage() {
    print(str)
}
