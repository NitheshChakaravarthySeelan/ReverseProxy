use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub struct BackendPool {
    backends: Vec<SocketAddr>,
    health: Mutex<Vec<bool>>,
    counter: AtomicUsize,
}

impl BackendPool {
    pub fn new(backends: Vec<SocketAddr>) -> Self {
        let healthy = vec![true; backends.len()];
        Self {
            backends,
            health: Mutex::new(healthy),
            counter: AtomicUsize::new(0),
        }
    }

    pub async fn next_backend(&self) -> Option<SocketAddr> {
        let health = self.health.lock().await;
        for _ in 0..self.backends.len() {
            let index = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
            if health[index] {
                return Some(self.backends[index]);
            }
        }
        None
    }

    pub async fn mark_healthy(&self, addr: SocketAddr) {
        let mut health = self.health.lock().await;
        if let Some(i) = self.backends.iter().position(|a| *a == addr) {
            health[i] = true;
        }
    }

    pub async fn mark_unhealthy(&self, addr: SocketAddr) {
        let mut health = self.health.lock().await;
        if let Some(i) = self.backends.iter().position(|a| *a == addr) {
            health[i] = false;
        }
    }

    pub async fn start_active_health_checks(self: &Arc<Self>, interval_secs: u64, health_path: &str) {
        let pool = Arc::clone(self);
        let path = health_path.to_string();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(interval_secs)).await;
                for addr in &pool.backends {
                    let ok = tokio::time::timeout(
                        Duration::from_secs(5),
                        check_backend(*addr, &path),
                    )
                    .await
                    .is_ok();
                    let mut health = pool.health.lock().await;
                    if let Some(i) = pool.backends.iter().position(|a| *a == *addr) {
                        health[i] = ok;
                    }
                }
            }
        });
    }
}

async fn check_backend(addr: SocketAddr, path: &str) -> Result<(), ()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(addr).await.map_err(|_| ())?;
    let request = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
    stream.write_all(request.as_bytes()).await.map_err(|_| ())?;
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await.map_err(|_| ())?;
    let resp = std::str::from_utf8(&buf[..n]).map_err(|_| ())?;
    if resp.contains("200 OK") || resp.contains("200 ok") {
        Ok(())
    } else {
        Err(())
    }
}

pub struct BackendConnPool {
    idle: Mutex<HashMap<SocketAddr, Vec<TcpStream>>>,
}

impl BackendConnPool {
    pub fn new() -> Self {
        Self {
            idle: Mutex::new(HashMap::new()),
        }
    }

    pub async fn borrow(&self, addr: SocketAddr) -> Option<TcpStream> {
        let mut idle = self.idle.lock().await;
        if let Some(conns) = idle.get_mut(&addr) {
            conns.pop()
        } else {
            None
        }
    }

    pub async fn return_conn(&self, addr: SocketAddr, stream: TcpStream) {
        let mut idle = self.idle.lock().await;
        idle.entry(addr).or_default().push(stream);
    }
}

pub type SharedBackendPool = Arc<BackendPool>;
pub type SharedConnPool = Arc<BackendConnPool>;
