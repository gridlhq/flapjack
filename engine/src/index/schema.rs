use crate::error::{FlapjackError, Result};
use std::collections::HashMap;
use tantivy::schema::{Field, Schema as TantivySchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Integer,
    Float,
    Date,
    Facet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextIndexing {
    Default,
    EdgeNgram,
}

#[derive(Debug, Clone)]
pub struct FieldOptions {
    pub stored: bool,
    pub indexed: bool,
    pub fast: bool,
    pub text_indexing: TextIndexing,
}

impl Default for FieldOptions {
    fn default() -> Self {
        FieldOptions {
            stored: true,
            indexed: true,
            fast: false,
            text_indexing: TextIndexing::EdgeNgram,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: String,
    pub field_type: FieldType,
    pub options: FieldOptions,
}

#[derive(Debug, Clone)]
pub struct Schema {
    fields: Vec<FieldDefinition>,
    field_map: HashMap<String, usize>,
}

impl Schema {
    pub fn builder() -> SchemaBuilder {
        SchemaBuilder::new()
    }

    pub fn get_field(&self, name: &str) -> Option<&FieldDefinition> {
        self.field_map.get(name).map(|&idx| &self.fields[idx])
    }

    pub fn fields(&self) -> &[FieldDefinition] {
        &self.fields
    }

    pub fn to_tantivy(&self) -> TantivySchema {
        let mut builder = TantivySchema::builder();

        builder.add_text_field(
            "_id",
            tantivy::schema::STRING | tantivy::schema::STORED | tantivy::schema::FAST,
        );

        let text_indexing = tantivy::schema::TextFieldIndexing::default()
            .set_tokenizer("edge_ngram_lower")
            .set_search_tokenizer("simple")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);

        let json_search_opts = tantivy::schema::JsonObjectOptions::default()
            .set_stored()
            .set_indexing_options(text_indexing);

        builder.add_json_field("_json_search", json_search_opts);

        let json_filter_indexing = tantivy::schema::TextFieldIndexing::default()
            .set_tokenizer("raw")
            .set_index_option(tantivy::schema::IndexRecordOption::Basic);

        let json_filter_opts = tantivy::schema::JsonObjectOptions::default()
            .set_stored()
            .set_indexing_options(json_filter_indexing)
            .set_fast(None)
            .set_fast(None);

        builder.add_json_field("_json_filter", json_filter_opts);

        let json_exact_indexing = tantivy::schema::TextFieldIndexing::default()
            .set_tokenizer("simple")
            .set_search_tokenizer("simple")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);

        let json_exact_opts =
            tantivy::schema::JsonObjectOptions::default().set_indexing_options(json_exact_indexing);

        builder.add_json_field("_json_exact", json_exact_opts);

        let facet_opts = tantivy::schema::FacetOptions::default();
        builder.add_facet_field("_facets", facet_opts);

        let f64_opts = tantivy::schema::NumericOptions::default()
            .set_fast()
            .set_stored();
        builder.add_f64_field("_geo_lat", f64_opts.clone());
        builder.add_f64_field("_geo_lng", f64_opts);

        builder.build()
    }

    pub fn from_tantivy(_tantivy_schema: TantivySchema) -> Result<Self> {
        Ok(Schema {
            fields: vec![],
            field_map: HashMap::new(),
        })
    }

    pub fn get_tantivy_field(&self, tantivy_schema: &TantivySchema, name: &str) -> Result<Field> {
        match tantivy_schema.get_field(name) {
            Ok(field) => Ok(field),
            Err(_) => Err(FlapjackError::FieldNotFound(name.to_string())),
        }
    }
}

pub struct SchemaBuilder {
    fields: Vec<FieldDefinition>,
}

impl Default for SchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaBuilder {
    pub fn new() -> Self {
        SchemaBuilder { fields: Vec::new() }
    }

    pub fn add_field(
        mut self,
        name: impl Into<String>,
        field_type: FieldType,
        options: FieldOptions,
    ) -> Self {
        self.fields.push(FieldDefinition {
            name: name.into(),
            field_type,
            options,
        });
        self
    }

    // DEPRECATED: Phase 1 migration to schemaless JSON fields
    // These methods are no longer used - schema is now hardcoded to dual JSON fields
    // Keeping temporarily to avoid breaking tests during migration

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_text_field(self, name: impl Into<String>) -> Self {
        let name_str = name.into();
        let ngram_name = format!("{}_ngram", name_str);

        self.add_field(
            name_str.clone(),
            FieldType::Text,
            FieldOptions {
                stored: true,
                indexed: true,
                fast: false,
                text_indexing: TextIndexing::Default,
            },
        )
        .add_field(
            ngram_name,
            FieldType::Text,
            FieldOptions {
                stored: false,
                indexed: true,
                fast: false,
                text_indexing: TextIndexing::EdgeNgram,
            },
        )
    }

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_filterable_text_field(self, name: impl Into<String>) -> Self {
        self.add_field(
            name,
            FieldType::Text,
            FieldOptions {
                stored: true,
                indexed: true,
                fast: false,
                text_indexing: TextIndexing::Default,
            },
        )
    }

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_prefix_text_field(self, name: impl Into<String>) -> Self {
        self.add_field(
            name,
            FieldType::Text,
            FieldOptions {
                stored: true,
                indexed: true,
                fast: false,
                text_indexing: TextIndexing::EdgeNgram,
            },
        )
    }

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_integer_field(self, name: impl Into<String>) -> Self {
        self.add_field(name, FieldType::Integer, FieldOptions::default())
    }

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_fast_field(self, name: impl Into<String>, field_type: FieldType) -> Self {
        self.add_field(
            name,
            field_type,
            FieldOptions {
                stored: true,
                indexed: true,
                fast: true,
                text_indexing: TextIndexing::EdgeNgram,
            },
        )
    }

    #[deprecated(note = "Phase 1: Use schemaless JSON fields instead")]
    pub fn add_facet_field(self, name: impl Into<String>) -> Self {
        self.add_field(
            name,
            FieldType::Facet,
            FieldOptions {
                stored: true,
                indexed: true,
                fast: false,
                text_indexing: TextIndexing::Default,
            },
        )
    }

    pub fn build(mut self) -> Schema {
        self.fields.insert(
            0,
            FieldDefinition {
                name: "_id".to_string(),
                field_type: FieldType::Text,
                options: FieldOptions {
                    stored: true,
                    indexed: false,
                    fast: false,
                    text_indexing: TextIndexing::Default,
                },
            },
        );

        let mut field_map = HashMap::new();
        for (idx, field) in self.fields.iter().enumerate() {
            field_map.insert(field.name.clone(), idx);
        }
        Schema {
            fields: self.fields,
            field_map,
        }
    }
}
