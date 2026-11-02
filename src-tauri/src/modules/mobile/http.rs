use std::io::{BufRead, Read, Write};

pub const MAX_HEAD_BYTES: usize = 8 * 1024;
pub const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_HEADERS: usize = 64;

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn bearer_token(&self) -> Option<&str> {
        let value = self.header("authorization")?;
        let (scheme, token) = value.split_once(' ')?;
        scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum HttpError {
    Closed,
    Malformed,
    HeadTooLarge,
    BodyTooLarge,
    Unsupported,
}

impl HttpError {
    pub fn status(&self) -> u16 {
        match self {
            HttpError::Closed | HttpError::Malformed => 400,
            HttpError::HeadTooLarge => 431,
            HttpError::BodyTooLarge => 413,
            HttpError::Unsupported => 501,
        }
    }
}

pub fn read_request(reader: &mut impl BufRead) -> Result<Request, HttpError> {
    let mut head_bytes = 0;
    let request_line = read_line(reader, &mut head_bytes)?;
    if request_line.is_empty() {
        return Err(HttpError::Closed);
    }
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HttpError::Malformed);
    };
    if !version.starts_with("HTTP/1.")
        || method.is_empty()
        || !method.bytes().all(|byte| byte.is_ascii_uppercase())
        || !target.starts_with('/')
    {
        return Err(HttpError::Malformed);
    }
    let path = target.split(['?', '#']).next().unwrap_or("/").to_string();

    let mut headers = Vec::new();
    loop {
        let line = read_line(reader, &mut head_bytes)?;
        if line.is_empty() {
            break;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(HttpError::HeadTooLarge);
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::Malformed)?;
        if name.is_empty() || name.contains(' ') {
            return Err(HttpError::Malformed);
        }
        headers.push((name.to_string(), value.trim().to_string()));
    }

    let mut request = Request {
        method: method.to_string(),
        path,
        headers,
        body: Vec::new(),
    };
    match request.header("transfer-encoding") {
        None => {}
        Some(value) if value.eq_ignore_ascii_case("chunked") => {
            if request.header("content-length").is_some() {
                return Err(HttpError::Malformed);
            }
            request.body = read_chunked(reader)?;
            return Ok(request);
        }
        Some(_) => return Err(HttpError::Unsupported),
    }
    let length = match request.header("content-length") {
        Some(value) => value.parse::<usize>().map_err(|_| HttpError::Malformed)?,
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return Err(HttpError::BodyTooLarge);
    }
    request.body = vec![0; length];
    reader
        .read_exact(&mut request.body)
        .map_err(|_| HttpError::Malformed)?;
    Ok(request)
}

fn read_chunked(reader: &mut impl BufRead) -> Result<Vec<u8>, HttpError> {
    let mut body = Vec::new();
    let mut line_bytes = 0;
    loop {
        let size_line = read_line(reader, &mut line_bytes)?;
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16).map_err(|_| HttpError::Malformed)?;
        if size > MAX_BODY_BYTES - body.len() {
            return Err(HttpError::BodyTooLarge);
        }
        if size == 0 {
            while !read_line(reader, &mut line_bytes)?.is_empty() {}
            return Ok(body);
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader
            .read_exact(&mut body[start..])
            .map_err(|_| HttpError::Malformed)?;
        if !read_line(reader, &mut line_bytes)?.is_empty() {
            return Err(HttpError::Malformed);
        }
    }
}

fn read_line(reader: &mut impl BufRead, head_bytes: &mut usize) -> Result<String, HttpError> {
    let remaining = MAX_HEAD_BYTES.saturating_sub(*head_bytes);
    let mut line = Vec::new();
    reader
        .by_ref()
        .take(remaining as u64 + 1)
        .read_until(b'\n', &mut line)
        .map_err(|_| HttpError::Malformed)?;
    *head_bytes += line.len();
    if *head_bytes > MAX_HEAD_BYTES {
        return Err(HttpError::HeadTooLarge);
    }
    if line.is_empty() {
        return Ok(String::new());
    }
    if line.pop() != Some(b'\n') {
        return Err(HttpError::Malformed);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line).map_err(|_| HttpError::Malformed)
}

pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
img-src 'self' data:; manifest-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub fn write_response(
    writer: &mut impl Write,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} {}\r\n\
Content-Type: {content_type}\r\n\
Content-Length: {}\r\n\
Cache-Control: no-store\r\n\
Content-Security-Policy: {CSP}\r\n\
X-Content-Type-Options: nosniff\r\n\
X-Frame-Options: DENY\r\n\
Referrer-Policy: no-referrer\r\n\
Connection: close\r\n\r\n",
        reason(status),
        body.len(),
    );
    writer.write_all(head.as_bytes())?;
    writer.write_all(body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &[u8]) -> Result<Request, HttpError> {
        read_request(&mut std::io::BufReader::new(raw))
    }

    #[test]
    fn parses_post_with_body_and_bearer() {
        let request = parse(
            b"POST /api/list?x=1 HTTP/1.1\r\nHost: a\r\nAuthorization: Bearer abc \r\nContent-Length: 2\r\n\r\n{}",
        )
        .expect("request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/list");
        assert_eq!(request.bearer_token(), Some("abc"));
        assert_eq!(request.body, b"{}");
    }

    #[test]
    fn rejects_malformed_and_oversized_input() {
        assert_eq!(parse(b"").unwrap_err(), HttpError::Closed);
        assert_eq!(parse(b"GET\r\n\r\n").unwrap_err(), HttpError::Malformed);
        assert_eq!(
            parse(b"GET http://x/ HTTP/1.1\r\n\r\n").unwrap_err(),
            HttpError::Malformed
        );
        assert_eq!(
            parse(b"GET / HTTP/1.1\r\nBad Header: x\r\n\r\n").unwrap_err(),
            HttpError::Malformed
        );
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\n\r\n").unwrap_err(),
            HttpError::Unsupported
        );
        let big = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert_eq!(parse(big.as_bytes()).unwrap_err(), HttpError::BodyTooLarge);
        let long_header = format!(
            "GET / HTTP/1.1\r\nX: {}\r\n\r\n",
            "a".repeat(MAX_HEAD_BYTES)
        );
        assert_eq!(
            parse(long_header.as_bytes()).unwrap_err(),
            HttpError::HeadTooLarge
        );
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n{}").unwrap_err(),
            HttpError::Malformed
        );
    }

    #[test]
    fn decodes_chunked_bodies_within_limits() {
        let request = parse(
            b"POST /api/send HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4;ext=1\r\n{\"a\"\r\n2\r\n:1\r\n1\r\n}\r\n0\r\n\r\n",
        )
        .expect("chunked request");
        assert_eq!(request.body, b"{\"a\":1}");
        let oversized = format!(
            "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
            MAX_BODY_BYTES + 1
        );
        assert_eq!(
            parse(oversized.as_bytes()).unwrap_err(),
            HttpError::BodyTooLarge
        );
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 2\r\n\r\n")
                .unwrap_err(),
            HttpError::Malformed
        );
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nabc\r\n0\r\n\r\n")
                .unwrap_err(),
            HttpError::Malformed
        );
    }

    #[test]
    fn bearer_requires_the_scheme() {
        let request =
            parse(b"GET / HTTP/1.1\r\nAuthorization: Basic abc\r\n\r\n").expect("request");
        assert_eq!(request.bearer_token(), None);
    }
}
