use arrow::array::{
    ArrayRef, BooleanBuilder, Float64Builder, Int64Builder, StringBuilder, UInt32Builder,
};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::schema::{insight_event_schema, search_event_schema, InsightEvent, SearchEvent};

/// Write search events to a Parquet file with ZSTD compression.
pub fn flush_search_events(events: &[SearchEvent], dir: &Path) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }

    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", date));
    fs::create_dir_all(&partition_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let filename = format!("searches_{}.parquet", timestamp);
    let path = partition_dir.join(filename);

    let schema = search_event_schema();
    let batch = search_events_to_batch(events, &schema)?;

    write_parquet_file(&path, batch)?;
    Ok(())
}

/// Write insight events to a Parquet file with ZSTD compression.
pub fn flush_insight_events(events: &[InsightEvent], dir: &Path) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }

    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let partition_dir = dir.join(format!("date={}", date));
    fs::create_dir_all(&partition_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let filename = format!("events_{}.parquet", timestamp);
    let path = partition_dir.join(filename);

    let schema = insight_event_schema();
    let batch = insight_events_to_batch(events, &schema)?;

    write_parquet_file(&path, batch)?;
    Ok(())
}

fn write_parquet_file(path: &Path, batch: RecordBatch) -> Result<(), String> {
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .set_max_row_group_size(100_000)
        .build();

    let file =
        fs::File::create(path).map_err(|e| format!("Failed to create parquet file: {}", e))?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))
        .map_err(|e| format!("Failed to create arrow writer: {}", e))?;

    writer
        .write(&batch)
        .map_err(|e| format!("Failed to write batch: {}", e))?;
    writer
        .close()
        .map_err(|e| format!("Failed to close writer: {}", e))?;

    Ok(())
}

