//! 仅绑定临时 loopback 端口的 RESP 测试服务，不依赖真实 Redis。

use std::io::{BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use axutils::RedisConfig;

pub struct RedisTestServer {
    pub address: SocketAddr,
    pub commands: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl RedisTestServer {
    /// 根据命令返回 RESP；`None` 关闭连接，空字节表示不响应。
    pub fn start(
        reply: impl Fn(&[String]) -> Option<&'static [u8]> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定测试端口");
        let address = listener.local_addr().expect("读取测试端口");
        listener.set_nonblocking(true).expect("设置非阻塞 accept");
        let stop = Arc::new(AtomicBool::new(false));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let worker_stop = Arc::clone(&stop);
        let worker_commands = Arc::clone(&commands);
        let reply = Arc::new(reply);
        let worker = thread::spawn(move || {
            let mut connections = Vec::new();
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).expect("设置阻塞测试连接");
                        stream
                            .set_read_timeout(Some(Duration::from_millis(50)))
                            .expect("设置测试读取超时");
                        stream
                            .set_write_timeout(Some(Duration::from_secs(1)))
                            .expect("设置测试写入超时");
                        let stop = Arc::clone(&worker_stop);
                        let commands = Arc::clone(&worker_commands);
                        let reply = Arc::clone(&reply);
                        connections.push(thread::spawn(move || {
                            let mut reader = BufReader::new(stream.try_clone().expect("复制连接"));
                            let mut parser = redis::Parser::new();
                            while !stop.load(Ordering::Acquire) {
                                let value = match parser.parse_value(&mut reader) {
                                    Ok(value) => value,
                                    Err(error) if error.is_timeout() => continue,
                                    Err(error) if error.is_io_error() => break,
                                    Err(error) => panic!("测试请求协议错误: {error}"),
                                };
                                let command: Vec<String> =
                                    redis::from_redis_value(value).expect("解析测试命令");
                                commands
                                    .lock()
                                    .expect("命令记录锁")
                                    .push(command[0].clone());
                                // 为 Cluster 初始化提供覆盖全部 slot 的单节点测试拓扑。
                                if command == ["CLUSTER", "SLOTS"] {
                                    let slots = format!(
                                        "*1\r\n*3\r\n:0\r\n:16383\r\n*2\r\n$9\r\n127.0.0.1\r\n:{}\r\n",
                                        address.port()
                                    );
                                    stream.write_all(slots.as_bytes()).expect("写入测试拓扑");
                                    continue;
                                }
                                let Some(response) = reply(&command) else {
                                    break;
                                };
                                if stream.write_all(response).is_err() {
                                    break; // 客户端可能已经因测试中的超时而关闭连接。
                                }
                            }
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("接收测试连接失败: {error}"),
                }
            }
            for connection in connections {
                connection.join().expect("测试连接线程");
            }
        });
        Self {
            address,
            commands,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for RedisTestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("测试服务线程");
        }
    }
}

/// 限制测试的连接、连接池等待和命令响应时间。
pub fn test_config(url: &str) -> RedisConfig {
    RedisConfig::single(url)
        .expect("测试 Redis 配置")
        .with_pool_size(1)
        .expect("测试连接池大小")
        .with_connection_timeout(Duration::from_millis(200))
        .expect("测试连接超时")
        .with_pool_checkout_timeout(Duration::from_millis(500))
        .expect("测试连接池超时")
        .with_response_timeout(Duration::from_millis(200))
        .expect("测试响应超时")
}
