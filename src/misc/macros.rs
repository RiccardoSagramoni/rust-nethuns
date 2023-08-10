use std::mem;


/// Compute the maximum between any two values of the same type (`x` and `y`).
macro_rules! max {
    ($x: expr, $y: expr) => {{
        if $x > $y {
            $x
        } else {
            $y
        }
    }};
}
pub(crate) use max;


/// Compute the minimum between any two values of the same type (`x` and `y`).
macro_rules! min {
    ($x: expr, $y: expr) => {{
        if $x < $y {
            $x
        } else {
            $y
        }
    }};
}
pub(crate) use min;


/// Compute the closest power of 2 larger or equal than `x`
#[inline(always)]
pub fn nethuns_lpow2(x: usize) -> usize {
    if x != 0 && (x & (x - 1)) == 0 {
        x
    } else {
        1 << (mem::size_of::<usize>() * 8 - x.leading_zeros() as usize)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn test_max() {
        assert_eq!(10, super::max!(5, 10))
    }
    
    #[test]
    fn test_min() {
        assert_eq!(5, super::min!(5, 10))
    }
    
    #[test]
    fn lpow2() {
        assert_eq!(super::nethuns_lpow2(0), 1);
        assert_eq!(super::nethuns_lpow2(1), 1);
        assert_eq!(super::nethuns_lpow2(2), 2);
        assert_eq!(super::nethuns_lpow2(30), 32);
    }
}
