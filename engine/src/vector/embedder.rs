use std::sync::OnceLock;

use super::config::{EmbedderConfig, EmbedderSource};
use super::VectorError;

// ── UserProvidedEmbedder ──

/// Embedder for user-supplied vectors. Cannot generate embeddings —
/// only validates dimensions of vectors provided via `_vectors` field.
#[derive(Debug)]
pub struct UserProvidedEmbedder {
    dimensions: usize,
}

impl UserProvidedEmbedder {
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    pub async fn embed_documents(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorError> {
        Err(VectorError::EmbeddingError(
            "userProvided embedder cannot generate embeddings; supply vectors via _vectors field"
                .into(),
        ))
    }

    pub async fn embed_query(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
        Err(VectorError::EmbeddingError(
            "userProvided embedder cannot generate embeddings; supply vectors via _vectors field"
                .into(),
        ))
    }

    pub fn validate_vector(&self, vector: &[f32]) -> Result<(), VectorError> {
        if vector.len() != self.dimensions {
            return Err(VectorError::DimensionMismatch {
                expected: self.dimensions,
                got: vector.len(),
            });
        }
        Ok(())
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn source(&self) -> EmbedderSource {
        EmbedderSource::UserProvided
    }
}

// ── RestEmbedder ──

/// Generic HTTP embedder using request/response JSON templates.
#[derive(Debug)]
pub struct RestEmbedder {
    client: reqwest::Client,
    url: String,
    request_template: serde_json::Value,
    response_template: serde_json::Value,
    dimensions: usize,
}

// Stub — implementation follows in later items
impl RestEmbedder {
    pub fn new(config: &EmbedderConfig) -> Result<Self, VectorError> {
        config.validate()?;
        let mut client_builder = reqwest::Client::builder();
        let headers_map = config.headers.clone().unwrap_or_default();

        // Build default headers
        let mut header_map = reqwest::header::HeaderMap::new();
        for (k, v) in &headers_map {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| VectorError::EmbeddingError(format!("invalid header name: {e}")))?;
            let val = reqwest::header::HeaderValue::from_str(v)
                .map_err(|e| VectorError::EmbeddingError(format!("invalid header value: {e}")))?;
            header_map.insert(name, val);
        }
        client_builder = client_builder.default_headers(header_map);

        let client = client_builder.build().map_err(|e| {
            VectorError::EmbeddingError(format!("failed to build HTTP client: {e}"))
        })?;

        Ok(Self {
            client,
            url: config.url.clone().unwrap_or_default(),
            request_template: config.request.clone().unwrap_or(serde_json::Value::Null),
            response_template: config.response.clone().unwrap_or(serde_json::Value::Null),
            dimensions: config.dimensions.unwrap_or(0),
        })
    }

    pub async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        // Check if template supports batch (has {{..}} in an array with {{text}})
        if self.is_batch_template() {
            let body = self.render_batch_request(texts);
            let response = self.send_request(&body).await?;
            self.extract_batch_embeddings(&response)
        } else {
            // One request per text
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                let body = self.render_single_request(text);
                let response = self.send_request(&body).await?;
                let embeddings = self.extract_single_embedding(&response)?;
                results.push(embeddings);
            }
            Ok(results)
        }
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let results = self.embed_documents(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::EmbeddingError("empty response from embedder".into()))
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn source(&self) -> EmbedderSource {
        EmbedderSource::Rest
    }

    // ── Template rendering helpers ──

    fn is_batch_template(&self) -> bool {
        Self::json_contains_str(&self.request_template, "{{..}}")
    }

    fn render_single_request(&self, text: &str) -> serde_json::Value {
        Self::replace_text_placeholder(&self.request_template, text)
    }

    fn render_batch_request(&self, texts: &[&str]) -> serde_json::Value {
        Self::replace_batch_placeholders(&self.request_template, texts)
    }

