//! Serde HTTP 便捷 API 的共享请求构造和响应解码。

//! 基于 Serde 的 JSON 请求与响应便捷方法。

use serde::{de::DeserializeOwned, Serialize};
use url::{form_urlencoded, Url};

use super::super::options::HttpRequestOptions;
use super::super::{HttpError, HttpMethod, HttpRequest, HttpResponse};

pub(super) fn build_query_request<Q: Serialize>(
    method: HttpMethod,
    url: impl AsRef<str>,
    query: Option<Q>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    build_request(
        method,
        append_query(url.as_ref(), query)?,
        None,
        options,
        json_response,
    )
}

pub(super) fn build_body_request<B: Serialize>(
    method: HttpMethod,
    url: impl AsRef<str>,
    body: Option<B>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    let body = body
        .map(|body| serde_json::to_vec(&body).map_err(|_| HttpError::JsonSerialize))
        .transpose()?;
    build_request(
        method,
        url.as_ref().to_owned(),
        body,
        options,
        json_response,
    )
}

pub(super) fn build_request(
    method: HttpMethod,
    url: String,
    body: Option<Vec<u8>>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    let has_body = body.is_some();
    let mut request = HttpRequest::new(method, url)?;
    if let Some(body) = body {
        request = request.with_body(body)?;
    }
    if let Some(options) = options {
        request = options.apply_to_request(request)?;
    }
    if has_body && !request.headers().contains("content-type") {
        request = request.with_header("content-type", "application/json")?;
    }
    if json_response && !request.headers().contains("accept") {
        request = request.with_header("accept", "application/json")?;
    }
    Ok(request)
}

pub(super) fn append_query<Q: Serialize>(url: &str, query: Option<Q>) -> Result<String, HttpError> {
    let Some(query) = query else {
        return Ok(url.to_owned());
    };
    let encoded = serde_urlencoded::to_string(&query).map_err(|_| HttpError::QuerySerialize)?;
    if encoded.is_empty() {
        return Ok(url.to_owned());
    }

    if let Ok(mut parsed) = Url::parse(url) {
        {
            let mut pairs = parsed.query_pairs_mut();
            for (key, value) in form_urlencoded::parse(encoded.as_bytes()) {
                pairs.append_pair(&key, &value);
            }
        }
        return Ok(parsed.into());
    }

    if url.contains('#') {
        return Err(HttpError::InvalidUrl);
    }
    let separator = if url.contains('?') {
        if url.ends_with('?') || url.ends_with('&') {
            ""
        } else {
            "&"
        }
    } else {
        "?"
    };
    Ok(format!("{url}{separator}{encoded}"))
}

pub(super) fn decode_json<T: DeserializeOwned>(response: HttpResponse) -> Result<T, HttpError> {
    response.json()
}

pub(super) fn decode_bytes(response: HttpResponse) -> Result<Vec<u8>, HttpError> {
    Ok(response.into_body())
}
