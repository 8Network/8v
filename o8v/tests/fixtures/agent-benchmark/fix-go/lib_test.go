package fixgo

import "testing"

func TestSumRangeInclusive(t *testing.T) {
	cases := []struct {
		name     string
		lo, hi   int
		expected int
	}{
		{"single", 5, 5, 5},
		{"one_to_three", 1, 3, 6},
		{"one_to_five", 1, 5, 15},
		{"neg_to_pos", -2, 2, 0},
		{"empty_range", 5, 4, 0},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := SumRangeInclusive(tc.lo, tc.hi)
			if got != tc.expected {
				t.Errorf("SumRangeInclusive(%d, %d) = %d, want %d",
					tc.lo, tc.hi, got, tc.expected)
			}
		})
	}
}

func TestMaxInSlice(t *testing.T) {
	cases := []struct {
		name     string
		xs       []int
		expected int
	}{
		{"single", []int{7}, 7},
		{"ascending", []int{1, 2, 3}, 3},
		{"descending", []int{9, 4, 1}, 9},
		{"mixed", []int{3, 7, 2, 5}, 7},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := MaxInSlice(tc.xs)
			if got != tc.expected {
				t.Errorf("MaxInSlice(%v) = %d, want %d", tc.xs, got, tc.expected)
			}
		})
	}
}
