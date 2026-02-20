use std::collections::HashMap;

#[derive(Debug)]
pub enum ParseError {
    InvalidHeader,
    // Add more specific error types as needed
}

#[derive(Debug)]
pub enum BodyKind {
    None,
    ContentLength(usize),
    Chunked,
}

pub struct RequestMeta {
    pub body_kind: BodyKind,
    pub connection_close: bool,
    pub host: Option<Vec<u8>>,
    pub method: Vec<u8>,
    pub uri: Vec<u8>,
    pub http_version: Vec<u8>,
}

pub struct HeadParser {
    cursor: usize,
    pub lines: Vec<Vec<u8>>,
}

pub enum ParseEvent<'a> {
    Line(&'a [u8]),
    End,
    NeedMore,
}

impl HeadParser {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            lines: Vec::new(),
        }
    }

    pub fn advance<'a>(&mut self, buf: &'a [u8]) -> ParseEvent<'a> {
        if self.cursor >= buf.len() {
            return ParseEvent::NeedMore;
        }

        let remaining = &buf[self.cursor..];

        if let Some(pos) = remaining.windows(2).position(|w| w == b"\r\n") {
            if pos == 0 {
                self.cursor += 2;
                return ParseEvent::End;
            }
            let line = &remaining[..pos];
            self.lines.push(line.to_vec()); // Store the line
            self.cursor += pos + 2;
            return ParseEvent::Line(line);
        }
        ParseEvent::NeedMore
    }

    pub fn parse_request_meta(&self) -> Result<RequestMeta, ParseError> {
        if self.lines.is_empty() {
            return Err(ParseError::InvalidHeader);
        }

        // Parse the request line (e.g., GET /index.html HTTP/1.1)
        let request_line = &self.lines[0];
        let parts: Vec<&[u8]> = request_line.splitn(3, |&b| b == b' ').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidHeader);
        }
        let method = parts[0].to_vec();
        let uri = parts[1].to_vec();
        let http_version = parts[2].to_vec();

        let mut content_length: Option<usize> = None;
        let mut is_chunked = false;
        let mut connection_close = false;
        let mut host: Option<Vec<u8>> = None;

        // Process other headers
        for line in self.lines.iter().skip(1) {
            if let Some(colon_pos) = line.iter().position(|&b| b == b':') {
                let name = &line[..colon_pos];
                let value = &line[colon_pos + 1..].trim_ascii_start(); // Trim leading whitespace

                if name.eq_ignore_ascii_case(b"Content-Length") {
                    let s = std::str::from_utf8(value).map_err(|_| ParseError::InvalidHeader)?;
                    content_length = Some(s.parse().map_err(|_| ParseError::InvalidHeader)?);
                } else if name.eq_ignore_ascii_case(b"Transfer-Encoding") {
                    if value.eq_ignore_ascii_case(b"chunked") {
                        is_chunked = true;
                    }
                } else if name.eq_ignore_ascii_case(b"Connection") {
                    if value.eq_ignore_ascii_case(b"close") {
                        connection_close = true;
                    }
                } else if name.eq_ignore_ascii_case(b"Host") {
                    host = Some(value.to_vec());
                }
            }
        }

        let body_kind = if is_chunked {
            BodyKind::Chunked
        } else if let Some(len) = content_length {
            BodyKind::ContentLength(len)
        } else {
            BodyKind::None
        };

        Ok(RequestMeta {
            body_kind,
            connection_close,
            host,
            method,
            uri,
            http_version,
        })
    }
}

/* */
