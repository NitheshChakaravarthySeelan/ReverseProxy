use std::{collections::HashMap, io};

struct HttpRequest {
    method: String,
    uri: String,
    http_version: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
    body_start_idx: usize,
}

/// GET /api/users HTTP/1.1\r\n
/// Host: example.com\r\n
/// User-Agent: curl/8.0.1\r\n
/// Accept: */*\r\n
/// Content-Length: 5\r\n
/// \r\n
/// hello
pub fn parse_header(header: &[u8], header_end: usize) -> Result<HttpRequest, io::Error> {
    let header_string = String::from_utf8_lossy(&header.to_vec());
    let http_request_header = header_validation(&header_string, header_end: usize);
    http_request_header
}

pub fn header_validation(header: &str, header_end: usize) -> Result<HttpRequest, io::Error> {
    // Extract request line
    // Validate token rules
    // Validate version
    // Parse headers line by line
    // enforce uniqueness rules
    let mut lines = header.lines();

    // Parse request line
    let request_line = lines.next().expect("missing request line");
    let request_parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = request_parts[0].to_string();
    let uri = request_parts[1].to_string();
    let http_version = request_parts[2].to_string();

    // Parse the headers
    let mut headers: HashMap<String, String> = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(":") {
            let trimmed_key = key.trim().to_string();
            let trimmed_value = value.trim().to_string();

            /// for further checking try to check if the key is null.
            headers.insert(trimmed_key, trimmed_value);
        }
    }
    let body_start: usize = header_end + 4;
    // Store it in the struct
    let parsed_request = HttpRequest {
        method: method,
        uri: uri,
        http_version: http_version,
        headers: headers,
        body: None,
        body_start_idx: body_start,
    };
    println!("The parsed output is: {}", parsed_request);
    Ok(parsed_request)
}
