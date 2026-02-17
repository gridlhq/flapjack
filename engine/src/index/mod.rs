pub mod document;
pub mod facet_translation;
pub mod manager;
pub mod memory;
pub mod memory_observer;
pub mod oplog;
pub mod relevance;
pub mod rules;
#[cfg(feature = "s3-snapshots")]
pub mod s3;
pub mod schema;
pub mod settings;
#[cfg(feature = "s3-snapshots")]
pub mod snapshot;
pub mod synonyms;
pub mod task_queue;
mod utils;
pub mod write_queue;
pub mod writer;

use crate::error::Result;
use crate::types::Document;
use document::DocumentConverter;
use memory::{MemoryBudget, MemoryBudgetConfig};
use schema::Schema;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tantivy::Index as TantivyIndex;
pub use writer::ManagedIndexWriter;

static GLOBAL_BUDGET: OnceLock<Arc<MemoryBudget>> = OnceLock::new();

pub fn get_global_budget() -> Arc<MemoryBudget> {
    Arc::clone(
        GLOBAL_BUDGET.get_or_init(|| Arc::new(MemoryBudget::new(MemoryBudgetConfig::from_env()))),
    )
}

pub fn reset_global_budget_for_test() {
    if let Some(budget) = GLOBAL_BUDGET.get() {
        budget.reset_for_test();
    }
}

/// A single search index backed by Tantivy.
///
/// `Index` wraps a Tantivy index with a dynamic JSON schema, CJK-aware
/// tokenization, and edge-ngram prefix search. Documents can be added via
/// the simple JSON API ([`Index::add_documents_simple`]) or with an explicit
/// writer ([`Index::writer`]).
///
/// # Examples
///
/// ```rust,no_run
/// use flapjack::index::Index;
/// use serde_json::json;
///
/// # fn main() -> flapjack::Result<()> {
/// let index = Index::create_in_dir("./my_index")?;
/// index.add_documents_simple(&[
///     json!({"objectID": "1", "title": "Hello world"}),
/// ])?;
/// # Ok(())
/// # }
/// ```
pub struct Index {
    inner: TantivyIndex,
    reader: tantivy::IndexReader,
    schema: Schema,
    converter: Arc<DocumentConverter>,
    budget: Arc<MemoryBudget>,
    searchable_paths_cache: std::sync::RwLock<Option<Vec<String>>>,
}

impl Index {
    pub const DEFAULT_BUFFER_SIZE: usize = 20_000_000;
}

