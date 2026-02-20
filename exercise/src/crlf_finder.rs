/// Find a crlf without using strings.
pub fn crlf_finder(buffer: &[u8]) -> Option<u8> {
    if let Some(pos) = buffer.windows(2).position(|w| w == b"\r\n") {
        pos
    }
}
