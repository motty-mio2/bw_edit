use regex::Regex;

/// Extract Bitwarden item UUID from the first few lines and return the stripped content (with header line removed).
pub fn extract_and_strip_header(content: &str) -> (Option<String>, String) {
    let re_bw_id = Regex::new(
        r"(?i)\bbw(?:_id)?\s*[:=]\s*([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})\b",
    )
    .unwrap();

    let re_chezmoi = Regex::new(
        r#"(?i)bitwarden\s+"item"\s+"([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})""#,
    )
    .unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut item_id = None;
    let mut header_indices = Vec::new();

    for (idx, line) in lines.iter().take(15).enumerate() {
        if let Some(caps) = re_bw_id.captures(line) {
            if let Some(m) = caps.get(1) {
                if item_id.is_none() {
                    item_id = Some(m.as_str().to_string());
                }
                header_indices.push(idx);
            }
        } else if let Some(caps) = re_chezmoi.captures(line) {
            if let Some(m) = caps.get(1) {
                if item_id.is_none() {
                    item_id = Some(m.as_str().to_string());
                }
                header_indices.push(idx);
            }
        }
    }

    let stripped = if !header_indices.is_empty() {
        let remaining_lines: Vec<&str> = lines
            .into_iter()
            .enumerate()
            .filter_map(|(i, l)| {
                if !header_indices.contains(&i) {
                    Some(l)
                } else {
                    None
                }
            })
            .collect();
        let mut res = remaining_lines.join("\n");
        if content.ends_with('\n') && !res.is_empty() {
            res.push('\n');
        }
        res
    } else {
        content.to_string()
    };

    (item_id, stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_and_strip_hash_comment() {
        let content = "# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\nHost foo\n  HostName foo.com\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "Host foo\n  HostName foo.com\n");
    }

    #[test]
    fn test_extract_and_strip_slash_comment() {
        let content = "// bw_id: 418f2f48-2d8e-495b-bc44-b17500e9aa1b\n{\n  \"key\": \"value\"\n}\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("418f2f48-2d8e-495b-bc44-b17500e9aa1b".to_string()));
        assert_eq!(stripped, "{\n  \"key\": \"value\"\n}\n");
    }

    #[test]
    fn test_extract_and_strip_dash_comment() {
        let content = "-- bw_id: 7540fa95-a318-49a3-b140-b17500e9b87e\nreturn { theme = 'tokyonight' }\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("7540fa95-a318-49a3-b140-b17500e9b87e".to_string()));
        assert_eq!(stripped, "return { theme = 'tokyonight' }\n");
    }

    #[test]
    fn test_extract_and_strip_chezmoi_template() {
        let content = "{{ if lookPath \"bw\" }}\n{{ (bitwarden \"item\" \"e71b158e-c584-4de3-8ae1-b003011c67d4\").notes }}\n{{ end }}\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "{{ if lookPath \"bw\" }}\n{{ end }}\n");
    }

    #[test]
    fn test_extract_and_strip_duplicated_headers() {
        let content = "# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\n# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\nHost foo\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "Host foo\n");
    }

    #[test]
    fn test_extract_and_strip_none() {
        let content = "Host foo\n  HostName foo.com\n  User test\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, None);
        assert_eq!(stripped, content);
    }
}
