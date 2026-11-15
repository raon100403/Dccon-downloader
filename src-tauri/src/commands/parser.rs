pub fn normalize_package_id(input: &str) -> Result<String, String> {
    if input.is_empty() || input.len() > 20 || !input.bytes().all(|c| c.is_ascii_digit()) {
        return Err("유효한 디시콘 번호를 입력하세요.".into());
    }
    let id = input.trim_start_matches('0');
    if id.is_empty() {
        return Err("유효한 디시콘 번호를 입력하세요.".into());
    }
    Ok(id.to_string())
}

#[tauri::command]
pub fn parse_dccon_url(input: String) -> Result<String, String> {
    let input = input.trim();
    if let Ok(id) = normalize_package_id(input) {
        return Ok(id);
    }
    let url = reqwest::Url::parse(input).map_err(|_| "디시콘 페이지 주소나 번호를 입력하세요.")?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str() != Some("dccon.dcinside.com") {
        return Err("dccon.dcinside.com의 디시콘 페이지 주소를 입력하세요.".into());
    }
    if let Some(fragment) = url.fragment() {
        if let Ok(id) = normalize_package_id(fragment) {
            return Ok(id);
        }
    }
    let segments: Vec<_> = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
    if segments.len() == 2 && matches!(segments[0], "detail" | "package") {
        return normalize_package_id(segments[1]);
    }
    for (key, value) in url.query_pairs() {
        if matches!(key.as_ref(), "no" | "package_idx" | "idx") {
            return normalize_package_id(&value);
        }
    }
    Err("주소에서 디시콘 번호를 찾을 수 없습니다.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_numbers_and_urls() {
        for input in [
            "12345",
            " 0012345 ",
            "https://dccon.dcinside.com/detail/12345",
            "https://dccon.dcinside.com/#12345",
            "https://dccon.dcinside.com/package/12345/",
            "https://dccon.dcinside.com/index/package_detail?no=12345",
            "https://dccon.dcinside.com/?package_idx=12345",
            "https://dccon.dcinside.com/?idx=12345",
        ] {
            assert_eq!(parse_dccon_url(input.into()).unwrap(), "12345", "{input}");
        }
    }

    #[test]
    fn rejects_empty_unrelated_and_partially_numeric_inputs() {
        for input in [
            "",
            " ",
            "0",
            "hello123",
            "https://example.com/detail/12345",
            "https://dccon.dcinside.com/?version=2",
            "https://dccon.dcinside.com/detail/123abc",
            "https://dccon.dcinside.com/?no=12abc",
            "https://dccon.dcinside.com/detail/１２３",
            "123456789012345678901",
        ] {
            assert!(parse_dccon_url(input.into()).is_err(), "{input}");
        }
    }
}
