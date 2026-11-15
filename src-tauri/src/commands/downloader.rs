use crate::{
    state::AppState,
    types::{DcconImage, DcconPackageInfo},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tauri::ipc::Channel;
use tokio::{fs, sync::watch, task::JoinSet};

const IMAGE_ENDPOINT: &str = "https://dcimg5.dcinside.com/dccon.php";
const CONCURRENCY: usize = 2;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MANIFEST_NAME: &str = ".dccon-download.json";

fn package_directory_name(title: &str, package_idx: &str) -> String {
    let sanitized: String = title
        .chars()
        .take(48)
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let name = sanitized.trim().trim_matches('.').trim();
    format!(
        "{}_{}",
        if name.is_empty() { "dccon" } else { name },
        package_idx
    )
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadProgress {
    pub current: usize,
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub filename: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct DownloadResult {
    pub cancelled: bool,
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub save_path: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    version: u8,
    package_idx: String,
    files: BTreeMap<String, SavedImage>,
}

#[derive(Serialize, Deserialize)]
struct SavedImage {
    source: String,
    size: u64,
    sha256: String,
}

fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_file_name(format!(
        "{}.part",
        path.file_name().unwrap().to_string_lossy()
    ));
    let result = async {
        fs::write(&temporary, bytes)
            .await
            .map_err(|e| format!("임시 파일 저장 실패: {e}"))?;
        fs::rename(&temporary, path)
            .await
            .map_err(|e| format!("파일 저장 실패: {e}"))
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result
}

async fn read_manifest(directory: &Path, package_idx: &str) -> Result<Manifest, String> {
    match fs::read(directory.join(MANIFEST_NAME)).await {
        Ok(bytes) => {
            let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| {
                "다운로드 기록을 읽을 수 없습니다. 다른 저장 위치를 선택하세요.".to_string()
            })?;
            if manifest.version != 1 || manifest.package_idx != package_idx {
                return Err(
                    "저장 폴더의 다운로드 기록이 일치하지 않습니다. 다른 저장 위치를 선택하세요."
                        .into(),
                );
            }
            Ok(manifest)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Manifest {
            version: 1,
            package_idx: package_idx.into(),
            ..Default::default()
        }),
        Err(e) => Err(format!("다운로드 기록 확인 실패: {e}")),
    }
}

async fn already_saved(
    directory: &Path,
    filename: &str,
    image: &DcconImage,
    manifest: &Manifest,
) -> bool {
    let Some(saved) = manifest.files.get(filename).filter(|saved| {
        saved.source == image.path && saved.size > 0 && saved.size <= MAX_IMAGE_BYTES as u64
    }) else {
        return false;
    };
    let path = directory.join(filename);
    let Ok(metadata) = fs::metadata(&path).await else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != saved.size {
        return false;
    }
    let Ok(bytes) = fs::read(&path).await else {
        return false;
    };
    checksum(&bytes) == saved.sha256
}

fn is_image(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "jpg" | "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}

async fn fetch_image(
    client: reqwest::Client,
    endpoint: String,
    image: DcconImage,
    index: usize,
) -> Result<(usize, Vec<u8>), String> {
    let mut url = reqwest::Url::parse(&endpoint).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("no", &image.path);
    let mut response = client
        .get(url)
        .header("Referer", "https://dccon.dcinside.com/")
        .send()
        .await
        .map_err(|e| format!("{}.{} 다운로드 실패: {e}", index, image.ext))?;
    if !response.status().is_success() {
        return Err(format!(
            "{}.{} 다운로드 실패: HTTP {}. 자동 재시도 없이 중단했습니다.",
            index,
            image.ext,
            response.status()
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_IMAGE_BYTES as u64)
    {
        return Err("이미지 크기가 저장 한도(20 MB)를 초과했습니다.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("이미지 데이터 수신 실패: {e}"))?
    {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES {
            return Err("이미지 크기가 저장 한도(20 MB)를 초과했습니다.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if !is_image(&bytes, &image.ext) {
        return Err(format!(
            "{}.{} 응답이 올바른 이미지 형식이 아닙니다.",
            index, image.ext
        ));
    }
    Ok((index, bytes))
}

/// Network tasks only receive images. The coordinator commits files and resume records
/// without cancellation in the middle of a write, then updates completed-file progress.
async fn run_download(
    client: &reqwest::Client,
    info: &DcconPackageInfo,
    save_path: &Path,
    mut cancel: watch::Receiver<bool>,
    report: impl Fn(DownloadProgress),
    endpoint: &str,
) -> Result<DownloadResult, String> {
    let directory = save_path.join(package_directory_name(&info.title, &info.package_idx));
    let mut result = DownloadResult {
        cancelled: false,
        total: info.images.len(),
        downloaded: 0,
        skipped: 0,
        save_path: directory.to_string_lossy().into_owned(),
    };
    let emit = |result: &DownloadResult, filename: String, status: &str| {
        report(DownloadProgress {
            current: result.downloaded + result.skipped,
            total: result.total,
            downloaded: result.downloaded,
            skipped: result.skipped,
            filename,
            status: status.into(),
        })
    };
    emit(&result, String::new(), "저장된 파일 확인 중");
    fs::create_dir_all(&directory)
        .await
        .map_err(|e| format!("저장 폴더 생성 실패: {e}"))?;
    let mut manifest = read_manifest(&directory, &info.package_idx).await?;
    let mut pending = Vec::new();
    for (index, image) in info.images.iter().enumerate() {
        if *cancel.borrow() {
            result.cancelled = true;
            return Ok(result);
        }
        let filename = format!("{index}.{}", image.ext);
        if already_saved(&directory, &filename, image, &manifest).await {
            result.skipped += 1;
            emit(&result, filename, "저장된 파일 확인 중");
        } else {
            pending.push(index);
        }
    }
    let mut pending = pending.into_iter();
    let mut tasks = JoinSet::new();
    loop {
        if *cancel.borrow() {
            result.cancelled = true;
            break;
        }
        while tasks.len() < CONCURRENCY {
            let Some(index) = pending.next() else {
                break;
            };
            tasks.spawn(fetch_image(
                client.clone(),
                endpoint.into(),
                info.images[index].clone(),
                index,
            ));
        }
        if tasks.is_empty() {
            break;
        }
        emit(&result, String::new(), "다운로드 중");
        let completed = tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    result.cancelled = true;
                    break;
                }
                continue;
            }
            completed = tasks.join_next() => completed,
        };
        let (index, bytes) = completed
            .ok_or("다운로드 작업이 종료되었습니다.")?
            .map_err(|e| format!("다운로드 작업 실패: {e}"))??;
        if *cancel.borrow() {
            result.cancelled = true;
            break;
        }
        let image = &info.images[index];
        let filename = format!("{index}.{}", image.ext);
        atomic_write(&directory.join(&filename), &bytes).await?;
        manifest.files.insert(
            filename.clone(),
            SavedImage {
                source: image.path.clone(),
                size: bytes.len() as u64,
                sha256: checksum(&bytes),
            },
        );
        let record =
            serde_json::to_vec(&manifest).map_err(|e| format!("다운로드 기록 생성 실패: {e}"))?;
        atomic_write(&directory.join(MANIFEST_NAME), &record).await?;
        result.downloaded += 1;
        emit(&result, filename, "다운로드 중");
    }
    // Dropping the set aborts outstanding HTTP tasks; file writes have already finished.
    drop(tasks);
    emit(
        &result,
        String::new(),
        if result.cancelled {
            "다운로드 중지됨"
        } else {
            "다운로드 완료"
        },
    );
    Ok(result)
}

#[tauri::command]
pub async fn download_dccon(
    package_idx: String,
    save_path: String,
    download_id: String,
    progress: Channel<DownloadProgress>,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadResult, String> {
    let info = state
        .cached(&package_idx)
        .ok_or("조회 정보가 만료되었습니다. 디시콘을 다시 조회하세요.")?;
    if save_path.trim().is_empty() {
        return Err("저장 위치를 선택하세요.".into());
    }
    let (_guard, cancel) = state.begin_download(download_id)?;
    run_download(
        &state.client,
        &info,
        &PathBuf::from(save_path),
        cancel,
        |update| {
            let _ = progress.send(update);
        },
        IMAGE_ENDPOINT,
    )
    .await
}

#[tauri::command]
pub fn cancel_download(download_id: String, state: tauri::State<'_, AppState>) {
    state.cancel_download(&download_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Reply, TestDirectory, TestServer, GIF};
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };

    fn info(count: usize) -> DcconPackageInfo {
        DcconPackageInfo {
            title: "테스트".into(),
            package_idx: "123".into(),
            image_count: count,
            images: (0..count)
                .map(|index| DcconImage {
                    path: index.to_string(),
                    ext: "gif".into(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    #[ignore = "실서버 확인 전용: DCCON_SMOKE_PACKAGE에 공개 디시콘 번호를 지정"]
    async fn live_package_and_one_image() {
        let id = std::env::var("DCCON_SMOKE_PACKAGE").expect("DCCON_SMOKE_PACKAGE is required");
        let state = AppState::new().unwrap();
        let mut package = crate::commands::dccon::fetch_package(
            &state,
            &id,
            crate::commands::dccon::PACKAGE_ENDPOINT,
        )
        .await
        .unwrap();
        let image_count = package.images.len();
        package.images.truncate(1);
        package.image_count = 1;
        let directory = TestDirectory::new();
        let (_sender, receiver) = watch::channel(false);
        let result = run_download(
            &state.client,
            &package,
            &directory.0,
            receiver.clone(),
            |_| {},
            IMAGE_ENDPOINT,
        )
        .await
        .unwrap();
        assert_eq!(result.downloaded, 1);
        let resumed = run_download(
            &state.client,
            &package,
            &directory.0,
            receiver,
            |_| {},
            IMAGE_ENDPOINT,
        )
        .await
        .unwrap();
        assert_eq!((resumed.downloaded, resumed.skipped), (0, 1));
        println!(
            "LIVE_SMOKE_OK package={} images={} saved=1 reused=1",
            id, image_count
        );
    }

    #[test]
    fn directory_names_are_distinct_and_bounded() {
        assert_eq!(package_directory_name("고양이:콘", "123"), "고양이_콘_123");
        assert_eq!(package_directory_name("...", "123"), "dccon_123");
        assert_ne!(
            package_directory_name("a:b", "123"),
            package_directory_name("a?b", "124")
        );
        assert!(package_directory_name(&"고".repeat(200), "123").len() < 255);
    }

    #[tokio::test]
    async fn resumes_after_failure_and_detects_same_size_local_corruption() {
        let fail = Arc::new(AtomicBool::new(true));
        let flag = fail.clone();
        let server = TestServer::start(move |request| {
            if request.contains("no=1") && flag.load(Ordering::SeqCst) {
                Reply {
                    status: 503,
                    body: vec![],
                    delay: Duration::from_millis(100),
                }
            } else {
                Reply::ok(GIF.to_vec())
            }
        })
        .await;
        let temp = TestDirectory::new();
        let state = AppState::new().unwrap();
        let (_sender, receiver) = watch::channel(false);
        assert!(run_download(
            &state.client,
            &info(2),
            &temp.0,
            receiver.clone(),
            |_| {},
            &server.url()
        )
        .await
        .is_err());
        assert!(temp.0.join("테스트_123/0.gif").exists());
        fail.store(false, Ordering::SeqCst);
        let second = run_download(
            &state.client,
            &info(2),
            &temp.0,
            receiver.clone(),
            |progress| {
                if progress.current == progress.total {
                    assert!(temp.0.join("테스트_123/0.gif").exists());
                    assert!(temp.0.join("테스트_123/1.gif").exists());
                }
            },
            &server.url(),
        )
        .await
        .unwrap();
        assert_eq!((second.downloaded, second.skipped), (1, 1));
        assert_eq!(
            server
                .requests()
                .iter()
                .filter(|r| r.contains("no=0"))
                .count(),
            1
        );
        fs::write(temp.0.join("테스트_123/0.gif"), vec![0; GIF.len()])
            .await
            .unwrap();
        let third = run_download(
            &state.client,
            &info(2),
            &temp.0,
            receiver,
            |_| {},
            &server.url(),
        )
        .await
        .unwrap();
        assert_eq!((third.downloaded, third.skipped), (1, 1));
        assert_eq!(
            fs::read(temp.0.join("테스트_123/0.gif")).await.unwrap(),
            GIF
        );
    }

    #[tokio::test]
    async fn bounded_parallel_download_and_zero_requests_for_completed_package() {
        let server = TestServer::start(|_| Reply {
            status: 200,
            body: GIF.to_vec(),
            delay: Duration::from_millis(30),
        })
        .await;
        let temp = TestDirectory::new();
        let state = AppState::new().unwrap();
        let (_sender, receiver) = watch::channel(false);
        let first = run_download(
            &state.client,
            &info(5),
            &temp.0,
            receiver.clone(),
            |_| {},
            &server.url(),
        )
        .await
        .unwrap();
        assert_eq!(first.downloaded, 5);
        assert_eq!(server.peak(), CONCURRENCY);
        let second = run_download(
            &state.client,
            &info(5),
            &temp.0,
            receiver,
            |_| {},
            &server.url(),
        )
        .await
        .unwrap();
        assert_eq!((second.downloaded, second.skipped), (0, 5));
        assert_eq!(server.requests().len(), 5);
    }

    #[tokio::test]
    async fn cancellation_interrupts_stalled_request() {
        let server = TestServer::start(|_| Reply {
            status: 200,
            body: GIF.to_vec(),
            delay: Duration::from_secs(3),
        })
        .await;
        let temp = TestDirectory::new();
        let state = AppState::new().unwrap();
        let (sender, receiver) = watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            sender.send_replace(true);
        });
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_download(
                &state.client,
                &info(2),
                &temp.0,
                receiver,
                |_| {},
                &server.url(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(result.cancelled);
        assert_eq!(result.downloaded, 0);
        assert!(!temp.0.join("테스트_123/0.gif").exists());
    }

    #[tokio::test]
    async fn rate_limit_and_html_response_stop_without_saving_or_retrying() {
        for status in [429, 200] {
            let server = TestServer::start(move |_| Reply {
                status,
                body: b"<html>access denied</html>".to_vec(),
                delay: Duration::ZERO,
            })
            .await;
            let temp = TestDirectory::new();
            let state = AppState::new().unwrap();
            let (_sender, receiver) = watch::channel(false);
            assert!(run_download(
                &state.client,
                &info(1),
                &temp.0,
                receiver,
                |_| {},
                &server.url()
            )
            .await
            .is_err());
            assert_eq!(server.requests().len(), 1);
            assert!(!temp.0.join("테스트_123/0.gif").exists());
        }
    }

    #[tokio::test]
    async fn changed_image_source_is_downloaded_again() {
        let server = TestServer::start(|_| Reply::ok(GIF.to_vec())).await;
        let temp = TestDirectory::new();
        let state = AppState::new().unwrap();
        let (_sender, receiver) = watch::channel(false);
        let mut package = info(1);
        run_download(
            &state.client,
            &package,
            &temp.0,
            receiver.clone(),
            |_| {},
            &server.url(),
        )
        .await
        .unwrap();
        package.images[0].path = "replacement".into();
        let result = run_download(
            &state.client,
            &package,
            &temp.0,
            receiver,
            |_| {},
            &server.url(),
        )
        .await
        .unwrap();
        assert_eq!((result.downloaded, result.skipped), (1, 0));
        assert_eq!(server.requests().len(), 2);
    }

    #[tokio::test]
    async fn request_timeout_stops_slow_server() {
        let server = TestServer::start(|_| Reply {
            status: 200,
            body: GIF.to_vec(),
            delay: Duration::from_secs(3),
        })
        .await;
        let temp = TestDirectory::new();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();
        let (_sender, receiver) = watch::channel(false);
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            run_download(&client, &info(1), &temp.0, receiver, |_| {}, &server.url()),
        )
        .await
        .unwrap();
        assert!(result.is_err());
    }
}
