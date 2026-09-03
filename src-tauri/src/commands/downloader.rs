use crate::commands::dccon::fetch_dccon_info;
use tauri::ipc::Channel;

fn package_directory_name(title: &str, package_idx: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();
    let name = sanitized.trim().trim_matches('.').trim();

    if name.is_empty() {
        format!("dccon-{}", package_idx)
    } else {
        name.to_string()
    }
}

/// Progress information for download
#[derive(Debug, serde::Serialize, Clone)]
pub struct DownloadProgress {
    pub current: usize,
    pub total: usize,
    pub filename: String,
    pub status: String,
}

/// Download all images from a dccon package
#[tauri::command]
pub async fn download_dccon(
    package_idx: String,
    save_path: String,
    progress: Channel<DownloadProgress>,
) -> Result<String, String> {
    // Fetch dccon information
    let dccon_info = fetch_dccon_info(package_idx.clone())
        .await
        .map_err(|e| format!("디시콘 정보 조회 실패: {}", e))?;

    let total_files = dccon_info.images.len();
    let client = reqwest::Client::new();
    let package_directory = package_directory_name(&dccon_info.title, &dccon_info.package_idx);
    let package_path = std::path::Path::new(&save_path).join(package_directory);
    std::fs::create_dir_all(&package_path)
        .map_err(|e| format!("디시콘 폴더 생성 실패: {}", e))?;

    // Send initial progress
    progress
        .send(DownloadProgress {
            current: 0,
            total: total_files,
            filename: String::new(),
            status: format!("다운로드 시작... (총 {}개)", total_files),
        })
        .ok();

    // Download each image
    for (index, image) in dccon_info.images.iter().enumerate() {
        let filename = format!("{}.{}", index, image.ext);
        let file_path = package_path.join(&filename);

        // Send progress update
        progress
            .send(DownloadProgress {
                current: index + 1,
                total: total_files,
                filename: filename.clone(),
                status: format!("다운로드 중: {}/{}", index + 1, total_files),
            })
            .ok();

        // Download image
        let image_url = format!("https://dcimg5.dcinside.com/dccon.php?no={}", image.path);
        
        let response = client
            .get(&image_url)
            .header("Referer", "https://dccon.dcinside.com/")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| format!("이미지 다운로드 실패 ({}): {}", filename, e))?;

        if !response.status().is_success() {
            return Err(format!(
                "이미지 다운로드 실패 ({}): HTTP {}",
                filename,
                response.status()
            ));
        }

        // Save image to file
        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("이미지 데이터 읽기 실패 ({}): {}", filename, e))?;

        std::fs::write(&file_path, &bytes)
            .map_err(|e| format!("파일 저장 실패 ({}): {}", filename, e))?;
    }

    // Send completion progress
    progress
        .send(DownloadProgress {
            current: total_files,
            total: total_files,
            filename: String::new(),
            status: "다운로드 완료!".to_string(),
        })
        .ok();

    Ok(format!(
        "{}개의 파일이 성공적으로 다운로드되었습니다.",
        total_files
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_directory_name_sanitizes_title() {
        assert_eq!(
            package_directory_name("고양이: 귀여운 디시콘", "12345"),
            "고양이_ 귀여운 디시콘"
        );
        assert_eq!(package_directory_name("...", "12345"), "dccon-12345");
    }

    #[test]
    fn test_download_progress_serialization() {
        let progress = DownloadProgress {
            current: 5,
            total: 10,
            filename: "test.gif".to_string(),
            status: "downloading".to_string(),
        };
        
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"current\":5"));
    }
}
