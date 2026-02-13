//! Tests for InsightEvent validation (schema.rs).

use flapjack::analytics::schema::InsightEvent;

fn valid_click_event() -> InsightEvent {
    InsightEvent {
        event_type: "click".to_string(),
        event_subtype: None,
        event_name: "Product Click".to_string(),
        index: "products".to_string(),
        user_token: "user123".to_string(),
        authenticated_user_token: None,
        query_id: Some("a".repeat(32)),
        object_ids: vec!["obj1".to_string()],
        object_ids_alt: vec![],
        positions: Some(vec![1]),
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: None,
        currency: None,
    }
}

fn valid_conversion_event() -> InsightEvent {
    InsightEvent {
        event_type: "conversion".to_string(),
        event_subtype: None,
        event_name: "Purchase".to_string(),
        index: "products".to_string(),
        user_token: "user123".to_string(),
        authenticated_user_token: None,
        query_id: Some("b".repeat(32)),
        object_ids: vec!["obj1".to_string()],
        object_ids_alt: vec![],
        positions: None,
        timestamp: Some(chrono::Utc::now().timestamp_millis()),
        value: Some(99.99),
        currency: Some("USD".to_string()),
    }
}

fn valid_view_event() -> InsightEvent {
    InsightEvent {
        event_type: "view".to_string(),
        event_subtype: None,
        event_name: "Product Viewed".to_string(),
        index: "products".to_string(),
        user_token: "user456".to_string(),
        authenticated_user_token: None,
        query_id: None,
        object_ids: vec!["obj1".to_string(), "obj2".to_string()],
        object_ids_alt: vec![],
        positions: None,
        timestamp: None,
        value: None,
        currency: None,
    }
}

#[test]
fn valid_click_passes() {
    assert!(valid_click_event().validate().is_ok());
}

#[test]
fn valid_conversion_passes() {
    assert!(valid_conversion_event().validate().is_ok());
}

#[test]
fn valid_view_passes() {
    assert!(valid_view_event().validate().is_ok());
}

#[test]
fn invalid_event_type() {
    let mut e = valid_click_event();
    e.event_type = "hover".to_string();
    let err = e.validate().unwrap_err();
    assert!(err.contains("Invalid eventType"), "got: {}", err);
}

#[test]
fn empty_event_name() {
    let mut e = valid_click_event();
    e.event_name = "".to_string();
    let err = e.validate().unwrap_err();
    assert!(err.contains("eventName"), "got: {}", err);
}

#[test]
fn event_name_too_long() {
    let mut e = valid_click_event();
    e.event_name = "a".repeat(65);
    let err = e.validate().unwrap_err();
    assert!(err.contains("eventName"), "got: {}", err);
}

#[test]
fn empty_user_token() {
    let mut e = valid_click_event();
    e.user_token = "".to_string();
    let err = e.validate().unwrap_err();
    assert!(err.contains("userToken"), "got: {}", err);
}

#[test]
fn user_token_too_long() {
    let mut e = valid_click_event();
    e.user_token = "x".repeat(130);
    let err = e.validate().unwrap_err();
    assert!(err.contains("userToken"), "got: {}", err);
}

#[test]
fn empty_object_ids() {
    let mut e = valid_click_event();
    e.object_ids = vec![];
    let err = e.validate().unwrap_err();
    assert!(err.contains("objectIDs"), "got: {}", err);
}

#[test]
fn too_many_object_ids() {
    let mut e = valid_click_event();
    e.object_ids = (0..21).map(|i| format!("obj{}", i)).collect();
    e.positions = Some((0..21).map(|i| i as u32).collect());
    let err = e.validate().unwrap_err();
    assert!(err.contains("objectIDs"), "got: {}", err);
}

#[test]
fn click_after_search_missing_positions() {
    let mut e = valid_click_event();
    e.positions = None;
    let err = e.validate().unwrap_err();
    assert!(err.contains("positions required"), "got: {}", err);
}

