use crate::types::{DcconApiResponse, DcconPackageInfo};

/// Fetch dccon package information from dcinside API
#[tauri::command]
pub async fn fetch_dccon_info(package_idx: String) -> Result<DcconPackageInfo, String> {
    let client = reqwest::Client::new();
    
    // Construct multipart form data
    let form = reqwest::multipart::Form::new()
        .text("package_idx", package_idx.clone());
    
    // Make POST request to dccon API
    let response = client
        .post("https://dccon.dcinside.com/index/package_detail")
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .header("X-Requested-With", "XMLHttpRequest")
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("API 요청 실패: {}", e))?;
    
    // Check response status
    if !response.status().is_success() {
        return Err(format!("API 응답 오류: {}", response.status()));
    }
    
    // Parse JSON response
    let api_response: DcconApiResponse = response
        .json()
        .await
        .map_err(|e| format!("JSON 파싱 실패: {}", e))?;
    
    // Convert to simplified format
    let info = DcconPackageInfo {
        title: api_response.info.title,
        package_idx: package_idx,
        image_count: api_response.detail.len(),
        images: api_response.detail,
    };
    
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_dccon_info() {
        // This test requires network access and a valid package_idx
        // Skip in CI environments
        if std::env::var("CI").is_ok() {
            return;
        }
        
        // Test with a known dccon package (you may need to update this)
        // let result = fetch_dccon_info("12345".to_string()).await;
        // This test is commented out as it requires a real package_idx
    }
}
