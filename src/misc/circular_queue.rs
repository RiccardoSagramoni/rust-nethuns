//! Module which provides [`CircularQueue`], an optimized circular queue with head and tail indexes.

use std::iter::Cycle;
use std::num::Wrapping;
use std::slice::Iter;

use derivative::Derivative;


/// An optimized circular queue with head and tail indexes.
///
/// In order to avoid an integer division for each push, we allocate a array of actual
/// size equals to the closest power of 2 larger or equal than the requested `max_items` size.
/// In this way, we can increment the head and tail indexes relying on wrapping overflow
/// and filter them with a bit mask when accessing any array item.
///
/// The queue is empty when `head == tail` and is full when `(tail - head) >= max_items`.
#[derive(Default, Derivative)]
#[derivative(Debug)]
pub struct CircularQueue<T> {
    #[derivative(Debug = "ignore")]
    buffer: Box<[T]>,
    head: Wrapping<usize>,
    tail: Wrapping<usize>,
    mask: usize,
    num_items: usize,
}


impl<T> CircularQueue<T> {
    /// Generate a new circular buffer.
    ///
    /// # Parameters
    /// * `size` - the required number of items (the actual allocated size could be larger).
    /// * `generator` - a function which generates a new item.
    ///
    /// # Panics
    /// If size is equals to 0.
    #[inline(always)]
    pub fn new(size: usize, generator: &dyn Fn() -> T) -> Self {
        assert!(size > 0);
        
        let num_items = size;
        let size = size.next_power_of_two();
        
        let mut buffer = Vec::with_capacity(size);
        for _ in 0..size {
            buffer.push(generator());
        }
        
        CircularQueue {
            buffer: buffer.into_boxed_slice(),
            head: Wrapping(0),
            tail: Wrapping(0),
            mask: size - 1,
            num_items,
        }
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
        self.head += 1;
    }
    
    /// Advance the tail index of one position
    #[inline(always)]
    pub fn advance_tail(&mut self) {
        self.tail += 1;
    }
    
    /// Returns a cycle iterator over the buffer
    #[inline(always)]
    pub fn iter(&self) -> Cycle<Iter<T>> {
        self.buffer.iter().cycle()
    }
    
    
    /// Return an immutable reference to the item specified by the `head` index
    /// and advance the `head` index of one position.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn pop(&mut self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            Some(self.pop_unchecked())
        }
    }
    
    /// Return an immutable reference to the item specified by the `head` index
    /// and advance the `head` index of one position.
    ///
    /// **It doesn't check if the buffer is empty.**
    #[inline(always)]
    pub fn pop_unchecked(&mut self) -> &T {
        let head_idx = self.head.0;
        self.advance_head();
        &self.buffer[head_idx & self.mask]
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
        self.buffer[self.tail.0 & self.mask] = value;
        self.advance_tail();
    }
    
    
    /// Get an immutable reference to an element of the buffer
    #[inline(always)]
    pub fn get(&self, index: usize) -> &T {
        &self.buffer[index & self.mask]
    }
    
    /// Get a mutable reference to an element of the buffer
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.buffer[index & self.mask]
    }
}


impl<T: Clone> CircularQueue<T> {
    /// Return a cloned instance of the item specified by the `head` index
    /// and advance the `head` index of one position.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn clone_pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            Some(self.clone_pop_unchecked())
        }
    }
    
    /// Return a cloned instance of the item specified by the `head` index
    /// and advance the `head` index of one position.
    ///
    /// **It doesn't check if the buffer is empty.**
    #[inline(always)]
    pub fn clone_pop_unchecked(&mut self) -> T {
        let ret = self.clone_get(self.head());
        self.advance_head();
        ret
    }
    
    /// Get a cloned copy of an element of the buffer
    #[inline(always)]
    pub fn clone_get(&self, index: usize) -> T {
        self.buffer[index & self.mask].clone()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_small_size() {
        // Create new buffer
        let num_items = 1;
        let mut b = CircularQueue::new(num_items, &|| 0_u8);
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 0);
        assert_eq!(b.tail(), 0);
        assert_eq!(b.size(), num_items.next_power_of_two());
        
        // Push item
        let value = 12;
        assert_eq!(b.push(value), true);
        assert!(!b.is_empty());
        assert!(b.is_full());
        assert_eq!(b.tail(), 1);
        
        // Pop item
        assert_eq!(b.clone_pop(), Some(value));
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 1);
    }
    
    
    #[test]
    fn test_medium_size() {
        // Create new buffer
        let num_items = 10;
        let mut b = CircularQueue::new(num_items, &|| 0);
        assert!(b.is_empty());
        assert!(!b.is_full());
        assert_eq!(b.head(), 0);
        assert_eq!(b.tail(), 0);
        assert!(b.size() >= num_items);
        assert_eq!(b.size(), num_items.next_power_of_two());
        
        // Push item
        let value = 12;
        assert_eq!(b.push(value), true);
        assert!(!b.is_empty());
        assert_eq!(b.tail(), 1);
        
        // Pop item
        assert_eq!(b.clone_pop(), Some(value));
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
}