    /// Walk JSON tree and replace `"{{text}}"` string values with actual text.
    fn replace_text_placeholder(value: &serde_json::Value, text: &str) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) if s == "{{text}}" => {
                serde_json::Value::String(text.to_owned())
            }
            serde_json::Value::Object(map) => {
                let new_map: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::replace_text_placeholder(v, text)))
                    .collect();
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                let new_arr: Vec<serde_json::Value> = arr
                    .iter()
                    .map(|v| Self::replace_text_placeholder(v, text))
                    .collect();
                serde_json::Value::Array(new_arr)
            }
            other => other.clone(),
        }
    }

    /// Walk JSON tree and replace arrays containing `["{{text}}", "{{..}}"]` with all texts.
    fn replace_batch_placeholders(value: &serde_json::Value, texts: &[&str]) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let new_map: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::replace_batch_placeholders(v, texts)))
                    .collect();
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                // Check if this array has both {{text}} and {{..}}
                let has_text = arr.iter().any(|v| v.as_str() == Some("{{text}}"));
                let has_repeat = arr.iter().any(|v| v.as_str() == Some("{{..}}"));
                if has_text && has_repeat {
                    // Replace with all texts
                    let new_arr: Vec<serde_json::Value> = texts
                        .iter()
                        .map(|t| serde_json::Value::String(t.to_string()))
                        .collect();
                    serde_json::Value::Array(new_arr)
                } else {
                    let new_arr: Vec<serde_json::Value> = arr
                        .iter()
                        .map(|v| Self::replace_batch_placeholders(v, texts))
                        .collect();
                    serde_json::Value::Array(new_arr)
                }
            }
            serde_json::Value::String(s) if s == "{{text}}" && !texts.is_empty() => {
                serde_json::Value::String(texts[0].to_string())
            }
            other => other.clone(),
        }
    }

    async fn send_request(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, VectorError> {
        let resp = self
            .client
            .post(&self.url)
            .json(body)
            .send()
            .await
            .map_err(|e| VectorError::EmbeddingError(format!("HTTP request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read response body".into());
            return Err(VectorError::EmbeddingError(format!(
                "embedder returned {status}: {body_text}"
            )));
        }

        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| VectorError::EmbeddingError(format!("failed to parse response JSON: {e}")))
    }

    /// Extract a single embedding from the response using the response template.
    fn extract_single_embedding(
        &self,
        response: &serde_json::Value,
    ) -> Result<Vec<f32>, VectorError> {
        let path = Self::find_embedding_path(&self.response_template);
        let embedding_val = Self::navigate_path(response, &path);
        Self::value_to_f32_vec(embedding_val)
    }

    /// Extract batch embeddings from the response.
    fn extract_batch_embeddings(
        &self,
        response: &serde_json::Value,
    ) -> Result<Vec<Vec<f32>>, VectorError> {
        let path = Self::find_embedding_path(&self.response_template);
        // Check if template has {{..}} indicating array of embeddings
        if Self::response_has_batch_marker(&self.response_template) {
            // Navigate to the parent array
            let parent_path = &path[..path.len().saturating_sub(1)];
            let arr_val = Self::navigate_path(response, parent_path);
            match arr_val {
                serde_json::Value::Array(arr) => {
                    let last_key = path.last().map(|s| s.as_str()).unwrap_or("");
                    arr.iter()
                        .map(|item| {
                            let emb = if last_key.is_empty() {
                                item
                            } else {
                                item.get(last_key).unwrap_or(item)
                            };
                            Self::value_to_f32_vec(emb)
                        })
                        .collect()
                }
                _ => {
                    // Fall back to single embedding
                    let embedding_val = Self::navigate_path(response, &path);
                    Ok(vec![Self::value_to_f32_vec(embedding_val)?])
                }
            }
        } else {
            // Single embedding path
            let embedding_val = Self::navigate_path(response, &path);
            Ok(vec![Self::value_to_f32_vec(embedding_val)?])
        }
    }

    /// Find the path to `{{embedding}}` in the response template.
    fn find_embedding_path(template: &serde_json::Value) -> Vec<String> {
        let mut path = Vec::new();
        Self::find_embedding_recursive(template, &mut path);
        path
    }

    fn find_embedding_recursive(value: &serde_json::Value, path: &mut Vec<String>) -> bool {
        match value {
            serde_json::Value::String(s) if s == "{{embedding}}" => true,
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    path.push(k.clone());
                    if Self::find_embedding_recursive(v, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            serde_json::Value::Array(arr) => {
                for (i, v) in arr.iter().enumerate() {
                    path.push(i.to_string());
                    if Self::find_embedding_recursive(v, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            _ => false,
        }
    }

    fn response_has_batch_marker(template: &serde_json::Value) -> bool {
        Self::json_contains_str(template, "{{..}}")
    }

    fn json_contains_str(value: &serde_json::Value, target: &str) -> bool {
        match value {
            serde_json::Value::String(s) => s == target,
            serde_json::Value::Object(map) => {
                map.values().any(|v| Self::json_contains_str(v, target))
            }
            serde_json::Value::Array(arr) => arr.iter().any(|v| Self::json_contains_str(v, target)),
            _ => false,
        }
    }

    /// Navigate a JSON value by a path of keys.
    fn navigate_path<'a>(value: &'a serde_json::Value, path: &[String]) -> &'a serde_json::Value {
        let mut current = value;
        for key in path {
            current = match current {
                serde_json::Value::Object(map) => {
                    map.get(key.as_str()).unwrap_or(&serde_json::Value::Null)
                }
                serde_json::Value::Array(arr) => {
                    if let Ok(idx) = key.parse::<usize>() {
                        arr.get(idx).unwrap_or(&serde_json::Value::Null)
                    } else {
                        &serde_json::Value::Null
                    }
                }
                _ => &serde_json::Value::Null,
            };
        }
        current
    }

    fn value_to_f32_vec(value: &serde_json::Value) -> Result<Vec<f32>, VectorError> {
        match value {
            serde_json::Value::Array(arr) => arr
                .iter()
                .map(|v| {
                    v.as_f64().map(|f| f as f32).ok_or_else(|| {
                        VectorError::EmbeddingError(
                            "embedding array contains non-numeric value".into(),
                        )
                    })
                })
                .collect(),
            _ => Err(VectorError::EmbeddingError(
                "expected array for embedding vector".into(),
            )),
        }
    }
}

// ── OpenAiEmbedder ──

/// OpenAI-compatible embedder (works with OpenAI, Azure, and proxies).
#[derive(Debug)]
pub struct OpenAiEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    configured_dimensions: Option<usize>,
    detected_dimensions: OnceLock<usize>,
}

impl OpenAiEmbedder {
    pub fn new(config: &EmbedderConfig) -> Result<Self, VectorError> {
        config.validate()?;
        let api_key = config
            .api_key
            .clone()
            .ok_or_else(|| VectorError::EmbeddingError("openAi embedder requires apiKey".into()))?;
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "text-embedding-3-small".into());
        let base_url = config
            .url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com".into());
        // Strip trailing slash for consistent URL building
        let base_url = base_url.trim_end_matches('/').to_owned();

        let client = reqwest::Client::builder().build().map_err(|e| {
            VectorError::EmbeddingError(format!("failed to build HTTP client: {e}"))
        })?;

        Ok(Self {
            client,
            api_key,
            model,
            base_url,
            configured_dimensions: config.dimensions,
            detected_dimensions: OnceLock::new(),
        })
    }

    pub async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/v1/embeddings", self.base_url);
        let mut body = serde_json::json!({
            "input": texts,
            "model": self.model,
            "encoding_format": "float"
        });
        if let Some(dims) = self.configured_dimensions {
            body["dimensions"] = serde_json::json!(dims);
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorError::EmbeddingError(format!("OpenAI request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "failed to read response body".into());
            // Try to parse OpenAI error format
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body_text) {
                if let Some(msg) = error_json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                {
                    return Err(VectorError::EmbeddingError(format!(
                        "OpenAI API error ({status}): {msg}"
                    )));
                }
            }
            return Err(VectorError::EmbeddingError(format!(
                "OpenAI API error ({status}): {body_text}"
            )));
        }

        let response: serde_json::Value = resp.json().await.map_err(|e| {
            VectorError::EmbeddingError(format!("failed to parse OpenAI response: {e}"))
        })?;

        // Parse data array, order by index
        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| {
                VectorError::EmbeddingError("OpenAI response missing `data` array".into())
            })?;

        let mut indexed: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
        for item in data {
            let index = item
                .get("index")
                .and_then(|i| i.as_u64())
                .unwrap_or(indexed.len() as u64) as usize;
            let embedding = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| {
                    VectorError::EmbeddingError("OpenAI response item missing `embedding`".into())
                })?;
            let vec: Vec<f32> = embedding
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();

            // Auto-detect dimensions from first response
            let _ = self.detected_dimensions.set(vec.len());

            indexed.push((index, vec));
        }

        // Sort by index to ensure correct ordering
        indexed.sort_by_key(|(i, _)| *i);
        Ok(indexed.into_iter().map(|(_, v)| v).collect())
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let results = self.embed_documents(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::EmbeddingError("empty response from OpenAI".into()))
    }

    pub fn dimensions(&self) -> usize {
        if let Some(d) = self.configured_dimensions {
            return d;
        }
        // Return auto-detected dimensions, or 0 if no embeddings have been made yet
        self.detected_dimensions.get().copied().unwrap_or(0)
    }

    pub fn source(&self) -> EmbedderSource {
        EmbedderSource::OpenAi
    }
}

