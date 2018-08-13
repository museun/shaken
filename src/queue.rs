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
        let mut out = None;
        if self.queue.len() == self.queue.capacity() {
            out = self.queue.pop_front();
        }
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
}