impl Index {
    /// Create a new index at `path` with the default schema.
    ///
    /// Creates the directory (and parents) if it does not exist.
    pub fn create_in_dir<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(path.as_ref())?;
        let schema = Schema::builder().build();
        Self::create(path, schema)
    }

    /// Create a new index at `path` with an explicit schema.
    pub fn create<P: AsRef<Path>>(path: P, schema: Schema) -> Result<Self> {
        Self::create_with_budget(path, schema, get_global_budget())
    }

    /// Create a new index with an explicit schema and memory budget.
    pub fn create_with_budget<P: AsRef<Path>>(
        path: P,
        schema: Schema,
        budget: Arc<MemoryBudget>,
    ) -> Result<Self> {
        let tantivy_schema = schema.to_tantivy();
        let inner = TantivyIndex::create_in_dir(path, tantivy_schema.clone())?;

        let edge_ngram_tokenizer =
            tantivy::tokenizer::TextAnalyzer::builder(crate::tokenizer::CjkAwareTokenizer)
                .filter(tantivy::tokenizer::LowerCaser)
                .filter(tantivy::tokenizer::EdgeNgramFilter::new(2, 20).unwrap())
                .build();

        inner
            .tokenizers()
            .register("edge_ngram_lower", edge_ngram_tokenizer);

        let simple_tokenizer =
            tantivy::tokenizer::TextAnalyzer::builder(crate::tokenizer::CjkAwareTokenizer)
                .filter(tantivy::tokenizer::LowerCaser)
                .build();

        inner.tokenizers().register("simple", simple_tokenizer);

        let reader = inner
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let converter = Arc::new(DocumentConverter::new(&schema, &tantivy_schema)?);
        Ok(Index {
            inner,
            reader,
            schema,
            converter,
            budget,
            searchable_paths_cache: std::sync::RwLock::new(None),
        })
    }

    /// Open an existing index at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_budget(path, get_global_budget())
    }

    /// Open an existing index with an explicit memory budget.
    pub fn open_with_budget<P: AsRef<Path>>(path: P, budget: Arc<MemoryBudget>) -> Result<Self> {
        let inner = TantivyIndex::open_in_dir(path)?;

        let edge_ngram_tokenizer =
            tantivy::tokenizer::TextAnalyzer::builder(crate::tokenizer::CjkAwareTokenizer)
                .filter(tantivy::tokenizer::LowerCaser)
                .filter(tantivy::tokenizer::EdgeNgramFilter::new(2, 20).unwrap())
                .build();

        inner
            .tokenizers()
            .register("edge_ngram_lower", edge_ngram_tokenizer);

        let simple_tokenizer =
            tantivy::tokenizer::TextAnalyzer::builder(crate::tokenizer::CjkAwareTokenizer)
                .filter(tantivy::tokenizer::LowerCaser)
                .build();

        inner.tokenizers().register("simple", simple_tokenizer);

        let reader = inner
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let tantivy_schema = inner.schema();
        let schema = Schema::from_tantivy(tantivy_schema.clone())?;
        let converter = Arc::new(DocumentConverter::new(&schema, &tantivy_schema)?);
        Ok(Index {
            inner,
            reader,
            schema,
            converter,
            budget,
            searchable_paths_cache: std::sync::RwLock::new(None),
        })
    }

    /// Create an index writer with the default buffer size (20 MB).
    ///
    /// The writer holds a slot in the global memory budget. Drop it (or call
    /// `commit()`) when finished to release the slot.
    pub fn writer(&self) -> Result<ManagedIndexWriter> {
        self.writer_with_size(Self::DEFAULT_BUFFER_SIZE)
    }

    /// Create an index writer with a custom buffer size (in bytes).
    pub fn writer_with_size(&self, buffer_size: usize) -> Result<ManagedIndexWriter> {
        let validated_size = self.budget.validate_buffer_size(buffer_size)?;
        let guard = self.budget.acquire_writer()?;
        let writer = self.inner.writer(validated_size)?;
        Ok(ManagedIndexWriter::new(writer, guard))
    }

    /// Get a reference to the index reader (for searching).
    pub fn reader(&self) -> &tantivy::IndexReader {
        &self.reader
    }

    /// Get the index schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Access the underlying Tantivy index.
    pub fn inner(&self) -> &TantivyIndex {
        &self.inner
    }

    /// Get the document converter for this index.
    pub fn converter(&self) -> Arc<DocumentConverter> {
        Arc::clone(&self.converter)
    }

    /// Add a single [`Document`] using an explicit writer.
    ///
    /// You must call `writer.commit()` afterwards to persist, then
    /// `index.reader().reload()` to make the documents searchable.
    pub fn add_document(&self, writer: &mut ManagedIndexWriter, doc: Document) -> Result<()> {
        let tantivy_doc = self.converter.to_tantivy(&doc, None)?;
        writer.add_document(tantivy_doc)?;
        Ok(())
    }

    /// Add multiple [`Document`]s using an explicit writer.
    ///
    /// Convenience wrapper — calls [`Index::add_document`] for each doc.
    /// Caller must commit and reload afterwards.
    pub fn add_documents(
        &self,
        writer: &mut ManagedIndexWriter,
        docs: Vec<Document>,
    ) -> Result<()> {
        for doc in docs {
            self.add_document(writer, doc)?;
        }
        Ok(())
    }

    /// Get the memory budget associated with this index.
    pub fn memory_budget(&self) -> &Arc<MemoryBudget> {
        &self.budget
    }

    /// Add JSON documents, commit, and refresh the reader in one call.
    ///
    /// This is the easiest way to index documents. Each JSON object must
    /// contain either `"objectID"` (Algolia convention) or `"_id"` as the
    /// document identifier. All other fields are indexed automatically.
    ///
    /// Documents are searchable immediately after this method returns.
    ///
    /// # Errors
    ///
    /// Returns [`crate::FlapjackError::MissingField`] if a document lacks an ID,
    /// or [`crate::FlapjackError::InvalidDocument`] if a value is not a JSON object.
    pub fn add_documents_simple(&self, docs: &[serde_json::Value]) -> Result<()> {
        use crate::index::document::json_to_tantivy_doc;
        let mut writer = self.writer()?;

        let schema = self.inner.schema();
        let id_field = schema.get_field("_id").unwrap();
        let json_search_field = schema.get_field("_json_search").unwrap();
        let json_filter_field = schema.get_field("_json_filter").unwrap();
        let json_exact_field = schema.get_field("_json_exact").unwrap();
        let facets_field = schema.get_field("_facets").unwrap();

        for json_doc in docs {
            let tantivy_doc = json_to_tantivy_doc(
                json_doc,
                id_field,
                json_search_field,
                json_filter_field,
                json_exact_field,
                facets_field,
            )?;
            writer.add_document(tantivy_doc)?;
        }

        writer.commit()?;
        self.reader.reload()?;
        self.invalidate_searchable_paths_cache();
        Ok(())
    }

    /// Return the list of field paths that contain indexed text.
    ///
    /// Results are cached; call [`Index::invalidate_searchable_paths_cache`]
    /// after adding documents to refresh.
    pub fn searchable_paths(&self) -> Vec<String> {
        {
            let cache = self.searchable_paths_cache.read().unwrap();
            if let Some(paths) = cache.as_ref() {
                return paths.clone();
            }
        }

        let searcher = self.reader.searcher();
        let schema = self.inner.schema();
        let json_search_field = match schema.get_field("_json_search") {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        let mut paths = std::collections::HashSet::new();
        for segment in searcher.segment_readers() {
            if let Ok(inv_index) = segment.inverted_index(json_search_field) {
                if let Ok(mut terms) = inv_index.terms().stream() {
                    while terms.advance() {
                        let term_bytes = terms.key();
                        if let Some(pos) = term_bytes.windows(2).position(|w| w == b"\0s") {
                            let path = String::from_utf8_lossy(&term_bytes[..pos]).to_string();
                            paths.insert(path);
                        }
                    }
                }
            }
        }

        let result: Vec<String> = paths.into_iter().collect();
        {
            let mut cache = self.searchable_paths_cache.write().unwrap();
            *cache = Some(result.clone());
        }
        result
    }

    /// Clear the cached searchable paths so the next call recomputes them.
    pub fn invalidate_searchable_paths_cache(&self) {
        let mut cache = self.searchable_paths_cache.write().unwrap();
        *cache = None;
    }
}