// ── FastEmbedEmbedder ──

#[cfg(feature = "vector-search-local")]
fn parse_embedding_model(model: Option<&str>) -> Result<fastembed::EmbeddingModel, VectorError> {
    match model.map(|s| s.to_lowercase()).as_deref() {
        None | Some("bge-small-en-v1.5") => Ok(fastembed::EmbeddingModel::BGESmallENV15),
        Some("bge-base-en-v1.5") => Ok(fastembed::EmbeddingModel::BGEBaseENV15),
        Some("bge-large-en-v1.5") => Ok(fastembed::EmbeddingModel::BGELargeENV15),
        Some("all-minilm-l6-v2") => Ok(fastembed::EmbeddingModel::AllMiniLML6V2),
        Some("all-minilm-l12-v2") => Ok(fastembed::EmbeddingModel::AllMiniLML12V2),
        Some("nomic-embed-text-v1.5") => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15),
        Some("multilingual-e5-small") => Ok(fastembed::EmbeddingModel::MultilingualE5Small),
        Some(unknown) => Err(VectorError::EmbeddingError(format!(
            "unknown fastembed model: \"{unknown}\". Supported models: \
             bge-small-en-v1.5, bge-base-en-v1.5, bge-large-en-v1.5, \
             all-MiniLM-L6-v2, all-MiniLM-L12-v2, nomic-embed-text-v1.5, \
             multilingual-e5-small"
        ))),
    }
}

