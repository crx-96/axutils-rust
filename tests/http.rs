#![cfg(feature = "http")]

use std::io::ErrorKind;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use axutils::http::{
    DeduplicationPolicy, HttpClient, HttpConfig, HttpError, HttpHeaders, HttpMethod, HttpRequest,
    HttpTransportErrorKind, RetryPolicy,
};
use axutils::utils::HttpUtils;
#[cfg(feature = "http-async")]
use tokio::time as tokio_time;

#[path = "http/support.rs"]
mod support;
use support::*;

#[path = "http/coalescing.rs"]
mod coalescing;
#[path = "http/transport.rs"]
mod transport;
