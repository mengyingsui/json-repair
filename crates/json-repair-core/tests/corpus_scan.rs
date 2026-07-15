#![cfg(not(miri))]

use std::fs;
use std::panic;
use std::path::Path;

#[test]
fn test_all_corpus_entries_produce_balanced_output() {
    let corpus_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus/repair");
    if !corpus_dir.exists() {
        eprintln!("corpus dir not found: {:?}", corpus_dir);
        return;
    }
    let mut i = 0u32;
    let mut bad: Option<(String, String, String)> = None;
    let mut entries: Vec<_> = fs::read_dir(&corpus_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in &entries {
        let data = fs::read(entry.path()).unwrap();
        let s = String::from_utf8(data).unwrap_or_default();
        if s.is_empty() {
            continue;
        }
        i += 1;
        let path = entry.path().display().to_string();
        let result = panic::catch_unwind(|| json_repair_core::repair_json(&s));
        match result {
            Ok(Ok(out)) => {
                // Manually check brackets (same logic as is_output_balanced)
                let mut stack: Vec<char> = Vec::new();
                let mut in_string = false;
                let mut esc = false;
                let mut is_bad = false;
                for c in out.chars() {
                    if esc {
                        esc = false;
                        continue;
                    }
                    if c == '\\' {
                        esc = true;
                        continue;
                    }
                    if c == '"' {
                        in_string = !in_string;
                        continue;
                    }
                    if in_string {
                        continue;
                    }
                    match c {
                        '{' | '[' => stack.push(c),
                        '}' if stack.pop() == Some('{') => {}
                        '}' => {
                            is_bad = true;
                            break;
                        }
                        ']' if stack.pop() == Some('[') => {}
                        ']' => {
                            is_bad = true;
                            break;
                        }
                        _ => {}
                    }
                }
                if is_bad || !stack.is_empty() {
                    let msg = format!("unbalanced brackets (stack: {:?})", stack);
                    bad = Some((path, s.clone(), msg));
                    break;
                }
            }
            Ok(Err(_)) => {} // repair returned error, OK
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown".to_string()
                };
                bad = Some((path, s.clone(), msg));
                break;
            }
        }
    }
    if let Some((path, input, msg)) = bad {
        panic!(
            "BAD corpus entry: {}\ninput (len={}): {:?}\nmsg: {}",
            path,
            input.len(),
            input,
            msg
        );
    }
    eprintln!("All {i} corpus entries processed OK");
}
