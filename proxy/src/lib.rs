use std::error;

pub fn parse_header(buffer: &Vec<u8>, header_end: usize) -> [u8] {
    let headers = buffer[..header_end];
    headers
}

pub fn header_validation(header: &[u8]) -> Result<&str, error> {
    // Extract request line
    // Validate token rules
    // Validate version
    // Parse headers line by line
    // enforce uniqueness rules
    for line in header.split(|&b| b == b"\r\n") {}
}

fn one_colon_in_header(&header: [u8]) {
    // reject if
    // no colon
    // colon at pos 0
    // multiple colon before val begin
}