/// Local ONNX embedder using fastembed. Wraps `TextEmbedding` in a Mutex
/// because `embed()` requires `&mut self`. Uses `spawn_blocking` to run
/// the synchronous ONNX inference off the async runtime.
#[cfg(feature = "vector-search-local")]
pub struct FastEmbedEmbedder {
    model: std::sync::Arc<std::sync::Mutex<fastembed::TextEmbedding>>,
    dimensions: usize,
}

#[cfg(feature = "vector-search-local")]
impl std::fmt::Debug for FastEmbedEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedEmbedder")
            .field("dimensions", &self.dimensions)
            .finish()
    }
}

#[cfg(feature = "vector-search-local")]
impl FastEmbedEmbedder {
    pub fn new(config: &EmbedderConfig) -> Result<Self, VectorError> {
        let model_enum = parse_embedding_model(config.model.as_deref())?;

        let model_info = fastembed::TextEmbedding::get_model_info(&model_enum)
            .map_err(|e| VectorError::EmbeddingError(format!("failed to get model info: {e}")))?;
        let dim = model_info.dim;

        if let Some(configured_dim) = config.dimensions {
            if configured_dim != dim {
                return Err(VectorError::EmbeddingError(format!(
                    "configured dimensions ({configured_dim}) do not match model dimensions ({dim})"
                )));
            }
        }

        let mut options =
            fastembed::TextInitOptions::new(model_enum).with_show_download_progress(true);

        if let Ok(cache_dir) = std::env::var("FASTEMBED_CACHE_DIR") {
            options = options.with_cache_dir(std::path::PathBuf::from(cache_dir));
        }

        let text_embedding = fastembed::TextEmbedding::try_new(options).map_err(|e| {
            VectorError::EmbeddingError(format!("failed to initialize fastembed model: {e}"))
        })?;

        Ok(Self {
            model: std::sync::Arc::new(std::sync::Mutex::new(text_embedding)),
            dimensions: dim,
        })
    }

    pub async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let model = self.model.clone();
        let owned_texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

        tokio::task::spawn_blocking(move || {
            let mut guard = model.lock().map_err(|e| {
                VectorError::EmbeddingError(format!("fastembed mutex poisoned: {e}"))
            })?;
            guard.embed(owned_texts, None).map_err(|e| {
                VectorError::EmbeddingError(format!("fastembed embedding failed: {e}"))
            })
        })
        .await
        .map_err(|e| VectorError::EmbeddingError(format!("fastembed task panicked: {e}")))?
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        let results = self.embed_documents(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::EmbeddingError("empty response from fastembed".into()))
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn source(&self) -> EmbedderSource {
        EmbedderSource::FastEmbed
    }
}

// ── Embedder Enum ──

/// Dispatch enum for all embedder types. Uses enum dispatch instead of
/// trait objects because async fn in traits is not dyn-safe in Rust 1.93.
#[derive(Debug)]
pub enum Embedder {
    UserProvided(UserProvidedEmbedder),
    Rest(Box<RestEmbedder>),
    OpenAi(Box<OpenAiEmbedder>),
    #[cfg(feature = "vector-search-local")]
    FastEmbed(Box<FastEmbedEmbedder>),
}

