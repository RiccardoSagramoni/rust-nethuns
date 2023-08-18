use std::iter::Cycle;
use std::mem;
use std::num::Wrapping;
use std::slice::Iter;


/// An optimized circular buffer for items which implements [`Clone`] trait.
///
/// In order to avoid a division for each push, we allocate a buffer of actual
/// size equals to the closest power of 2 larger or equal than the requested `max_items` size.
/// In this way we can increment the head and tail indexes relying on wrapping overflow
/// and filter them with a bit mask when accessing any buffer item.
///
/// The buffer is empty when `head == tail` and is full when `(tail - head) >= max_items`.
///
/// Whenever an item is read from the buffer, the item is **cloned**, not moved.
/// Thus, this buffer more appropriate for reference-counting pointer
/// ([`std::rc::Rc`], [`std::sync::Arc`]) and primitive types (which implement
/// the [`Copy`] trait).
#[derive(Debug, Default)]
pub struct CircularCloneBuffer<T: Clone> {
    buffer: Vec<T>,
    head: Wrapping<usize>,
    tail: Wrapping<usize>,
    mask: usize,
    num_items: usize,
}


impl<T: Clone> CircularCloneBuffer<T> {
    /// Generate a new circular buffer.
    ///
    /// # Parameters
    /// * `size` - the number of items.
    /// * `builder` - a function which generates a new item.
    ///
    /// # Panics
    /// If size is equals to zero.
    #[inline(always)]
    pub fn new(size: usize, builder: &dyn Fn() -> T) -> Self {
        assert!(size > 0);
        
        let num_items = size;
        let size = nethuns_lpow2(size);
        
        let mut buffer = Vec::with_capacity(size);
        for _ in 0..size {
            buffer.push(builder());
        }
        
        CircularCloneBuffer {
            buffer,
            head: Wrapping(0),
            tail: Wrapping(0),
            mask: size - 1,
            num_items,
        }
    }
    
    /// Return a clone instance of the item specified by the `head` index
    /// and advance the `head` index of one position.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            Some(self.pop_unchecked())
        }
    }
    
    /// Return a clone instance of the item specified by the `head` index
    /// and advance the `head` index of one position.
    /// **It doesn't check if the buffer is empty.**
    #[inline(always)]
    pub fn pop_unchecked(&mut self) -> T {
        let ret = self.buffer[self.head.0].clone();
        self.head += 1;
        ret
    }
    
    /// Add a new item to the buffer at the position specified by the `tail` index
    /// and advance the `tail` index of one position.
    ///
    /// # Returns
    /// `true` if the buffer is not full, `false` otherwise.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn push(&mut self, value: T) -> bool {
        if self.is_full() {
            false
        } else {
            self.push_unchecked(value);
            true
        }
    }
    
    /// Add a new item to the buffer at the position specified by the `tail` index
    /// and advance the `tail` index of one position.
    ///  
    /// **It doesn't check if the buffer is full.**
    #[inline(always)]
    pub fn push_unchecked(&mut self, value: T) {
        self.buffer[self.tail.0] = value;
        self.tail += 1;
    }
    
    /// Get an element of the buffer
    #[inline(always)]
    pub fn get(&self, index: usize) -> T {
        self.buffer[index & self.mask].clone()
    }
    
    /// Get the allocated size of the buffer
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.mask + 1
    }
    
    /// Check if the buffer is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.head.0 == self.tail.0
    }
    
    /// Check if the buffer is full
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        (self.tail.0 - self.head.0) >= self.num_items
    }
    
    /// Get the current head index
    #[inline(always)]
    pub fn head(&self) -> usize {
        self.head.0
    }
    
    /// Get the current tail index
    #[inline(always)]
    pub fn tail(&self) -> usize {
        self.tail.0
    }
    
    /// Advance the head index of one position
    #[inline(always)]
    pub fn advance_head(&mut self) {
        self.head += 1
    }
    
    /// Advance the tail index of one position
    #[inline(always)]
    pub fn advance_tail(&mut self) {
        self.tail += 1
    }
    
    /// Returns a cycle iterator over the buffer
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
    use super::*;
    
    #[test]
    fn test_small_buffer() {
        // Create new buffer
        let num_items = 1;
        let mut b = CircularCloneBuffer::new(num_items, &|| 0_u8);
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 0);
        assert_eq!(b.tail(), 0);
        assert_eq!(b.size(), nethuns_lpow2(num_items));
        
        // Push item
        let value = 12;
        assert_eq!(b.push(value), true);
        assert!(!b.is_empty());
        assert!(b.is_full());
        assert_eq!(b.tail(), 1);
        
        // Pop item
        assert_eq!(b.pop(), Some(value));
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 1);
    }
    
    
    #[test]
    fn test_normal_buffer() {
        // Create new buffer
        let num_items = 10;
        let mut b = CircularCloneBuffer::new(num_items, &|| 0);
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 0);
        assert_eq!(b.tail(), 0);
        assert!(b.size() >= num_items);
        assert_eq!(b.size(), nethuns_lpow2(num_items));
        
        // Push item
        let value = 12;
        assert_eq!(b.push(value), true);
        assert!(!b.is_empty());
        assert_eq!(b.tail(), 1);
        
        // Pop item
        assert_eq!(b.pop(), Some(value));
        assert!(b.is_empty());
        assert_eq!(b.head(), 1);
        
        // Fill buffer
        for i in 0..num_items {
            assert_eq!(b.push(i), true);
        }
        assert!(!b.is_empty());
        assert!(b.is_full());
        
        assert!(!b.push(100)); // buffer is full!
    }
    
    
    #[test]
    fn lpow2() {
        assert_eq!(nethuns_lpow2(0), 1);
        assert_eq!(nethuns_lpow2(1), 1);
        assert_eq!(nethuns_lpow2(2), 2);
        assert_eq!(nethuns_lpow2(5), 8);
        assert_eq!(nethuns_lpow2(12), 16);
        assert_eq!(nethuns_lpow2(16), 16);
        assert_eq!(nethuns_lpow2(30), 32);
    }
}
