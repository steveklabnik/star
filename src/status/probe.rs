use std::sync::{Arc, RwLock};
use std::thread;

use status::StatusCache;

use hyper::client::Response;
use hyper::Client;
use hyper::error::Error;
use hyper::header::Connection;
use mio::{EventLoop, Handler};
use threadpool::ThreadPool;

pub fn start_probe_driver(targets: Vec<String>,
                          http_probe_ms: u64,
                          status_cache: Arc<RwLock<StatusCache>>) {
    println!("Starting probe driver");
    let mut event_loop = EventLoop::new().unwrap();
    let _ = event_loop.timeout_ms((), http_probe_ms);
    thread::spawn(move || {
        let _ = event_loop.run(&mut ProbeHandler {
            targets: targets,
            http_probe_ms: http_probe_ms,
            status_cache: status_cache,
            thread_pool: ThreadPool::new(4),
        });
    });
}

struct ProbeHandler {
    targets: Vec<String>,
    http_probe_ms: u64,
    status_cache: Arc<RwLock<StatusCache>>,
    thread_pool: ThreadPool,
}

impl Handler for ProbeHandler {
    type Timeout = ();
    type Message = String;

    fn timeout(&mut self,
               event_loop: &mut EventLoop<ProbeHandler>,
               _: ()) {
        println!("Probing all targets");
        let loop_channel = event_loop.channel();
        for target in self.targets.clone() {
            let _ = loop_channel.send(target);
        }
        let _ = event_loop.timeout_ms((), self.http_probe_ms);
    }

    fn notify(&mut self,
              _: &mut EventLoop<ProbeHandler>,
              target_url: String) {
        let status_cache = self.status_cache.clone();
        self.thread_pool.execute(move || {
            println!("Probing target: [{}]", target_url);

            let mut client = Client::new();

            let response: Result<Response, Error> =
                client.get(&target_url)
                    .header(Connection::close())
                    .send();

            // Obtain an exclusive write lock to the status cache.
            let mut status_cache = status_cache.write().unwrap();

            match response {
                Ok(_) => status_cache.reachable(target_url),
                Err(_) => status_cache.unreachable(target_url),
            }
        });
    }
}
