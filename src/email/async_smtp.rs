//! 异步 SMTP 事务直接持有连接，空闲池只接收已经读到最终完成回复的连接。

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError, Weak},
    time::Duration,
};

use lettre::{
    transport::smtp::{
        authentication::{Credentials, DEFAULT_MECHANISMS},
        client::{AsyncSmtpConnection, TlsParameters},
        commands::Noop,
        extension::ClientId,
    },
    Message,
};
use tokio::{
    runtime::Handle,
    task::JoinHandle,
    time::{self, Instant},
};

use super::{
    client::{require_completion, POOL_IDLE_TIMEOUT, POOL_MAX_IDLE_CONNECTIONS},
    config::EmailSecurity,
    error::{EmailError, EmailTransportErrorKind},
};
#[cfg(feature = "tracing")]
use crate::telemetry::email as email_trace;

/// 同一账号的连接参数和空闲池。借出的物理连接只由一次发送的 future 持有。
pub(super) struct AsyncSmtp {
    /// SMTP relay 的连接目标，TLS 校验名已绑定在 `tls` 中。
    host: String,
    /// 配置中显式指定的非零端口。
    port: u16,
    /// 决定在建立连接时使用 TLS，还是先 EHLO 再强制升级。
    security: EmailSecurity,
    /// 与同步传输相同的证书验证参数。
    tls: TlsParameters,
    /// 仅在 TLS 握手成功后交给 Lettre 的 AUTH 实现。
    credentials: Credentials,
    /// 从取连接至 DATA 最终回复的总预算。
    timeout: Duration,
    /// 只在借出、归还和回收空闲连接时加锁，网络 I/O 在锁外执行。
    idle: Arc<Mutex<IdlePool>>,
}

impl AsyncSmtp {
    /// 保存已验证的参数，不建连或创建 runtime 任务。
    pub(super) fn new(
        host: String,
        port: u16,
        security: EmailSecurity,
        tls: TlsParameters,
        credentials: Credentials,
        timeout: Duration,
    ) -> Self {
        Self {
            host,
            port,
            security,
            tls,
            credentials,
            timeout,
            idle: Arc::default(),
        }
    }

    /// 发送整封邮件。截止点覆盖探活、建连、认证和全部 SMTP 命令，包括 Lettre 的错误清理。
    pub(super) async fn send(&self, message: Message) -> Result<(), EmailError> {
        // 在访问 Tokio 时钟或网络前提供稳定的缺少 runtime 错误。
        if Handle::try_current().is_err() {
            return Err(EmailError::Transport(EmailTransportErrorKind::Client));
        }
        time::timeout(self.timeout, async {
            let raw = message.formatted();
            let mut conn = self.checkout().await?;
            // 中途取消会连同裸连接一起释放。只有确认本次 DATA 结束后才转移所有权给池。
            let reply = conn
                .send(message.envelope(), &raw)
                .await
                .map_err(|err| EmailError::from_smtp(&err))?;
            require_completion(&reply)?;
            self.recycle(conn);
            Ok(())
        })
        .await
        .map_err(|_| EmailError::Transport(EmailTransportErrorKind::Timeout))?
    }

    /// 优先借用健康且未过期的连接。探活失败时允许重新建连，此时尚未提交本次邮件。
    async fn checkout(&self) -> Result<AsyncSmtpConnection, EmailError> {
        loop {
            let entry = self
                .idle
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entries
                .pop_back();
            let Some((expires, mut conn)) = entry else {
                break;
            };
            if expires > Instant::now()
                && conn
                    .command(Noop)
                    .await
                    .is_ok_and(|r| require_completion(&r).is_ok())
            {
                return Ok(conn);
            }
        }
        // 只观测脱敏后的新连接结果，不记录对端、凭据或响应内容。
        let result = self.connect().await;
        #[cfg(feature = "tracing")]
        email_trace::record_transport_init(result.as_ref().map(|_| ()));
        result
    }

    /// 按配置建立 TLS 连接，再使用 Lettre 的默认认证机制完成登录。
    async fn connect(&self) -> Result<AsyncSmtpConnection, EmailError> {
        let hello = ClientId::default();
        let implicit = (self.security == EmailSecurity::ImplicitTls).then(|| self.tls.clone());
        let mut conn = AsyncSmtpConnection::connect_tokio1(
            (self.host.as_str(), self.port),
            Some(self.timeout),
            &hello,
            implicit,
            None,
        )
        .await
        .map_err(|err| EmailError::from_smtp(&err))?;
        // STARTTLS 必须升级成功并重新 EHLO，失败直接释放连接，凭据尚未发送。
        if self.security == EmailSecurity::StartTls {
            conn.starttls(self.tls.clone(), &hello)
                .await
                .map_err(|err| EmailError::from_smtp(&err))?;
        }
        let reply = conn
            .auth(DEFAULT_MECHANISMS, &self.credentials)
            .await
            .map_err(|err| EmailError::from_smtp(&err))?;
        require_completion(&reply)?;
        Ok(conn)
    }

    /// 在容量内归还成功连接，空闲池只需一个计时任务，过期和超额都直接释放 socket。
    fn recycle(&self, conn: AsyncSmtpConnection) {
        let mut pool = self.idle.lock().unwrap_or_else(PoisonError::into_inner);
        if pool.entries.len() < POOL_MAX_IDLE_CONNECTIONS as usize {
            let expires = Instant::now() + POOL_IDLE_TIMEOUT;
            pool.entries.push_back((expires, conn));
            // 只有进入空闲状态才启动回收。任务仅持弱引用，客户端可以在 runtime 外析构。
            if pool.cleanup.is_none() {
                pool.cleanup = Some(tokio::spawn(IdlePool::expire(
                    Arc::downgrade(&self.idle),
                    expires,
                )));
            }
        }
    }
}

/// 按归还时间排列的空闲连接和仅在有空闲连接时工作的定时器。
#[derive(Default)]
struct IdlePool {
    /// 到期时间在同一把锁内计算，队首最早过期，队尾最近使用。
    entries: VecDeque<(Instant, AsyncSmtpConnection)>,
    /// 并发归还共用的回收任务。析构时直接取消，任务不执行网络协议清理。
    cleanup: Option<JoinHandle<()>>,
}

impl IdlePool {
    /// 睡到最早到期时间并释放已过期连接。等待期间不持有池或连接的强引用。
    async fn expire(pool: Weak<Mutex<Self>>, mut deadline: Instant) {
        loop {
            time::sleep_until(deadline).await;
            let Some(pool) = pool.upgrade() else { return };
            let mut pool = pool.lock().unwrap_or_else(PoisonError::into_inner);
            let now = Instant::now();
            while pool
                .entries
                .front()
                .is_some_and(|(expires, _)| *expires <= now)
            {
                pool.entries.pop_front();
            }
            // 与归还在同一临界区决定下次唤醒，池空则由下次归还重新启动任务。
            if let Some((expires, _)) = pool.entries.front() {
                deadline = *expires;
            } else {
                pool.cleanup = None;
                return;
            }
        }
    }
}

impl Drop for IdlePool {
    /// 取消现有定时器后由字段析构直接关闭空闲连接，不依赖当前 runtime 或对端 QUIT 回复。
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup.abort();
        }
    }
}
