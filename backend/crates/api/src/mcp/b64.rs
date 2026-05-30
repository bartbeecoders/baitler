//! Minimal standard-alphabet Base64 (RFC 4648), dependency-free.
//!
//! MCP tool results travel as JSON text, so binary payloads (file contents,
//! exported PDFs/DOCX) are Base64-encoded. The standard library has no Base64,
//! and pulling a crate in just for this is overkill — this module is small and
//! fully unit-tested.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes to a padded Base64 string.
pub fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Decode a Base64 string, ignoring ASCII whitespace and stopping at padding.
/// Returns an error on any other invalid character.
pub fn decode(input: &str) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for c in input.bytes() {
        let val: u32 = match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err("invalid base64 character".to_string()),
        };
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            buf.push((acc >> bits) as u8);
        }
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn decodes_known_vectors() {
        assert_eq!(decode("").unwrap(), b"");
        assert_eq!(decode("Zg==").unwrap(), b"f");
        assert_eq!(decode("Zm8=").unwrap(), b"fo");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn decode_ignores_whitespace() {
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode("@@@@").is_err());
    }

    #[test]
    fn round_trips_arbitrary_bytes() {
        let data: Vec<u8> = (0..=255u8).collect();
        assert_eq!(decode(&encode(&data)).unwrap(), data);
    }
}
