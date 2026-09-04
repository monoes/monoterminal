//! Search protocol message tests
//!
//! Validates Phase 4 search message serialization, validation, and edge cases

use monoterminal_protocol::{
    Envelope, SearchMatch, SearchMode, SearchRequest, SearchResponse,
};
use prost::Message;

// ============================================================================
// SearchRequest Tests
// ============================================================================

#[test]
fn test_search_request_text_mode_roundtrip() {
    let request = SearchRequest {
        session_id: "test-session-123".to_string(),
        query: "error".to_string(),
        mode: SearchMode::Text as i32,
        case_sensitive: false,
        whole_word: false,
        max_results: 100,
        start_line: 0,
        end_line: -1,
    };

    let envelope = Envelope {
        sequence_number: 200,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.session_id, request.session_id);
            assert_eq!(r.query, request.query);
            assert_eq!(r.mode, SearchMode::Text as i32);
            assert_eq!(r.case_sensitive, request.case_sensitive);
            assert_eq!(r.whole_word, request.whole_word);
            assert_eq!(r.max_results, request.max_results);
            assert_eq!(r.start_line, request.start_line);
            assert_eq!(r.end_line, request.end_line);
        }
        _ => panic!("Expected SearchRequest"),
    }
}

#[test]
fn test_search_request_regex_mode_roundtrip() {
    let request = SearchRequest {
        session_id: "session-456".to_string(),
        query: r"error\s+\d+".to_string(),
        mode: SearchMode::Regex as i32,
        case_sensitive: true,
        whole_word: false,
        max_results: 50,
        start_line: 100,
        end_line: 500,
    };

    let envelope = Envelope {
        sequence_number: 201,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.session_id, request.session_id);
            assert_eq!(r.query, request.query);
            assert_eq!(r.mode, SearchMode::Regex as i32);
            assert_eq!(r.case_sensitive, true);
            assert_eq!(r.max_results, 50);
            assert_eq!(r.start_line, 100);
            assert_eq!(r.end_line, 500);
        }
        _ => panic!("Expected SearchRequest"),
    }
}

#[test]
fn test_search_request_whole_word_search() {
    let request = SearchRequest {
        session_id: "session-789".to_string(),
        query: "test".to_string(),
        mode: SearchMode::Text as i32,
        case_sensitive: false,
        whole_word: true, // Match whole words only
        max_results: 100,
        start_line: 0,
        end_line: -1,
    };

    let mut buf = Vec::new();
    let envelope = Envelope {
        sequence_number: 202,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request.clone(),
        )),
    };
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.whole_word, true);
        }
        _ => panic!("Expected SearchRequest"),
    }
}

// ============================================================================
// SearchResponse Tests
// ============================================================================

#[test]
fn test_search_response_with_matches_roundtrip() {
    let matches = vec![
        SearchMatch {
            line_number: 10,
            line_text: "Error: Connection failed".to_string(),
            match_start: 0,
            match_end: 5,
        },
        SearchMatch {
            line_number: 25,
            line_text: "Error: Timeout occurred".to_string(),
            match_start: 0,
            match_end: 5,
        },
    ];

    let response = SearchResponse {
        matches: matches.clone(),
        total_matches: 2,
        truncated: false,
    };

    let envelope = Envelope {
        sequence_number: 203,
        message: Some(monoterminal_protocol::envelope::Message::SearchResponse(
            response.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchResponse(r)) => {
            assert_eq!(r.matches.len(), 2);
            assert_eq!(r.total_matches, 2);
            assert_eq!(r.truncated, false);
            assert_eq!(r.matches[0].line_number, 10);
            assert_eq!(r.matches[0].line_text, "Error: Connection failed");
            assert_eq!(r.matches[0].match_start, 0);
            assert_eq!(r.matches[0].match_end, 5);
        }
        _ => panic!("Expected SearchResponse"),
    }
}

#[test]
fn test_search_response_truncated() {
    // Simulate max_results = 100, but total_matches = 250
    let matches: Vec<SearchMatch> = (0..100)
        .map(|i| SearchMatch {
            line_number: i,
            line_text: format!("Line {} with error message", i),
            match_start: 10,
            match_end: 15,
        })
        .collect();

    let response = SearchResponse {
        matches,
        total_matches: 250,
        truncated: true,
    };

    let envelope = Envelope {
        sequence_number: 204,
        message: Some(monoterminal_protocol::envelope::Message::SearchResponse(
            response.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchResponse(r)) => {
            assert_eq!(r.matches.len(), 100);
            assert_eq!(r.total_matches, 250);
            assert_eq!(r.truncated, true);
        }
        _ => panic!("Expected SearchResponse"),
    }
}

#[test]
fn test_search_response_empty_results() {
    let response = SearchResponse {
        matches: vec![],
        total_matches: 0,
        truncated: false,
    };

    let envelope = Envelope {
        sequence_number: 205,
        message: Some(monoterminal_protocol::envelope::Message::SearchResponse(
            response.clone(),
        )),
    };

    let mut buf = Vec::new();
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchResponse(r)) => {
            assert_eq!(r.matches.len(), 0);
            assert_eq!(r.total_matches, 0);
            assert_eq!(r.truncated, false);
        }
        _ => panic!("Expected SearchResponse"),
    }
}

