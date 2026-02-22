use super::config::Experiment;

#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentMethod {
    UserToken,
    SessionId,
    QueryId,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub arm: &'static str,
    pub method: AssignmentMethod,
}

pub fn assign_variant(
    experiment: &Experiment,
    user_token: Option<&str>,
    session_id: Option<&str>,
    query_id: &str,
) -> Assignment {
    let (key_suffix, method) = if let Some(ut) = user_token {
        (ut, AssignmentMethod::UserToken)
    } else if let Some(sid) = session_id {
        (sid, AssignmentMethod::SessionId)
    } else {
        (query_id, AssignmentMethod::QueryId)
    };

    let key = format!("{}:{}", experiment.id, key_suffix);
    let (h1, _) = murmurhash3_128(key.as_bytes(), 0);
    let bucket = (h1 % 10_000) as f64 / 10_000.0;

    let arm = if bucket < experiment.traffic_split {
        "variant"
    } else {
        "control"
    };
    Assignment { arm, method }
}

/// MurmurHash3_x64_128. Returns (h1, h2) — use h1 (lower 64 bits) for bucketing.
pub(crate) fn murmurhash3_128(data: &[u8], seed: u64) -> (u64, u64) {
    const C1: u64 = 0x87c37b91114253d5;
    const C2: u64 = 0x4cf5ad432745937f;

    let len = data.len();
    let mut h1 = seed;
    let mut h2 = seed;
    let nblocks = len / 16;

    // body — process 16-byte blocks
    for i in 0..nblocks {
        let off = i * 16;
        let mut k1 = u64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        let mut k2 = u64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap());

        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
        h1 = h1.rotate_left(27);
        h1 = h1.wrapping_add(h2);
        h1 = h1.wrapping_mul(5).wrapping_add(0x52dce729);

        k2 = k2.wrapping_mul(C2);
        k2 = k2.rotate_left(33);
        k2 = k2.wrapping_mul(C1);
        h2 ^= k2;
        h2 = h2.rotate_left(31);
        h2 = h2.wrapping_add(h1);
        h2 = h2.wrapping_mul(5).wrapping_add(0x38495ab5);
    }

    // tail — process remaining bytes
    let tail = &data[nblocks * 16..];
    let mut k1: u64 = 0;
    let mut k2: u64 = 0;

    #[allow(clippy::identity_op)]
    match tail.len() {
        15 => { k2 ^= (tail[14] as u64) << 48; k2 ^= (tail[13] as u64) << 40; k2 ^= (tail[12] as u64) << 32; k2 ^= (tail[11] as u64) << 24; k2 ^= (tail[10] as u64) << 16; k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        14 => { k2 ^= (tail[13] as u64) << 40; k2 ^= (tail[12] as u64) << 32; k2 ^= (tail[11] as u64) << 24; k2 ^= (tail[10] as u64) << 16; k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        13 => { k2 ^= (tail[12] as u64) << 32; k2 ^= (tail[11] as u64) << 24; k2 ^= (tail[10] as u64) << 16; k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        12 => { k2 ^= (tail[11] as u64) << 24; k2 ^= (tail[10] as u64) << 16; k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        11 => { k2 ^= (tail[10] as u64) << 16; k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        10 => { k2 ^= (tail[9] as u64) << 8; k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        9 =>  { k2 ^= tail[8] as u64; k2 = k2.wrapping_mul(C2); k2 = k2.rotate_left(33); k2 = k2.wrapping_mul(C1); h2 ^= k2; k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        8 =>  { k1 ^= (tail[7] as u64) << 56; k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        7 =>  { k1 ^= (tail[6] as u64) << 48; k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        6 =>  { k1 ^= (tail[5] as u64) << 40; k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        5 =>  { k1 ^= (tail[4] as u64) << 32; k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        4 =>  { k1 ^= (tail[3] as u64) << 24; k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        3 =>  { k1 ^= (tail[2] as u64) << 16; k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        2 =>  { k1 ^= (tail[1] as u64) << 8; k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        1 =>  { k1 ^= tail[0] as u64; k1 = k1.wrapping_mul(C1); k1 = k1.rotate_left(31); k1 = k1.wrapping_mul(C2); h1 ^= k1; }
        _ => {}
    }

    // finalization
    h1 ^= len as u64;
    h2 ^= len as u64;
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    h1 = fmix64(h1);
    h2 = fmix64(h2);
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);

    (h1, h2)
}

fn fmix64(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51afd7ed558ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ceb9fe1a85ec53);
    k ^= k >> 33;
    k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiments::config::*;

    fn exp_with_split(split: f64) -> Experiment {
        Experiment {
            id: "test-experiment-id".to_string(),
            traffic_split: split,
            name: "t".to_string(),
            index_name: "i".to_string(),
            status: ExperimentStatus::Draft,
            control: ExperimentArm {
                name: "control".to_string(),
                query_overrides: None,
                index_name: None,
            },
            variant: ExperimentArm {
                name: "variant".to_string(),
                query_overrides: Some(QueryOverrides::default()),
                index_name: None,
            },
            primary_metric: PrimaryMetric::Ctr,
            created_at: 0,
            started_at: None,
            ended_at: None,
            minimum_days: 14,
            winsorization_cap: None,
            conclusion: None,
        }
    }

    #[test]
    fn assignment_is_deterministic_for_same_inputs() {
        let exp = exp_with_split(0.5);
        let result1 = assign_variant(&exp, Some("user-abc"), None, "qid-xyz");
        let result2 = assign_variant(&exp, Some("user-abc"), None, "qid-xyz");
        assert_eq!(result1.arm, result2.arm);
        assert_eq!(result1.method, result2.method);
    }

    #[test]
    fn different_users_can_get_different_arms() {
        let exp = exp_with_split(0.5);
        let arms: Vec<&str> = (0..100)
            .map(|i| assign_variant(&exp, Some(&format!("user-{}", i)), None, "qid").arm)
            .collect();
        let variant_count = arms.iter().filter(|&&a| a == "variant").count();
        assert!(variant_count > 0);
        assert!(variant_count < 100);
    }

    #[test]
    fn assignment_respects_50_50_split_within_0_3_percent() {
        let exp = exp_with_split(0.5);
        let n = 100_000u64;
        let variant_count = (0..n)
            .filter(|i| assign_variant(&exp, Some(&format!("u{}", i)), None, "q").arm == "variant")
            .count() as u64;
        let ratio = variant_count as f64 / n as f64;
        assert!((ratio - 0.5).abs() < 0.003, "ratio was {}", ratio);
    }

    #[test]
    fn assignment_respects_20_percent_split_within_0_3_percent() {
        let exp = exp_with_split(0.2);
        let n = 100_000u64;
        let variant_count = (0..n)
            .filter(|i| assign_variant(&exp, Some(&format!("u{}", i)), None, "q").arm == "variant")
            .count() as u64;
        let ratio = variant_count as f64 / n as f64;
        assert!((ratio - 0.2).abs() < 0.003, "ratio was {}", ratio);
    }

    #[test]
    fn murmurhash3_128_known_vector_empty() {
        // MurmurHash3_x64_128("", seed=0) = (0, 0) — fmix64(0) is 0
        let (h1, h2) = murmurhash3_128(b"", 0);
        assert_eq!(h1, 0);
        assert_eq!(h2, 0);
    }

    #[test]
    fn murmurhash3_128_non_empty_is_nonzero() {
        let (h1, _) = murmurhash3_128(b"test", 0);
        assert_ne!(h1, 0, "non-empty input should hash to non-zero");
    }

    #[test]
    fn murmurhash3_128_matches_reference_implementation() {
        // Cross-validated against Python mmh3.hash_bytes(b"Hello, world!", seed=0)
        // interpreted as two little-endian u64s
        let (h1, h2) = murmurhash3_128(b"Hello, world!", 0);
        assert_eq!(h1, 0xf1512dd1d2d665df, "h1 mismatch vs Python mmh3");
        assert_eq!(h2, 0x2c326650a8f3c564, "h2 mismatch vs Python mmh3");
    }

    #[test]
    fn murmurhash3_128_is_deterministic() {
        let (h1a, h2a) = murmurhash3_128(b"hello world", 0);
        let (h1b, h2b) = murmurhash3_128(b"hello world", 0);
        assert_eq!(h1a, h1b);
        assert_eq!(h2a, h2b);
    }

    #[test]
    fn murmurhash3_128_different_inputs_differ() {
        let (h1a, _) = murmurhash3_128(b"input-a", 0);
        let (h1b, _) = murmurhash3_128(b"input-b", 0);
        assert_ne!(h1a, h1b);
    }

    #[test]
    fn murmurhash3_128_different_seeds_differ() {
        let (h1a, _) = murmurhash3_128(b"same-input", 0);
        let (h1b, _) = murmurhash3_128(b"same-input", 42);
        assert_ne!(h1a, h1b);
    }

    #[test]
    fn user_token_takes_priority_over_query_id() {
        let exp = exp_with_split(0.5);
        let result = assign_variant(&exp, Some("user-stable"), None, "qid-unstable");
        assert_eq!(result.method, AssignmentMethod::UserToken);
    }

    #[test]
    fn user_token_takes_priority_over_session_id() {
        let exp = exp_with_split(0.5);
        let result = assign_variant(&exp, Some("user-stable"), Some("session-123"), "qid-xyz");
        assert_eq!(result.method, AssignmentMethod::UserToken);
        // Verify the arm is determined by user_token, not session_id
        let result_no_session = assign_variant(&exp, Some("user-stable"), None, "qid-xyz");
        assert_eq!(result.arm, result_no_session.arm);
    }

    #[test]
    fn session_id_takes_priority_over_query_id() {
        let exp = exp_with_split(0.5);
        let result = assign_variant(&exp, None, Some("session-123"), "qid-xyz");
        assert_eq!(result.method, AssignmentMethod::SessionId);
    }

    #[test]
    fn query_id_fallback_produces_query_method() {
        let exp = exp_with_split(0.5);
        let result = assign_variant(&exp, None, None, "qid-xyz");
        assert_eq!(result.method, AssignmentMethod::QueryId);
    }

    #[test]
    fn assignment_changes_when_experiment_id_changes() {
        let mut exp1 = exp_with_split(0.5);
        let mut exp2 = exp_with_split(0.5);
        exp1.id = "experiment-aaa".to_string();
        exp2.id = "experiment-bbb".to_string();
        let differs = (0..1000).any(|i| {
            let u = format!("user-{}", i);
            assign_variant(&exp1, Some(&u), None, "q").arm
                != assign_variant(&exp2, Some(&u), None, "q").arm
        });
        assert!(differs, "assignments should vary between experiments");
    }
}