#[test]
fn click_after_search_positions_length_mismatch() {
    let mut e = valid_click_event();
    e.object_ids = vec!["obj1".to_string(), "obj2".to_string()];
    e.positions = Some(vec![1]); // Only 1 position for 2 objects
    let err = e.validate().unwrap_err();
    assert!(err.contains("positions length"), "got: {}", err);
}

#[test]
fn click_without_query_id_no_positions_ok() {
    // A click without queryID (not click-after-search) doesn't require positions
    let mut e = valid_click_event();
    e.query_id = None;
    e.positions = None;
    assert!(e.validate().is_ok());
}

#[test]
fn invalid_query_id_too_short() {
    let mut e = valid_click_event();
    e.query_id = Some("abc123".to_string());
    let err = e.validate().unwrap_err();
    assert!(err.contains("queryID"), "got: {}", err);
}

#[test]
fn invalid_query_id_non_hex() {
    let mut e = valid_click_event();
    e.query_id = Some("g".repeat(32)); // 'g' is not hex
    let err = e.validate().unwrap_err();
    assert!(err.contains("queryID"), "got: {}", err);
}

#[test]
fn timestamp_too_old_rejected() {
    let mut e = valid_click_event();
    // Set timestamp to 5 days ago
    let five_days_ago = chrono::Utc::now().timestamp_millis() - (5 * 24 * 60 * 60 * 1000);
    e.timestamp = Some(five_days_ago);
    let err = e.validate().unwrap_err();
    assert!(err.contains("4 days"), "got: {}", err);
}

#[test]
fn timestamp_within_4_days_accepted() {
    let mut e = valid_click_event();
    // Set timestamp to 3 days ago (within 4 day window)
    let three_days_ago = chrono::Utc::now().timestamp_millis() - (3 * 24 * 60 * 60 * 1000);
    e.timestamp = Some(three_days_ago);
    assert!(e.validate().is_ok());
}

#[test]
fn no_timestamp_accepted() {
    let mut e = valid_click_event();
    e.timestamp = None;
    assert!(e.validate().is_ok());
}

#[test]
fn effective_object_ids_prefers_primary() {
    let e = InsightEvent {
        event_type: "view".to_string(),
        event_subtype: None,
        event_name: "View".to_string(),
        index: "products".to_string(),
        user_token: "user1".to_string(),
        authenticated_user_token: None,
        query_id: None,
        object_ids: vec!["primary".to_string()],
        object_ids_alt: vec!["alt".to_string()],
        positions: None,
        timestamp: None,
        value: None,
        currency: None,
    };
    assert_eq!(e.effective_object_ids(), &["primary".to_string()]);
}

#[test]
fn effective_object_ids_falls_back_to_alt() {
    let e = InsightEvent {
        event_type: "view".to_string(),
        event_subtype: None,
        event_name: "View".to_string(),
        index: "products".to_string(),
        user_token: "user1".to_string(),
        authenticated_user_token: None,
        query_id: None,
        object_ids: vec![],
        object_ids_alt: vec!["alt1".to_string(), "alt2".to_string()],
        positions: None,
        timestamp: None,
        value: None,
        currency: None,
    };
    assert_eq!(
        e.effective_object_ids(),
        &["alt1".to_string(), "alt2".to_string()]
    );
}

#[test]
fn max_boundary_event_name_64_chars() {
    let mut e = valid_click_event();
    e.event_name = "a".repeat(64);
    assert!(e.validate().is_ok());
}

#[test]
fn max_boundary_user_token_129_chars() {
    let mut e = valid_click_event();
    e.user_token = "x".repeat(129);
    assert!(e.validate().is_ok());
}

#[test]
fn max_boundary_20_object_ids() {
    let mut e = valid_view_event();
    e.object_ids = (0..20).map(|i| format!("obj{}", i)).collect();
    assert!(e.validate().is_ok());
}
