pub fn is_valid_read_len(n: usize) -> bool {
    n != 0
}
pub fn buffer_window_end(buffer: &Vec<u8>) -> bool {
    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        return true;
    }
    return false;
}
pub mod check {
    pub use super::is_valid_read_len;
}
