use flapjack::index::rules::{Anchoring, Condition, Consequence, Hide, Promote, Rule, RuleStore};

#[test]
fn test_anchoring_is() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Is,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("laptop", None));
    assert!(rule.matches("LAPTOP", None));
    assert!(!rule.matches("gaming laptop", None));
    assert!(!rule.matches("lapto", None));
}

#[test]
fn test_anchoring_starts_with() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "gam".to_string(),
            anchoring: Anchoring::StartsWith,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("gaming", None));
    assert!(rule.matches("GAMing laptop", None));
    assert!(!rule.matches("laptop gaming", None));
    assert!(!rule.matches("laptop", None));
}

#[test]
fn test_anchoring_ends_with() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "top".to_string(),
            anchoring: Anchoring::EndsWith,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("laptop", None));
    assert!(rule.matches("gaming LAPTOP", None));
    assert!(!rule.matches("laptop gaming", None));
    assert!(!rule.matches("lapto", None));
}

#[test]
fn test_anchoring_contains() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "lap".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("laptop", None));
    assert!(rule.matches("gaming LAPTOP", None));
    assert!(rule.matches("overlap", None));
    assert!(!rule.matches("computer", None));
}

#[test]
fn test_disabled_rule_ignored() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: Some(false),
        validity: None,
    };

    assert!(!rule.matches("laptop", None));
}

#[test]
fn test_expired_rule_ignored() {
    use flapjack::index::rules::TimeRange;

    let past = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        - 3600;

    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: Some(vec![TimeRange {
            from: past - 3600,
            until: past,
        }]),
    };

    assert!(!rule.matches("laptop", None));
}

#[test]
fn test_valid_rule_within_timerange() {
    use flapjack::index::rules::TimeRange;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: Some(vec![TimeRange {
            from: now - 3600,
            until: now + 3600,
        }]),
    };

    assert!(rule.matches("laptop", None));
}

#[test]
fn test_context_matching() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: Some("mobile".to_string()),
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("laptop", Some("mobile")));
    assert!(!rule.matches("laptop", Some("desktop")));
    assert!(!rule.matches("laptop", None));
}

#[test]
fn test_multi_condition_any_match() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![
            Condition {
                pattern: "laptop".to_string(),
                anchoring: Anchoring::Contains,
                alternatives: None,
                context: None,
                filters: None,
            },
            Condition {
                pattern: "computer".to_string(),
                anchoring: Anchoring::Contains,
                alternatives: None,
                context: None,
                filters: None,
            },
        ],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("laptop", None));
    assert!(rule.matches("computer", None));
    assert!(!rule.matches("phone", None));
}

#[test]
fn test_hide_non_existent_id() {
    let mut store = RuleStore::new();
    store.insert(Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: Some(vec![Hide {
                object_id: "nonexistent".to_string(),
            }]),
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    let effects = store.apply_rules("laptop", None);
    assert_eq!(effects.hidden.len(), 1);
    assert_eq!(effects.hidden[0], "nonexistent");
}

#[test]
fn test_multiple_pins_same_position() {
    let mut store = RuleStore::new();

    store.insert(Rule {
        object_id: "rule1".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: Some(vec![Promote::Single {
                object_id: "item1".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    store.insert(Rule {
        object_id: "rule2".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: Some(vec![Promote::Single {
                object_id: "item2".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    let effects = store.apply_rules("laptop", None);
    assert_eq!(effects.pins.len(), 2);
    assert_eq!(effects.pins[0].1, 0);
    assert_eq!(effects.pins[1].1, 0);
}

#[test]
fn test_promote_multiple_objects() {
    let mut store = RuleStore::new();
    store.insert(Rule {
        object_id: "test".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: Some(vec![Promote::Multiple {
                object_ids: vec![
                    "item1".to_string(),
                    "item2".to_string(),
                    "item3".to_string(),
                ],
                position: 5,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    let effects = store.apply_rules("laptop", None);
    assert_eq!(effects.pins.len(), 3);
    assert_eq!(effects.pins[0], ("item1".to_string(), 5));
    assert_eq!(effects.pins[1], ("item2".to_string(), 6));
    assert_eq!(effects.pins[2], ("item3".to_string(), 7));
}

#[test]
fn test_empty_conditions_always_match() {
    let rule = Rule {
        object_id: "test".to_string(),
        conditions: vec![],
        consequence: Consequence {
            promote: Some(vec![Promote::Single {
                object_id: "item1".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("anything", None));
    assert!(rule.matches("", None));
}
#[test]
fn test_hide_and_pin_interaction() {
    let mut store = RuleStore::new();

    store.insert(Rule {
        object_id: "rule1".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: Some(vec![Promote::Single {
                object_id: "item1".to_string(),
                position: 0,
            }]),
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    store.insert(Rule {
        object_id: "rule2".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: Some(vec![Hide {
                object_id: "item1".to_string(),
            }]),
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    let effects = store.apply_rules("laptop", None);

    assert_eq!(effects.pins.len(), 1);
    assert_eq!(effects.hidden.len(), 1);
    assert_eq!(effects.pins[0].0, "item1");
    assert_eq!(effects.hidden[0], "item1");
}

#[test]
fn test_multi_consequence_single_rule() {
    let mut store = RuleStore::new();

    store.insert(Rule {
        object_id: "multi".to_string(),
        conditions: vec![Condition {
            pattern: "laptop".to_string(),
            anchoring: Anchoring::Contains,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: Some(vec![Promote::Single {
                object_id: "promoted".to_string(),
                position: 0,
            }]),
            hide: Some(vec![Hide {
                object_id: "hidden".to_string(),
            }]),
            filter_promotes: None,
            user_data: Some(serde_json::json!({"banner": "sale"})),
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    });

    let effects = store.apply_rules("laptop", None);

    assert_eq!(effects.pins.len(), 1);
    assert_eq!(effects.hidden.len(), 1);
    assert_eq!(effects.user_data.len(), 1);
    assert_eq!(effects.pins[0].0, "promoted");
    assert_eq!(effects.hidden[0], "hidden");
}

#[test]
fn test_pattern_empty_string_with_is_anchoring() {
    let rule = Rule {
        object_id: "empty".to_string(),
        conditions: vec![Condition {
            pattern: "".to_string(),
            anchoring: Anchoring::Is,
            alternatives: None,
            context: None,
            filters: None,
        }],
        consequence: Consequence {
            promote: None,
            hide: None,
            filter_promotes: None,
            user_data: None,
            params: None,
        },
        description: None,
        enabled: None,
        validity: None,
    };

    assert!(rule.matches("", None));
    assert!(!rule.matches("anything", None));
}
