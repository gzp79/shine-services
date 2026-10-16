//! Unit tests of the chat dispatcher.

use super::*;
use shine_test::test;

#[test]
fn stream_ids_order_numerically_not_lexically() {
    // Lexical order would rank "10-0" before "9-0"; numeric order must not.
    assert!(parse_stream_id("10-0") > parse_stream_id("9-0"));
    assert!(parse_stream_id("100-5") < parse_stream_id("100-10"));
    assert!(parse_stream_id("5-0") == parse_stream_id("5-0"));
    // The initial sentinel is the smallest possible cursor.
    assert!(parse_stream_id(INITIAL_CURSOR) < parse_stream_id("1-0"));
}
