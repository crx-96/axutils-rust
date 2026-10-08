//! Serde HTTP 便捷 API 的共享请求构造和响应解码。

use serde::{de::DeserializeOwned, Serialize};
use url::{form_urlencoded, Url};

use super::super::options::HttpRequestOptions;
use super::super::request;
use super::super::{HttpError, HttpMethod, HttpRequest, HttpResponse};

/// 把可选查询参数追加到 URL，再应用请求选项和可选 JSON Accept header。
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

/// 序列化可选 JSON 正文并构造请求；序列化失败只返回稳定错误，不回显输入。
pub(super) fn build_body_request<B: Serialize>(
    method: HttpMethod,
    url: impl AsRef<str>,
    body: Option<B>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    // None 保持无正文语义；Some 统一编码为 JSON 后交给请求模型的字节限制检查。
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

/// 组装便捷 API 请求，只在调用方未提供时补充 JSON 内容类型与 Accept header。
pub(super) fn build_request(
    method: HttpMethod,
    url: String,
    body: Option<Vec<u8>>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    // 请求模型校验 URL 和正文上限，选项中的重复 header 继续保留原顺序。
    let has_body = body.is_some();
    let mut request = HttpRequest::new(method, url)?;
    if let Some(body) = body {
        request = request.with_body(body)?;
    }
    if let Some(options) = options {
        request = options.apply_to_request(request)?;
    }
    // 不覆盖调用方明确设置的媒体类型，字节 API 也不隐式要求 JSON 响应。
    if has_body && !request.headers().contains("content-type") {
        request = request.with_header("content-type", "application/json")?;
    }
    if json_response && !request.headers().contains("accept") {
        request = request.with_header("accept", "application/json")?;
    }
    Ok(request)
}

/// 对原始 URL 先校验，再追加序列化 query；保留已有参数且不允许 fragment。
pub(super) fn append_query<Q: Serialize>(url: &str, query: Option<Q>) -> Result<String, HttpError> {
    // parser 会规范化甚至剔除控制字符，不能把它的输出当作原始输入已合法的证据。
    request::validate_raw_url(url)?;
    let Some(query) = query else {
        return Ok(url.to_owned());
    };
    let encoded = serde_urlencoded::to_string(&query).map_err(|_| HttpError::QuerySerialize)?;
    if encoded.is_empty() {
        return Ok(url.to_owned());
    }

    // 绝对地址用 URL API 追加编码参数，避免把 &、= 或非 ASCII 字符变为结构分隔符。
    if let Ok(mut parsed) = Url::parse(url) {
        {
            let mut pairs = parsed.query_pairs_mut();
            for (key, value) in form_urlencoded::parse(encoded.as_bytes()) {
                pairs.append_pair(&key, &value);
            }
        }
        return Ok(parsed.into());
    }

    // 相对地址尚无 base URL，按现有 query 分隔符追加；最终解析仍由请求模型负责。
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

/// 消费响应并以稳定错误边界解码 JSON；HTTP 错误状态本身不会改变解码流程。
pub(super) fn decode_json<T: DeserializeOwned>(response: HttpResponse) -> Result<T, HttpError> {
    response.json()
}

/// 消费响应并取回正文；未共享时直接移出缓冲区，共享时复制。
pub(super) fn decode_bytes(response: HttpResponse) -> Result<Vec<u8>, HttpError> {
    Ok(response.into_body())
}

#[cfg(test)]
mod tests {
    use super::{build_query_request, HttpError, HttpMethod, HttpRequest};

    #[test]
    fn regression_query_shortcuts_reject_raw_input_before_normalization() {
        for url in [
            "",
            "https://exa\nmple.com/",
            "\rhttps://example.com/",
            "https://example.com/\tpath",
        ] {
            assert!(matches!(
                HttpRequest::new(HttpMethod::Get, url),
                Err(HttpError::InvalidUrl)
            ));
            for query in [None, Some([("page", "1")])] {
                assert!(
                    matches!(
                        build_query_request(HttpMethod::Get, url, query, None, true),
                        Err(HttpError::InvalidUrl)
                    ),
                    "accepted {url:?}"
                );
            }
        }
    }
}
