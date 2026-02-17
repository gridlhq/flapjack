use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

const SEGMENT_MAX_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpLogEntry {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub node_id: String,
    pub tenant_id: String,
    pub op_type: String,
    pub payload: serde_json::Value,
}

struct ActiveSegment {
    writer: BufWriter<File>,
    path: PathBuf,
    size: u64,
    id: u32,
}

pub struct OpLog {
    dir: PathBuf,
    tenant_id: String,
    node_id: String,
    current_seq: AtomicU64,
    segment: Mutex<ActiveSegment>,
}

impl OpLog {
    pub fn open(dir: &Path, tenant_id: &str, node_id: &str) -> crate::error::Result<Self> {
        fs::create_dir_all(dir)?;

        let (max_seq, max_seg_id) = Self::scan_existing(dir)?;
        let next_seg_id = if max_seg_id > 0 { max_seg_id } else { 1 };
        let seg_path = dir.join(format!("segment_{:04}.jsonl", next_seg_id));
        let seg_size = seg_path.metadata().map(|m| m.len()).unwrap_or(0);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&seg_path)?;

        Ok(OpLog {
            dir: dir.to_path_buf(),
            tenant_id: tenant_id.to_string(),
            node_id: node_id.to_string(),
            current_seq: AtomicU64::new(max_seq),
            segment: Mutex::new(ActiveSegment {
                writer: BufWriter::new(file),
                path: seg_path,
                size: seg_size,
                id: next_seg_id,
            }),
        })
    }

    fn scan_existing(dir: &Path) -> crate::error::Result<(u64, u32)> {
        let mut max_seq: u64 = 0;
        let mut max_seg_id: u32 = 0;

        let mut entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("segment_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in &entries {
            let name = entry.file_name();
            let name_str = name.to_str().unwrap_or("");
            if let Some(id_str) = name_str
                .strip_prefix("segment_")
                .and_then(|s| s.strip_suffix(".jsonl"))
            {
                if let Ok(id) = id_str.parse::<u32>() {
                    if id > max_seg_id {
                        max_seg_id = id;
                    }
                }
            }
        }

        if let Some(last) = entries.last() {
            let f = File::open(last.path())?;
            let reader = BufReader::new(f);
            for line in reader.lines() {
                let line = line?;
                if let Ok(entry) = serde_json::from_str::<OpLogEntry>(&line) {
                    if entry.seq > max_seq {
                        max_seq = entry.seq;
                    }
                }
            }
        }

        Ok((max_seq, max_seg_id))
    }

    pub fn current_seq(&self) -> u64 {
        self.current_seq.load(Ordering::SeqCst)
    }

    pub fn append(&self, op_type: &str, payload: serde_json::Value) -> crate::error::Result<u64> {
        let seq = self.current_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let entry = OpLogEntry {
            seq,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            node_id: self.node_id.clone(),
            tenant_id: self.tenant_id.clone(),
            op_type: op_type.to_string(),
            payload,
        };

        let line = serde_json::to_string(&entry)
            .map_err(|e| crate::error::FlapjackError::Io(e.to_string()))?;

        let mut seg = self.segment.lock().unwrap();
        seg.writer.write_all(line.as_bytes())?;
        seg.writer.write_all(b"\n")?;
        seg.writer.flush()?;
        seg.size += line.len() as u64 + 1;

        if seg.size >= SEGMENT_MAX_BYTES {
            self.rotate_segment_locked(&mut seg)?;
        }

        Ok(seq)
    }

    pub fn append_batch(&self, ops: &[(String, serde_json::Value)]) -> crate::error::Result<u64> {
        let mut last_seq = self.current_seq.load(Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut seg = self.segment.lock().unwrap();
        for (op_type, payload) in ops {
            last_seq += 1;
            let entry = OpLogEntry {
                seq: last_seq,
                timestamp_ms: now,
                node_id: self.node_id.clone(),
                tenant_id: self.tenant_id.clone(),
                op_type: op_type.clone(),
                payload: payload.clone(),
            };
            let line = serde_json::to_string(&entry)
                .map_err(|e| crate::error::FlapjackError::Io(e.to_string()))?;
            seg.writer.write_all(line.as_bytes())?;
            seg.writer.write_all(b"\n")?;
            seg.size += line.len() as u64 + 1;
        }
        seg.writer.flush()?;
        self.current_seq.store(last_seq, Ordering::SeqCst);

        if seg.size >= SEGMENT_MAX_BYTES {
            self.rotate_segment_locked(&mut seg)?;
        }

        Ok(last_seq)
    }

    fn rotate_segment_locked(&self, seg: &mut ActiveSegment) -> crate::error::Result<()> {
        seg.writer.flush()?;
        seg.id += 1;
        let new_path = self.dir.join(format!("segment_{:04}.jsonl", seg.id));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&new_path)?;
        seg.writer = BufWriter::new(file);
        seg.path = new_path;
        seg.size = 0;
        Ok(())
    }

    pub fn read_since(&self, since_seq: u64) -> crate::error::Result<Vec<OpLogEntry>> {
        let mut results = Vec::new();
        let mut entries: Vec<_> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("segment_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        {
            let mut seg = self.segment.lock().unwrap();
            seg.writer.flush()?;
        }

        for entry in entries {
            let f = File::open(entry.path())?;
            let reader = BufReader::new(f);
            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<OpLogEntry>(&line) {
                    Ok(op) => {
                        if op.seq > since_seq {
                            results.push(op);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
        results.sort_by_key(|e| e.seq);
        Ok(results)
    }

    pub fn truncate_before(&self, before_seq: u64) -> crate::error::Result<u64> {
        let mut removed = 0u64;
        let seg = self.segment.lock().unwrap();
        let current_seg_name = seg.path.file_name().unwrap().to_str().unwrap().to_string();
        drop(seg);

        let mut entries: Vec<_> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("segment_") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let name = entry.file_name().to_str().unwrap().to_string();
            if name == current_seg_name {
                continue;
            }
            let f = File::open(entry.path())?;
            let reader = BufReader::new(f);
            let mut max_seq_in_file = 0u64;
            for line in reader.lines() {
                let line = line?;
                if let Ok(op) = serde_json::from_str::<OpLogEntry>(&line) {
                    if op.seq > max_seq_in_file {
                        max_seq_in_file = op.seq;
                    }
                }
            }
            if max_seq_in_file > 0 && max_seq_in_file < before_seq {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_append_and_read() {
        let tmp = TempDir::new().unwrap();
        let oplog = OpLog::open(tmp.path(), "t1", "node1").unwrap();

        assert_eq!(oplog.current_seq(), 0);
        let s1 = oplog
            .append("upsert", serde_json::json!({"objectID": "1"}))
            .unwrap();
        assert_eq!(s1, 1);
        let s2 = oplog
            .append("delete", serde_json::json!({"objectID": "2"}))
            .unwrap();
        assert_eq!(s2, 2);

        let all = oplog.read_since(0).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].seq, 1);
        assert_eq!(all[1].seq, 2);

        let since1 = oplog.read_since(1).unwrap();
        assert_eq!(since1.len(), 1);
        assert_eq!(since1[0].seq, 2);
    }

    #[test]
    fn test_batch_append() {
        let tmp = TempDir::new().unwrap();
        let oplog = OpLog::open(tmp.path(), "t1", "node1").unwrap();

        let ops: Vec<(String, serde_json::Value)> = vec![
            ("upsert".into(), serde_json::json!({"objectID": "a"})),
            ("upsert".into(), serde_json::json!({"objectID": "b"})),
            ("delete".into(), serde_json::json!({"objectID": "c"})),
        ];
        let last = oplog.append_batch(&ops).unwrap();
        assert_eq!(last, 3);
        assert_eq!(oplog.current_seq(), 3);

        let all = oplog.read_since(0).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_reopen_continues_seq() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();

        {
            let oplog = OpLog::open(&dir, "t1", "node1").unwrap();
            oplog.append("upsert", serde_json::json!({"x": 1})).unwrap();
            oplog.append("upsert", serde_json::json!({"x": 2})).unwrap();
        }

        let oplog2 = OpLog::open(&dir, "t1", "node1").unwrap();
        assert_eq!(oplog2.current_seq(), 2);
        let s3 = oplog2
            .append("delete", serde_json::json!({"x": 3}))
            .unwrap();
        assert_eq!(s3, 3);

        let all = oplog2.read_since(0).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_truncate() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();

        {
            let oplog = OpLog::open(&dir, "t1", "node1").unwrap();
            for i in 0..5 {
                oplog.append("upsert", serde_json::json!({"i": i})).unwrap();
            }
            oplog
                .rotate_segment_locked(&mut oplog.segment.lock().unwrap())
                .unwrap();
            for i in 5..10 {
                oplog.append("upsert", serde_json::json!({"i": i})).unwrap();
            }
        }

        let oplog = OpLog::open(&dir, "t1", "node1").unwrap();
        let removed = oplog.truncate_before(6).unwrap();
        assert_eq!(removed, 1);

        let remaining = oplog.read_since(0).unwrap();
        assert_eq!(remaining.len(), 5);
        assert_eq!(remaining[0].seq, 6);
    }
}
