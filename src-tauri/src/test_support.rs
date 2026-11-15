use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};

pub const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff\x21\xf9\x04\x01\x00\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";

pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub delay: Duration,
}
impl Reply {
    pub fn ok(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            body,
            delay: Duration::ZERO,
        }
    }
}

pub struct TestServer {
    address: String,
    requests: Arc<Mutex<Vec<String>>>,
    peak: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}
impl TestServer {
    pub async fn start(handler: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let peak = Arc::new(AtomicUsize::new(0));
        let request_log = requests.clone();
        let peak_log = peak.clone();
        let handler = Arc::new(handler);
        let active = Arc::new(AtomicUsize::new(0));
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut socket, _)) = accepted else { break; };
                        let (handler, requests, peak, active) = (handler.clone(), request_log.clone(), peak_log.clone(), active.clone());
                        tasks.spawn(async move {
                            let mut request = Vec::new();
                            let mut buffer = [0; 2048];
                            let mut expected = None;
                            loop {
                                let count = socket.read(&mut buffer).await.unwrap_or(0);
                                if count == 0 { return; }
                                request.extend_from_slice(&buffer[..count]);
                                if expected.is_none() {
                                    if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                                        let header = String::from_utf8_lossy(&request[..end]);
                                        let length = header.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").and_then(|length| length.trim().parse::<usize>().ok())).unwrap_or(0);
                                        expected = Some(end + 4 + length);
                                    }
                                }
                                if expected.is_some_and(|length| request.len() >= length) { break; }
                            }
                            let request = String::from_utf8_lossy(&request).into_owned();
                            requests.lock().unwrap().push(request.clone());
                            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            let reply = handler(&request);
                            tokio::time::sleep(reply.delay).await;
                            let headers = format!("HTTP/1.1 {} test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nSet-Cookie: ci_c=testtoken; Path=/\r\nConnection: close\r\n\r\n", reply.status, reply.body.len());
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.write_all(&reply.body).await;
                            active.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    _ = tasks.join_next(), if !tasks.is_empty() => {}
                }
            }
        });
        Self {
            address,
            requests,
            peak,
            task,
        }
    }
    pub fn url(&self) -> String {
        self.address.clone()
    }
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}
impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct TestDirectory(pub PathBuf);
impl TestDirectory {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "dccon-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
