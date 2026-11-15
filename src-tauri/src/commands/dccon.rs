use super::parser::normalize_package_id;
use crate::{
    state::AppState,
    types::{DcconApiResponse, DcconPackageInfo},
};
use reqwest::cookie::CookieStore;
use std::time::{Duration, Instant};

pub const PACKAGE_ENDPOINT: &str = "https://dccon.dcinside.com/index/package_detail";

// The public detail API expects the same anonymous session and CSRF pair as its page.
// Keep cookies in memory and serialize session initialization across lookups.
async fn anonymous_session(state: &AppState, page: &reqwest::Url) -> Result<String, String> {
    let token = || {
        state.cookies.cookies(page).and_then(|header| {
            header.to_str().ok()?.split(';').find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix("ci_c=")
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
        })
    };
    let mut initialized = state.session.lock().await;
    if initialized.is_some_and(|time| time.elapsed() < Duration::from_secs(10 * 60)) {
        if let Some(token) = token() {
            return Ok(token);
        }
    }
    let response = state
        .client
        .get(page.clone())
        .send()
        .await
        .map_err(|e| format!("디시콘 페이지 연결 실패: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "디시콘 페이지 연결 실패: HTTP {}",
            response.status()
        ));
    }
    response
        .bytes()
        .await
        .map_err(|e| format!("디시콘 페이지 수신 실패: {e}"))?;
    let token = token().ok_or("디시콘 조회 세션을 만들 수 없습니다. 잠시 후 다시 조회하세요.")?;
    *initialized = Some(Instant::now());
    Ok(token)
}

#[tauri::command]
pub async fn fetch_dccon_info(
    package_idx: String,
    state: tauri::State<'_, AppState>,
) -> Result<DcconPackageInfo, String> {
    fetch_package(&state, &package_idx, PACKAGE_ENDPOINT).await
}

pub(crate) async fn fetch_package(
    state: &AppState,
    package_idx: &str,
    endpoint: &str,
) -> Result<DcconPackageInfo, String> {
    let package_idx = normalize_package_id(package_idx)?;
    if let Some(info) = state.cached(&package_idx) {
        return Ok(info);
    }
    let page = reqwest::Url::parse(endpoint)
        .and_then(|url| url.join("/"))
        .map_err(|e| e.to_string())?;
    let csrf = anonymous_session(state, &page).await?;
    let response = state
        .client
        .post(endpoint)
        .header("Referer", page.as_str())
        .header("Origin", page.origin().ascii_serialization())
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&[("ci_t", csrf), ("package_idx", package_idx.clone())])
        .send()
        .await
        .map_err(|e| format!("정보 조회 요청 실패: {e}"))?;
    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            *state.session.lock().await = None;
        }
        return Err(format!(
            "정보 조회 실패: HTTP {}. 잠시 후 다시 조회하세요.",
            response.status()
        ));
    }
    let mut api: DcconApiResponse = response
        .json()
        .await
        .map_err(|e| format!("디시콘 정보를 읽을 수 없습니다: {e}"))?;
    if normalize_package_id(&api.info.package_idx)? != package_idx || api.detail.is_empty() {
        return Err("디시콘 정보가 올바르지 않거나 다운로드할 이미지가 없습니다.".into());
    }
    for image in &mut api.detail {
        image.ext.make_ascii_lowercase();
        if image.path.is_empty()
            || !matches!(image.ext.as_str(), "png" | "gif" | "jpg" | "jpeg" | "webp")
        {
            return Err("지원하지 않는 이미지 정보가 포함되어 있습니다.".into());
        }
    }
    let info = DcconPackageInfo {
        title: api.info.title,
        package_idx,
        image_count: api.detail.len(),
        images: api.detail,
    };
    state.remember(info.clone());
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Reply, TestServer};

    #[tokio::test]
    async fn repeated_lookup_uses_cached_response() {
        let server = TestServer::start(|_| Reply::ok(br#"{"info":{"title":"test","package_idx":"123"},"detail":[{"path":"abc","ext":"GIF"}]}"#.to_vec())).await;
        let state = AppState::new().unwrap();
        let first = fetch_package(&state, "123", &server.url()).await.unwrap();
        let second = fetch_package(&state, "00123", &server.url()).await.unwrap();
        assert_eq!(first.images[0].ext, "gif");
        assert_eq!(second.package_idx, "123");
        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET "));
        assert!(requests[1].starts_with("POST "));
        assert!(requests[1].contains("ci_t=testtoken"));
        assert!(requests[1].contains("ci_c=testtoken"));
    }

    #[tokio::test]
    async fn invalid_package_is_not_cached() {
        let server = TestServer::start(|_| {
            Reply::ok(br#"{"info":{"title":"test","package_idx":"124"},"detail":[]}"#.to_vec())
        })
        .await;
        let state = AppState::new().unwrap();
        assert!(fetch_package(&state, "123", &server.url()).await.is_err());
        assert!(state.cached("123").is_none());
    }

    #[tokio::test]
    async fn forbidden_response_invalidates_session_without_automatic_retry() {
        let server = TestServer::start(|request| {
            if request.starts_with("POST ") {
                Reply {
                    status: 403,
                    body: vec![],
                    delay: Duration::ZERO,
                }
            } else {
                Reply::ok(vec![])
            }
        })
        .await;
        let state = AppState::new().unwrap();
        assert!(fetch_package(&state, "123", &server.url()).await.is_err());
        assert_eq!(server.requests().len(), 2);
        assert!(state.session.lock().await.is_none());
        assert!(fetch_package(&state, "123", &server.url()).await.is_err());
        assert_eq!(server.requests().len(), 4);
    }
}
