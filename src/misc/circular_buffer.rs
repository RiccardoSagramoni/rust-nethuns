use std::iter::Cycle;
use std::mem;
use std::num::Wrapping;
use std::slice::Iter;

#[derive(Debug, Default)]
pub struct CircularBuffer<T: Clone> {
    buffer: Vec<T>,
    head: Wrapping<usize>,
    tail: Wrapping<usize>,
    mask: usize,
}


impl<T: Clone> CircularBuffer<T> {
    #[inline(always)]
    pub fn new(size: usize, builder: &dyn Fn() -> T) -> Self {
        let size = nethuns_lpow2(size);
        
        let mut buffer = Vec::with_capacity(size);
        for _ in 0..size {
            buffer.push(builder());
        }
        
        CircularBuffer {
            buffer,
            head: Wrapping(0),
            tail: Wrapping(0),
            mask: size - 1,
        }
    }
    
    #[inline(always)]
    pub fn pop(&mut self) -> T {
        let ret = self.buffer[self.head.0].clone();
        self.head += 1;
        ret
    }
    
    #[inline(always)]
    pub fn push(&mut self, value: T) {
        self.buffer[self.tail.0] = value;
        self.tail += 1;
    }
    
    #[inline(always)]
    pub fn get(&self, index: usize) -> T {
        self.buffer[index & self.mask].clone()
    }
    
    #[inline(always)]
    pub fn position(&self, elem: &T) -> usize {
        unsafe { (elem as *const T).offset_from(self.buffer.as_ptr()) as _ }
    }
    
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.mask + 1
    }
    
    #[inline(always)]
    pub fn empty(&self) -> bool {
        self.head.0 == self.tail.0
    }
    
    #[inline(always)]
    pub fn head(&self) -> usize {
        self.head.0
    }
    
    #[inline(always)]
    pub fn tail(&self) -> usize {
        self.tail.0
    }
    
    #[inline(always)]
    pub fn advance_head(&mut self) {
        self.head += 1
    }
    
    #[inline(always)]
    pub fn advance_tail(&mut self) {
        self.tail += 1
    }
    
    #[inline(always)]
    pub fn iter(&self) -> Cycle<Iter<T>> {
        self.buffer.iter().cycle()
    }
}


/// Compute the closest power of 2 larger or equal than `x`
#[inline(always)]
fn nethuns_lpow2(x: usize) -> usize {
    if x != 0 && (x & (x - 1)) == 0 {
        x
    } else {
        1 << (mem::size_of::<usize>() * 8 - x.leading_zeros() as usize)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn lpow2() {
        assert_eq!(super::nethuns_lpow2(0), 1);
        assert_eq!(super::nethuns_lpow2(1), 1);
        assert_eq!(super::nethuns_lpow2(2), 2);
        assert_eq!(super::nethuns_lpow2(5), 8);
        assert_eq!(super::nethuns_lpow2(12), 16);
        assert_eq!(super::nethuns_lpow2(16), 16);
        assert_eq!(super::nethuns_lpow2(30), 32);
    }
}