impl Embedder {
    pub async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, VectorError> {
        match self {
            Embedder::UserProvided(e) => e.embed_documents(texts).await,
            Embedder::Rest(e) => e.embed_documents(texts).await,
            Embedder::OpenAi(e) => e.embed_documents(texts).await,
            #[cfg(feature = "vector-search-local")]
            Embedder::FastEmbed(e) => e.embed_documents(texts).await,
        }
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        match self {
            Embedder::UserProvided(e) => e.embed_query(text).await,
            Embedder::Rest(e) => e.embed_query(text).await,
            Embedder::OpenAi(e) => e.embed_query(text).await,
            #[cfg(feature = "vector-search-local")]
            Embedder::FastEmbed(e) => e.embed_query(text).await,
        }
    }

    pub fn dimensions(&self) -> usize {
        match self {
            Embedder::UserProvided(e) => e.dimensions(),
            Embedder::Rest(e) => e.dimensions(),
            Embedder::OpenAi(e) => e.dimensions(),
            #[cfg(feature = "vector-search-local")]
            Embedder::FastEmbed(e) => e.dimensions(),
        }
    }

    pub fn source(&self) -> EmbedderSource {
        match self {
            Embedder::UserProvided(e) => e.source(),
            Embedder::Rest(e) => e.source(),
            Embedder::OpenAi(e) => e.source(),
            #[cfg(feature = "vector-search-local")]
            Embedder::FastEmbed(e) => e.source(),
        }
    }
}

