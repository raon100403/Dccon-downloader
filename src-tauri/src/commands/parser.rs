use regex::Regex;

/// Parse dccon URL or package_idx from user input
/// Supports:
/// - Direct package_idx: "12345"
/// - URL with path: "https://dccon.dcinside.com/detail/12345"
/// - URL with query: "https://dccon.dcinside.com/index/package_detail?no=12345"
#[tauri::command]
pub fn parse_dccon_url(input: String) -> Result<String, String> {
    let input = input.trim();
    
    // If the input is purely numeric, return it directly
    if input.chars().all(|c| c.is_ascii_digit()) {
        return Ok(input.to_string());
    }
    
    // Try to extract numbers from URL
    // Pattern 1: /detail/{number} or /package/{number}
    let detail_regex = Regex::new(r"/(?:detail|package)/(\d+)")
        .map_err(|e| format!("Regex error: {}", e))?;
    
    if let Some(caps) = detail_regex.captures(input) {
        if let Some(matched) = caps.get(1) {
            return Ok(matched.as_str().to_string());
        }
    }
    
    // Pattern 2: ?no={number} or ?package_idx={number} or ?idx={number}
    let query_regex = Regex::new(r"[?&](?:no|package_idx|idx)=(\d+)")
        .map_err(|e| format!("Regex error: {}", e))?;
    
    if let Some(caps) = query_regex.captures(input) {
        if let Some(matched) = caps.get(1) {
            return Ok(matched.as_str().to_string());
        }
    }
    
    // Pattern 3: Any consecutive digits in the URL (last resort)
    let number_regex = Regex::new(r"\d+")
        .map_err(|e| format!("Regex error: {}", e))?;
    
    if let Some(matched) = number_regex.find(input) {
        return Ok(matched.as_str().to_string());
    }
    
    Err(format!("유효한 디시콘 번호를 찾을 수 없습니다: {}", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pure_number() {
        assert_eq!(parse_dccon_url("12345".to_string()).unwrap(), "12345");
        assert_eq!(parse_dccon_url("  67890  ".to_string()).unwrap(), "67890");
    }

    #[test]
    fn test_parse_detail_url() {
        let url = "https://dccon.dcinside.com/detail/12345";
        assert_eq!(parse_dccon_url(url.to_string()).unwrap(), "12345");
    }

    #[test]
    fn test_parse_package_url() {
        let url = "https://dccon.dcinside.com/package/98765";
        assert_eq!(parse_dccon_url(url.to_string()).unwrap(), "98765");
    }

    #[test]
    fn test_parse_query_no() {
        let url = "https://dccon.dcinside.com/index/package_detail?no=54321";
        assert_eq!(parse_dccon_url(url.to_string()).unwrap(), "54321");
    }

    #[test]
    fn test_parse_query_package_idx() {
        let url = "https://dccon.dcinside.com/api?package_idx=11111";
        assert_eq!(parse_dccon_url(url.to_string()).unwrap(), "11111");
    }

    #[test]
    fn test_invalid_input() {
        let result = parse_dccon_url("https://example.com/".to_string());
        assert!(result.is_err());
    }
}
