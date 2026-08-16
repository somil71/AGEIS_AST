use needle::policy::parser::PolicyParser;

#[test]
fn test_fuzz_policy_parser_garbage() {
    // Fuzz test with garbage unicode and extremely long strings
    let raw_text = "\u{0}\u{ffff}garbage".repeat(10000);
    let result = PolicyParser::parse_str(&raw_text, "fuzz-doc-1", "Fuzz Title", "1.0");
    
    // As long as it doesn't panic, the fuzzer passes.
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_fuzz_policy_parser_empty() {
    let raw_text = "";
    let result = PolicyParser::parse_str(&raw_text, "fuzz-doc-2", "Empty", "1.0");
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_fuzz_policy_parser_weird_formatting() {
    let raw_text = "## \n ### \n # \n - [ ] \n - [x] \n > ".repeat(500);
    let result = PolicyParser::parse_str(&raw_text, "fuzz-doc-3", "Weird", "1.0");
    assert!(result.is_ok() || result.is_err());
}