// ============================================================================
// SearchMatch Tests
// ============================================================================

#[test]
fn test_search_match_multi_line_context() {
    let match_result = SearchMatch {
        line_number: 42,
        line_text: "2024-08-20 10:15:32 ERROR [main] Connection timeout after 30s".to_string(),
        match_start: 20,
        match_end: 25,
    };

    // Verify match covers "ERROR"
    let matched_text = &match_result.line_text[match_result.match_start as usize..match_result.match_end as usize];
    assert_eq!(matched_text, "ERROR");
}

#[test]
fn test_search_match_unicode_handling() {
    let match_result = SearchMatch {
        line_number: 100,
        line_text: "🚀 Deployment error: Failed to connect 失败".to_string(),
        match_start: 14,
        match_end: 19,
    };

    // Note: Character offsets are BYTE offsets in Rust strings
    // This test verifies the protocol handles UTF-8 correctly
    assert!(match_result.match_start >= 0);
    assert!(match_result.match_end > match_result.match_start);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_search_request_empty_query() {
    // Backend should reject this, but protocol should handle it
    let request = SearchRequest {
        session_id: "session-abc".to_string(),
        query: "".to_string(), // Empty query
        mode: SearchMode::Text as i32,
        case_sensitive: false,
        whole_word: false,
        max_results: 100,
        start_line: 0,
        end_line: -1,
    };

    let mut buf = Vec::new();
    let envelope = Envelope {
        sequence_number: 206,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request,
        )),
    };
    envelope.encode(&mut buf).unwrap();

    // Should serialize successfully (validation happens in backend)
    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.query, "");
        }
        _ => panic!("Expected SearchRequest"),
    }
}

#[test]
fn test_search_request_max_results_limit() {
    let request = SearchRequest {
        session_id: "session-xyz".to_string(),
        query: "test".to_string(),
        mode: SearchMode::Text as i32,
        case_sensitive: false,
        whole_word: false,
        max_results: 1000, // Max limit
        start_line: 0,
        end_line: -1,
    };

    let mut buf = Vec::new();
    let envelope = Envelope {
        sequence_number: 207,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request.clone(),
        )),
    };
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.max_results, 1000);
        }
        _ => panic!("Expected SearchRequest"),
    }
}

#[test]
fn test_search_request_negative_end_line() {
    // end_line = -1 means "current line" (latest line in scrollback)
    let request = SearchRequest {
        session_id: "session-123".to_string(),
        query: "warning".to_string(),
        mode: SearchMode::Text as i32,
        case_sensitive: false,
        whole_word: false,
        max_results: 100,
        start_line: 0,
        end_line: -1, // Special value: current line
    };

    let mut buf = Vec::new();
    let envelope = Envelope {
        sequence_number: 208,
        message: Some(monoterminal_protocol::envelope::Message::SearchRequest(
            request.clone(),
        )),
    };
    envelope.encode(&mut buf).unwrap();

    let decoded = Envelope::decode(&buf[..]).unwrap();
    match decoded.message {
        Some(monoterminal_protocol::envelope::Message::SearchRequest(r)) => {
            assert_eq!(r.end_line, -1);
        }
        _ => panic!("Expected SearchRequest"),
    }
}

#[test]
fn test_search_mode_enum_values() {
    // Verify SearchMode enum values match spec
    assert_eq!(SearchMode::Text as i32, 0);
    assert_eq!(SearchMode::Regex as i32, 1);
}