fn search_events_to_batch(
    events: &[SearchEvent],
    schema: &Arc<arrow::datatypes::Schema>,
) -> Result<RecordBatch, String> {
    let len = events.len();
    let mut timestamp_ms = Int64Builder::with_capacity(len);
    let mut query = StringBuilder::with_capacity(len, len * 20);
    let mut query_id = StringBuilder::with_capacity(len, len * 32);
    let mut index_name = StringBuilder::with_capacity(len, len * 20);
    let mut nb_hits = UInt32Builder::with_capacity(len);
    let mut processing_time_ms = UInt32Builder::with_capacity(len);
    let mut user_token = StringBuilder::with_capacity(len, len * 20);
    let mut user_ip = StringBuilder::with_capacity(len, len * 15);
    let mut filters = StringBuilder::with_capacity(len, len * 30);
    let mut facets = StringBuilder::with_capacity(len, len * 30);
    let mut analytics_tags = StringBuilder::with_capacity(len, len * 20);
    let mut page = UInt32Builder::with_capacity(len);
    let mut hits_per_page = UInt32Builder::with_capacity(len);
    let mut has_results = BooleanBuilder::with_capacity(len);
    let mut country = StringBuilder::with_capacity(len, len * 2);
    let mut region = StringBuilder::with_capacity(len, len * 10);

    for e in events {
        timestamp_ms.append_value(e.timestamp_ms);
        query.append_value(&e.query);
        match &e.query_id {
            Some(qid) => query_id.append_value(qid),
            None => query_id.append_null(),
        }
        index_name.append_value(&e.index_name);
        nb_hits.append_value(e.nb_hits);
        processing_time_ms.append_value(e.processing_time_ms);
        match &e.user_token {
            Some(t) => user_token.append_value(t),
            None => user_token.append_null(),
        }
        match &e.user_ip {
            Some(ip) => user_ip.append_value(ip),
            None => user_ip.append_null(),
        }
        match &e.filters {
            Some(f) => filters.append_value(f),
            None => filters.append_null(),
        }
        match &e.facets {
            Some(f) => facets.append_value(f),
            None => facets.append_null(),
        }
        match &e.analytics_tags {
            Some(t) => analytics_tags.append_value(t),
            None => analytics_tags.append_null(),
        }
        page.append_value(e.page);
        hits_per_page.append_value(e.hits_per_page);
        has_results.append_value(e.has_results);
        match &e.country {
            Some(c) => country.append_value(c),
            None => country.append_null(),
        }
        match &e.region {
            Some(r) => region.append_value(r),
            None => region.append_null(),
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(timestamp_ms.finish()),
        Arc::new(query.finish()),
        Arc::new(query_id.finish()),
        Arc::new(index_name.finish()),
        Arc::new(nb_hits.finish()),
        Arc::new(processing_time_ms.finish()),
        Arc::new(user_token.finish()),
        Arc::new(user_ip.finish()),
        Arc::new(filters.finish()),
        Arc::new(facets.finish()),
        Arc::new(analytics_tags.finish()),
        Arc::new(page.finish()),
        Arc::new(hits_per_page.finish()),
        Arc::new(has_results.finish()),
        Arc::new(country.finish()),
        Arc::new(region.finish()),
    ];

    RecordBatch::try_new(schema.clone(), columns).map_err(|e| format!("RecordBatch error: {}", e))
}

fn insight_events_to_batch(
    events: &[InsightEvent],
    schema: &Arc<arrow::datatypes::Schema>,
) -> Result<RecordBatch, String> {
    let len = events.len();
    let mut timestamp_ms = Int64Builder::with_capacity(len);
    let mut event_type = StringBuilder::with_capacity(len, len * 10);
    let mut event_subtype = StringBuilder::with_capacity(len, len * 10);
    let mut event_name = StringBuilder::with_capacity(len, len * 30);
    let mut index_name = StringBuilder::with_capacity(len, len * 20);
    let mut user_token = StringBuilder::with_capacity(len, len * 20);
    let mut auth_user_token = StringBuilder::with_capacity(len, len * 20);
    let mut query_id = StringBuilder::with_capacity(len, len * 32);
    let mut object_ids = StringBuilder::with_capacity(len, len * 50);
    let mut positions = StringBuilder::with_capacity(len, len * 20);
    let mut value = Float64Builder::with_capacity(len);
    let mut currency = StringBuilder::with_capacity(len, len * 3);

    for e in events {
        let ts = e
            .timestamp
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        timestamp_ms.append_value(ts);
        event_type.append_value(&e.event_type);
        match &e.event_subtype {
            Some(s) => event_subtype.append_value(s),
            None => event_subtype.append_null(),
        }
        event_name.append_value(&e.event_name);
        index_name.append_value(&e.index);
        user_token.append_value(&e.user_token);
        match &e.authenticated_user_token {
            Some(t) => auth_user_token.append_value(t),
            None => auth_user_token.append_null(),
        }
        match &e.query_id {
            Some(qid) => query_id.append_value(qid),
            None => query_id.append_null(),
        }
        let oids_json = serde_json::to_string(e.effective_object_ids()).unwrap_or_default();
        object_ids.append_value(&oids_json);
        match &e.positions {
            Some(p) => {
                let pos_json = serde_json::to_string(p).unwrap_or_default();
                positions.append_value(&pos_json);
            }
            None => positions.append_null(),
        }
        match e.value {
            Some(v) => value.append_value(v),
            None => value.append_null(),
        }
        match &e.currency {
            Some(c) => currency.append_value(c),
            None => currency.append_null(),
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(timestamp_ms.finish()),
        Arc::new(event_type.finish()),
        Arc::new(event_subtype.finish()),
        Arc::new(event_name.finish()),
        Arc::new(index_name.finish()),
        Arc::new(user_token.finish()),
        Arc::new(auth_user_token.finish()),
        Arc::new(query_id.finish()),
        Arc::new(object_ids.finish()),
        Arc::new(positions.finish()),
        Arc::new(value.finish()),
        Arc::new(currency.finish()),
    ];

    RecordBatch::try_new(schema.clone(), columns).map_err(|e| format!("RecordBatch error: {}", e))
}
