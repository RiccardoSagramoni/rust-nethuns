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


#[cfg(test)]
mod tests {
    #[test]
    fn max_pair() {
        assert_eq!(10, super::max!(5, 10))
    }
    
    #[test]
    fn min_pair() {
        assert_eq!(5, super::min!(5, 10))
    }
}
