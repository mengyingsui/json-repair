use json_repair_core::{fix_colon_in_key, fix_mixed_quotes};

#[test]
fn test_fix_mixed_quotes_pattern() {
    assert_eq!(fix_mixed_quotes("','key\":\""), "\",\"key\":\"");
}

#[test]
fn test_fix_colon_in_key_comma() {
    assert_eq!(fix_colon_in_key("\"key:value\","), "\"key\":\"value\",");
}

#[test]
fn test_fix_colon_in_key_brace() {
    assert_eq!(fix_colon_in_key("\"key:value\"}"), "\"key\":\"value\"}");
}
