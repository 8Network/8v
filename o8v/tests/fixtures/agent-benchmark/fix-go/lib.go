package fixgo

// SumRangeInclusive returns the sum of integers from lo to hi, inclusive on
// both ends. For lo > hi it returns 0.
func SumRangeInclusive(lo, hi int) int {
	if lo > hi {
		return 0
	}
	total := 0
	// BUG: off-by-one — the upper bound should be inclusive (i <= hi),
	// but this loop stops one short.
	for i := lo; i < hi; i++ {
		total += i
	}
	return total
}

// MaxInSlice returns the maximum value in xs. Panics on empty slice.
func MaxInSlice(xs []int) int {
	if len(xs) == 0 {
		panic("MaxInSlice: empty slice")
	}
	max := xs[0]
	for i := 1; i < len(xs); i++ {
		if xs[i] > max {
			max = xs[i]
		}
	}
	return max
}
