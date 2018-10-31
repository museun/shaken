use std::collections::VecDeque;

pub struct Queue<T> {
    queue: VecDeque<T>,
}

impl<T> Queue<T> {
    pub fn new(size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(size),
        }
    }

    pub fn push(&mut self, element: T) -> Option<T> {
        let out = if self.queue.len() == self.queue.capacity() {
            self.queue.pop_front()
        } else {
            None
        };

        self.queue.push_back(element);
        out
    }

    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.len() == 0
    }

    pub fn queue(&self) -> &VecDeque<T> {
        &self.queue
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.queue.iter()
    }
}
