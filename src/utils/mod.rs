

mod bounded_map;
/// Index types that may be used as keys for the bounded map.
pub trait Index: Copy + Clone {
    /// Creates a new key from the given index.
    fn from_index(index: usize) -> Self;
    /// Returns the index from the given key.
    fn into_index(self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Index for usize {
        fn from_index(index: usize) -> Self {
            index
        }
        fn into_index(self) -> usize {
            self
        }
    }
}
