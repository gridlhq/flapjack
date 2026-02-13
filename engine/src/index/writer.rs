use crate::index::memory::WriterGuard;
use std::ops::{Deref, DerefMut};

pub struct ManagedIndexWriter {
    inner: tantivy::IndexWriter,
    _guard: WriterGuard,
}

impl ManagedIndexWriter {
    pub(crate) fn new(inner: tantivy::IndexWriter, guard: WriterGuard) -> Self {
        ManagedIndexWriter {
            inner,
            _guard: guard,
        }
    }
}

impl Deref for ManagedIndexWriter {
    type Target = tantivy::IndexWriter;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ManagedIndexWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