/// Factory: validate config and create the appropriate embedder variant.
pub fn create_embedder(config: &EmbedderConfig) -> Result<Embedder, VectorError> {
    config.validate()?;
    match config.source {
        EmbedderSource::UserProvided => {
            let dims = config.dimensions.unwrap_or(0);
            Ok(Embedder::UserProvided(UserProvidedEmbedder::new(dims)))
        }
        EmbedderSource::Rest => {
            let embedder = RestEmbedder::new(config)?;
            Ok(Embedder::Rest(Box::new(embedder)))
        }
        EmbedderSource::OpenAi => {
            let embedder = OpenAiEmbedder::new(config)?;
            Ok(Embedder::OpenAi(Box::new(embedder)))
        }
        #[cfg(feature = "vector-search-local")]
        EmbedderSource::FastEmbed => {
            let embedder = FastEmbedEmbedder::new(config)?;
            Ok(Embedder::FastEmbed(Box::new(embedder)))
        }
        #[cfg(not(feature = "vector-search-local"))]
        EmbedderSource::FastEmbed => Err(VectorError::EmbeddingError(
            "local embedding (source: \"fastEmbed\") requires the server to be compiled with the `vector-search-local` feature".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── UserProvidedEmbedder tests (3.9) ──

    #[test]
    fn test_user_provided_dimensions_getter() {
        let e = UserProvidedEmbedder::new(384);
        assert_eq!(e.dimensions(), 384);
        assert_eq!(e.source(), EmbedderSource::UserProvided);
    }

    #[test]
    fn test_user_provided_validate_correct_dimensions() {
        let e = UserProvidedEmbedder::new(3);
        assert!(e.validate_vector(&[1.0, 2.0, 3.0]).is_ok());
    }

    #[test]
    fn test_user_provided_validate_wrong_dimensions() {
        let e = UserProvidedEmbedder::new(3);
        let err = e.validate_vector(&[1.0, 2.0]).unwrap_err();
        match err {
            VectorError::DimensionMismatch { expected, got } => {
                assert_eq!(expected, 3);
                assert_eq!(got, 2);
            }
            other => panic!("expected DimensionMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_user_provided_embed_query_returns_error() {
        let e = UserProvidedEmbedder::new(3);
        let result = e.embed_query("hello").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            VectorError::EmbeddingError(msg) => {
                assert!(msg.contains("cannot generate embeddings"));
            }
            other => panic!("expected EmbeddingError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_user_provided_embed_documents_returns_error() {
        let e = UserProvidedEmbedder::new(3);
        let result = e.embed_documents(&["hello", "world"]).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            VectorError::EmbeddingError(_)
        ));
    }

    // ── Factory tests (3.30) ──

    #[test]
    fn test_factory_creates_user_provided() {
        let config = EmbedderConfig {
            source: EmbedderSource::UserProvided,
            dimensions: Some(768),
            ..Default::default()
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimensions(), 768);
        assert_eq!(embedder.source(), EmbedderSource::UserProvided);
    }

    #[test]
    fn test_factory_rejects_invalid_config() {
        let config = EmbedderConfig {
            source: EmbedderSource::OpenAi,
            // Missing api_key
            ..Default::default()
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_factory_creates_rest() {
        let config = EmbedderConfig {
            source: EmbedderSource::Rest,
            url: Some("http://localhost:1234/embed".into()),
            request: Some(serde_json::json!({"input": "{{text}}"})),
            response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
            dimensions: Some(384),
            ..Default::default()
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.source(), EmbedderSource::Rest);
        assert_eq!(embedder.dimensions(), 384);
    }

    #[test]
    fn test_factory_creates_openai() {
        let config = EmbedderConfig {
            source: EmbedderSource::OpenAi,
            api_key: Some("sk-test".into()),
            ..Default::default()
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.source(), EmbedderSource::OpenAi);
    }

    // ── RestEmbedder tests (3.13) ──

    mod rest_tests {
        use std::collections::HashMap;

        use super::*;
        use wiremock::matchers::{body_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        #[tokio::test]
        async fn test_rest_embedder_request_format() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/embed"))
                .and(body_json(serde_json::json!({"input": "hello world"})))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some(format!("{}/embed", server.uri())),
                request: Some(serde_json::json!({"input": "{{text}}"})),
                response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
                dimensions: Some(3),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let result = e.embed_query("hello world").await.unwrap();
            assert_eq!(result.len(), 3);
            assert!((result[0] - 0.1).abs() < 0.001);
        }

        #[tokio::test]
        async fn test_rest_embedder_response_parsing() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {"embedding": [1.0, 2.0, 3.0, 4.0]}
                })))
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some(format!("{}/embed", server.uri())),
                request: Some(serde_json::json!({"text": "{{text}}"})),
                response: Some(serde_json::json!({"data": {"embedding": "{{embedding}}"}})),
                dimensions: Some(4),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await.unwrap();
            assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
        }

        #[tokio::test]
        async fn test_rest_embedder_batch_request() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embeddings": [
                        [0.1, 0.2],
                        [0.3, 0.4]
                    ]
                })))
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some(format!("{}/embed", server.uri())),
                request: Some(serde_json::json!({"inputs": ["{{text}}", "{{..}}"]})),
                response: Some(serde_json::json!({"embeddings": ["{{embedding}}", "{{..}}"]})),
                dimensions: Some(2),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let results = e.embed_documents(&["hello", "world"]).await.unwrap();
            assert_eq!(results.len(), 2);
            assert_eq!(results[0], vec![0.1, 0.2]);
            assert_eq!(results[1], vec![0.3, 0.4]);
        }

        #[tokio::test]
        async fn test_rest_embedder_network_error() {
            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some("http://127.0.0.1:1/embed".into()),
                request: Some(serde_json::json!({"input": "{{text}}"})),
                response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
                dimensions: Some(3),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                VectorError::EmbeddingError(_)
            ));
        }

        #[tokio::test]
        async fn test_rest_embedder_bad_response() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some(format!("{}/embed", server.uri())),
                request: Some(serde_json::json!({"input": "{{text}}"})),
                response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
                dimensions: Some(3),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_rest_embedder_custom_headers() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(header("X-Custom", "my-value"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "embedding": [0.1, 0.2, 0.3]
                })))
                .mount(&server)
                .await;

            let mut headers = HashMap::new();
            headers.insert("X-Custom".into(), "my-value".into());

            let config = EmbedderConfig {
                source: EmbedderSource::Rest,
                url: Some(format!("{}/embed", server.uri())),
                request: Some(serde_json::json!({"input": "{{text}}"})),
                response: Some(serde_json::json!({"embedding": "{{embedding}}"})),
                headers: Some(headers),
                dimensions: Some(3),
                ..Default::default()
            };
            let e = RestEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await.unwrap();
            assert_eq!(result.len(), 3);
        }
    }

    // ── OpenAiEmbedder tests (3.20) ──

    mod openai_tests {
        use super::*;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn openai_response(embeddings: Vec<Vec<f64>>) -> serde_json::Value {
            let data: Vec<serde_json::Value> = embeddings
                .into_iter()
                .enumerate()
                .map(|(i, emb)| {
                    serde_json::json!({
                        "object": "embedding",
                        "embedding": emb,
                        "index": i
                    })
                })
                .collect();
            serde_json::json!({
                "object": "list",
                "data": data,
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 5, "total_tokens": 5}
            })
        }

        #[tokio::test]
        async fn test_openai_sends_correct_request() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/embeddings"))
                .and(header("Authorization", "Bearer sk-test123"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(openai_response(vec![vec![0.1, 0.2, 0.3]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test123".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            let result = e.embed_query("hello").await.unwrap();
            assert_eq!(result.len(), 3);
        }

        #[tokio::test]
        async fn test_openai_parses_response() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(openai_response(vec![vec![1.0, 2.0, 3.0]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await.unwrap();
            assert_eq!(result, vec![1.0, 2.0, 3.0]);
        }

        #[tokio::test]
        async fn test_openai_batch_multiple_texts() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(openai_response(vec![vec![0.1, 0.2], vec![0.3, 0.4]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            let results = e.embed_documents(&["hello", "world"]).await.unwrap();
            assert_eq!(results.len(), 2);
            assert_eq!(results[0], vec![0.1, 0.2]);
            assert_eq!(results[1], vec![0.3, 0.4]);
        }

        #[tokio::test]
        async fn test_openai_custom_model() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(openai_response(vec![vec![0.5, 0.5]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                model: Some("text-embedding-ada-002".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            assert_eq!(e.model, "text-embedding-ada-002");
            let result = e.embed_query("test").await.unwrap();
            assert_eq!(result.len(), 2);
        }

        #[tokio::test]
        async fn test_openai_custom_url() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/embeddings"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(openai_response(vec![vec![0.1]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            assert!(e.base_url.starts_with("http://127.0.0.1"));
            let result = e.embed_query("test").await.unwrap();
            assert_eq!(result.len(), 1);
        }

        #[tokio::test]
        async fn test_openai_error_response() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                    "error": {
                        "message": "Invalid API key",
                        "type": "invalid_request_error",
                        "code": "invalid_api_key"
                    }
                })))
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-bad".into()),
                url: Some(server.uri()),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            let result = e.embed_query("test").await;
            assert!(result.is_err());
            let err_msg = format!("{}", result.unwrap_err());
            assert!(err_msg.contains("Invalid API key"));
        }

        #[tokio::test]
        async fn test_openai_dimensions_in_request() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(openai_response(vec![vec![0.1, 0.2, 0.3]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                url: Some(server.uri()),
                dimensions: Some(256),
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            assert_eq!(e.dimensions(), 256);
            let _ = e.embed_query("test").await.unwrap();
        }

        #[tokio::test]
        async fn test_openai_dimensions_auto_detection() {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(openai_response(vec![vec![0.1, 0.2, 0.3, 0.4, 0.5]])),
                )
                .mount(&server)
                .await;

            let config = EmbedderConfig {
                source: EmbedderSource::OpenAi,
                api_key: Some("sk-test".into()),
                url: Some(server.uri()),
                // No dimensions configured — should auto-detect
                ..Default::default()
            };
            let e = OpenAiEmbedder::new(&config).unwrap();
            assert_eq!(e.dimensions(), 0); // Before first call
            let _ = e.embed_query("test").await.unwrap();
            assert_eq!(e.dimensions(), 5); // Auto-detected from response
        }
    }

    // ── FastEmbedEmbedder tests (9.7) ──

    #[cfg(feature = "vector-search-local")]
    mod fastembed_tests {
        use super::*;

        // ── Model lookup tests ──

        #[test]
        fn test_parse_embedding_model_default() {
            let model = parse_embedding_model(None).unwrap();
            assert!(matches!(model, fastembed::EmbeddingModel::BGESmallENV15));
        }

        #[test]
        fn test_parse_embedding_model_known() {
            let model = parse_embedding_model(Some("all-MiniLM-L6-v2")).unwrap();
            assert!(matches!(model, fastembed::EmbeddingModel::AllMiniLML6V2));
        }

        #[test]
        fn test_parse_embedding_model_case_insensitive() {
            let model = parse_embedding_model(Some("BGE-Small-EN-V1.5")).unwrap();
            assert!(matches!(model, fastembed::EmbeddingModel::BGESmallENV15));
        }

        #[test]
        fn test_parse_embedding_model_all_supported() {
            let cases = vec![
                (
                    "bge-small-en-v1.5",
                    fastembed::EmbeddingModel::BGESmallENV15,
                ),
                ("bge-base-en-v1.5", fastembed::EmbeddingModel::BGEBaseENV15),
                (
                    "bge-large-en-v1.5",
                    fastembed::EmbeddingModel::BGELargeENV15,
                ),
                ("all-MiniLM-L6-v2", fastembed::EmbeddingModel::AllMiniLML6V2),
                (
                    "all-MiniLM-L12-v2",
                    fastembed::EmbeddingModel::AllMiniLML12V2,
                ),
                (
                    "nomic-embed-text-v1.5",
                    fastembed::EmbeddingModel::NomicEmbedTextV15,
                ),
                (
                    "multilingual-e5-small",
                    fastembed::EmbeddingModel::MultilingualE5Small,
                ),
            ];
            for (input, expected) in cases {
                let result = parse_embedding_model(Some(input)).unwrap();
                assert_eq!(
                    std::mem::discriminant(&result),
                    std::mem::discriminant(&expected),
                    "failed for input: {input}"
                );
            }
        }

        #[test]
        fn test_parse_embedding_model_unknown() {
            let result = parse_embedding_model(Some("nonexistent-model"));
            assert!(result.is_err());
            let err = result.unwrap_err();
            match err {
                VectorError::EmbeddingError(msg) => {
                    assert!(
                        msg.contains("nonexistent-model"),
                        "error should mention the invalid model"
                    );
                    assert!(
                        msg.contains("bge-small-en-v1.5"),
                        "error should list valid models"
                    );
                }
                other => panic!("expected EmbeddingError, got: {other}"),
            }
        }

        // ── Embedder behavior tests ──

        fn fastembed_test_config() -> EmbedderConfig {
            EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                // Default model (bge-small-en-v1.5, 384d)
                ..Default::default()
            }
        }

        #[test]
        fn test_fastembed_dimensions_from_model() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            assert_eq!(e.dimensions(), 384);
        }

        #[test]
        fn test_fastembed_source_returns_fastembed() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            assert_eq!(e.source(), EmbedderSource::FastEmbed);
        }

        #[tokio::test]
        async fn test_fastembed_embed_documents() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            let texts = &["hello world", "rust programming", "vector search"];
            let results = e.embed_documents(texts).await.unwrap();
            assert_eq!(results.len(), 3);
            for vec in &results {
                assert_eq!(vec.len(), 384, "each vector should be 384-dim");
            }
        }

        #[tokio::test]
        async fn test_fastembed_embed_query() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            let result = e.embed_query("hello world").await.unwrap();
            assert_eq!(result.len(), 384);
        }

        #[tokio::test]
        async fn test_fastembed_embed_deterministic() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            let v1 = e.embed_query("deterministic test").await.unwrap();
            let v2 = e.embed_query("deterministic test").await.unwrap();
            assert_eq!(v1, v2, "same input should produce identical vectors");
        }

        #[tokio::test]
        async fn test_fastembed_embed_empty_batch() {
            let e = FastEmbedEmbedder::new(&fastembed_test_config()).unwrap();
            let results = e.embed_documents(&[]).await.unwrap();
            assert!(results.is_empty());
        }

        #[test]
        fn test_fastembed_dimension_mismatch_in_new() {
            let config = EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                // bge-small-en-v1.5 is 384d, but we claim 768
                dimensions: Some(768),
                ..Default::default()
            };
            let result = FastEmbedEmbedder::new(&config);
            assert!(result.is_err());
            match result.unwrap_err() {
                VectorError::EmbeddingError(msg) => {
                    assert!(
                        msg.contains("384"),
                        "error should mention actual dimensions"
                    );
                    assert!(
                        msg.contains("768"),
                        "error should mention configured dimensions"
                    );
                }
                other => panic!("expected EmbeddingError, got: {other}"),
            }
        }

        #[test]
        fn test_factory_creates_fastembed() {
            let config = EmbedderConfig {
                source: EmbedderSource::FastEmbed,
                ..Default::default()
            };
            let embedder = create_embedder(&config).unwrap();
            assert_eq!(embedder.source(), EmbedderSource::FastEmbed);
            assert_eq!(embedder.dimensions(), 384);
        }
    }

    // Test the error path when vector-search-local is NOT enabled
    #[cfg(not(feature = "vector-search-local"))]
    #[test]
    fn test_factory_fastembed_rejected_without_feature() {
        let config = EmbedderConfig {
            source: EmbedderSource::FastEmbed,
            ..Default::default()
        };
        let result = create_embedder(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            VectorError::EmbeddingError(msg) => {
                assert!(
                    msg.contains("vector-search-local"),
                    "error should mention the required feature flag, got: {msg}"
                );
            }
            other => panic!("expected EmbeddingError, got: {other}"),
        }
    }
}
