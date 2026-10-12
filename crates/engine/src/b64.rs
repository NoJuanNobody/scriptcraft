//! Base64 (RFC 4648, standard alphabet, padded) for passing file bytes through JSON.

const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c.first().copied().unwrap_or(0), c.get(1).copied().unwrap_or(0), c.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].iter().enumerate() {
            if i <= c.len() {
                out.push(char::from(T[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decode; whitespace is ignored, anything else invalid is an error.
pub fn decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err(format!("invalid base64 character `{}`", c as char)),
        } as u32)
    };
    let clean: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let body: Vec<u8> = clean.iter().copied().take_while(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * i as u32);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        match chunk.len() {
            4 => out.extend_from_slice(&bytes),
            3 => out.extend_from_slice(&bytes[..2]),
            2 => out.push(bytes[0]),
            _ => return Err("truncated base64".into()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        for s in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            assert_eq!(super::decode(&super::encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(super::encode(b"foobar"), "Zm9vYmFy");
        assert!(super::decode("Zm9v!").is_err());
        assert!(super::decode("Z").is_err());
    }
}
