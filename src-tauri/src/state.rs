use crate::types::DcconPackageInfo;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;

const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_CAPACITY: usize = 16;

struct CachedPackage {
    info: DcconPackageInfo,
    fetched_at: Instant,
}

struct ActiveDownload {
    id: String,
    cancel: watch::Sender<bool>,
}

pub struct AppState {
    pub client: reqwest::Client,
    pub cookies: Arc<reqwest::cookie::Jar>,
    pub session: tokio::sync::Mutex<Option<Instant>>,
    cache: Mutex<HashMap<String, CachedPackage>>,
    active: Mutex<Option<ActiveDownload>>,
}

impl AppState {
    pub fn new() -> Result<Self, reqwest::Error> {
        let cookies = Arc::new(reqwest::cookie::Jar::default());
        Ok(Self {
            client: reqwest::Client::builder()
                .cookie_provider(cookies.clone())
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(60))
                .build()?,
            cookies,
            session: tokio::sync::Mutex::new(None),
            cache: Mutex::new(HashMap::new()),
            active: Mutex::new(None),
        })
    }

    pub fn cached(&self, id: &str) -> Option<DcconPackageInfo> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|_, entry| entry.fetched_at.elapsed() < CACHE_TTL);
        cache.get(id).map(|entry| entry.info.clone())
    }

    pub fn remember(&self, info: DcconPackageInfo) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.retain(|_, entry| entry.fetched_at.elapsed() < CACHE_TTL);
        if cache.len() >= CACHE_CAPACITY {
            if let Some(oldest) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.fetched_at)
                .map(|(id, _)| id.clone())
            {
                cache.remove(&oldest);
            }
        }
        cache.insert(
            info.package_idx.clone(),
            CachedPackage {
                info,
                fetched_at: Instant::now(),
            },
        );
    }

    pub fn begin_download(
        &self,
        id: String,
    ) -> Result<(DownloadGuard<'_>, watch::Receiver<bool>), String> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.is_some() {
            return Err("이미 다운로드가 진행 중입니다.".into());
        }
        let (cancel, receiver) = watch::channel(false);
        *active = Some(ActiveDownload { id, cancel });
        Ok((DownloadGuard(self), receiver))
    }

    pub fn cancel_download(&self, id: &str) {
        let active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(download) = active.as_ref().filter(|download| download.id == id) {
            download.cancel.send_replace(true);
        }
    }
}

pub struct DownloadGuard<'a>(&'a AppState);

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        *self.0.active.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_expires_and_stays_bounded() {
        let state = AppState::new().unwrap();
        for id in 1..=20 {
            state.remember(DcconPackageInfo {
                title: "test".into(),
                package_idx: id.to_string(),
                image_count: 0,
                images: vec![],
            });
        }
        assert_eq!(state.cache.lock().unwrap().len(), CACHE_CAPACITY);
        assert!(state.cached("1").is_none());
        assert!(state.cached("20").is_some());
        state
            .cache
            .lock()
            .unwrap()
            .get_mut("20")
            .unwrap()
            .fetched_at = Instant::now() - CACHE_TTL;
        assert!(state.cached("20").is_none());
    }

    #[test]
    fn only_current_download_can_be_cancelled_and_guard_releases_slot() {
        let state = AppState::new().unwrap();
        let (guard, receiver) = state.begin_download("first".into()).unwrap();
        assert!(state.begin_download("second".into()).is_err());
        state.cancel_download("old");
        assert!(!*receiver.borrow());
        state.cancel_download("first");
        assert!(*receiver.borrow());
        drop(guard);
        assert!(state.begin_download("second".into()).is_ok());
    }
}
