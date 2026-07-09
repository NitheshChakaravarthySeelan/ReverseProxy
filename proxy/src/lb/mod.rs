use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;

pub struct BackendPool {
    backends: Vec<SocketAddr>,
    counter: AtomicUsize,
}

impl BackendPool {
    pub fn new(backends: Vec<SocketAddr>) -> Self {
        assert!(!backends.is_empty(), "BackendPool must have at least one backend");
        Self {
            backends,
            counter: AtomicUsize::new(0),
        }
    }

    pub fn next_backend(&self) -> SocketAddr {
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        self.backends[index]
    }

    pub fn backends(&self) -> &[SocketAddr] {
        &self.backends
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
