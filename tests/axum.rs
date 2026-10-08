#![cfg(feature = "axum")]

#[cfg(feature = "axum-tower-http")]
use axum::http::{HeaderName, Method};
use axum::{extract::Request, middleware, routing::get, Router};
#[cfg(feature = "axum-tower-http")]
use axutils::axum::AxumTimeoutStatus;
use axutils::axum::{AxumApp, AxumConfig, AxumError, AxumServer, AxumShutdownReason};
use axutils::utils::AxumUtils;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
#[cfg(feature = "axum-governor")]
use tokio::runtime::Builder as RuntimeBuilder;
#[cfg(feature = "axum-tower")]
use tokio::sync::Notify;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task as tokio_task, time as tokio_time,
};

#[path = "axum/support.rs"]
mod support;
use support::*;

#[path = "axum/lifecycle.rs"]
mod lifecycle;
#[path = "axum/middleware.rs"]
mod middleware_cases;
