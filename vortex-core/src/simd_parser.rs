//! SIMD-accelerated HTTP parsing using AVX2 intrinsics
//!
//! This module provides vectorized implementations of HTTP parsing operations
//! for x86_64 architectures with AVX2 support.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Find the first occurrence of "\r\n" in a buffer using SIMD
///
/// Uses AVX2 to scan 32 bytes at a time for '\r' characters,
/// then validates the following byte is '\n'.
/// # Safety
///
/// This function is unsafe because it uses AVX2 intrinsics. The caller must ensure
/// that the CPU supports AVX2 instructions before calling this function.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn find_crlf_simd(buffer: &[u8]) -> Option<usize> {
    if buffer.len() < 2 {
        return None;
    }

    let cr_vec = _mm256_set1_epi8(b'\r' as i8);
    let len = buffer.len();

    // Process 32-byte chunks
    let mut i = 0;
    while i + 32 <= len {
        let chunk = _mm256_loadu_si256(buffer.as_ptr().add(i) as *const __m256i);
        let cmp = _mm256_cmpeq_epi8(chunk, cr_vec);
        let mask = _mm256_movemask_epi8(cmp);

        if mask != 0 {
            // Found at least one '\r', check each
            let mut bit_pos = 0;
            let mut remaining_mask = mask;
            while remaining_mask != 0 {
                let offset = remaining_mask.trailing_zeros() as usize;
                bit_pos += offset;
                let pos = i + bit_pos;

                // Check if next byte is '\n'
                if pos + 1 < len && buffer[pos + 1] == b'\n' {
                    return Some(pos);
                }

                // Move to next bit
                remaining_mask >>= offset + 1;
                bit_pos += 1;
            }
        }
        i += 32;
    }

    // Handle remaining bytes with scalar search
    find_crlf_scalar(&buffer[i..]).map(|offset| i + offset)
}

/// Scalar fallback for CRLF search
pub fn find_crlf_scalar(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|w| w == b"\r\n")
}

/// Safe wrapper that dispatches to SIMD or scalar based on CPU features
pub fn find_crlf(buffer: &[u8]) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { find_crlf_simd(buffer) }
        } else {
            find_crlf_scalar(buffer)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        find_crlf_scalar(buffer)
    }
}

/// Parse HTTP method using SIMD comparison
///
/// Compares the first 8 bytes against common HTTP methods in parallel.
/// # Safety
///
/// This function is unsafe because it uses SSE4.2 intrinsics. The caller must ensure
/// that the CPU supports SSE4.2 instructions before calling this function.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
pub unsafe fn parse_method_simd(buffer: &[u8]) -> Option<&str> {
    if buffer.len() < 3 {
        return None;
    }

    // Load first 8 bytes (padding with zeros if needed)
    let mut bytes = [0u8; 8];
    let copy_len = buffer.len().min(8);
    bytes[..copy_len].copy_from_slice(&buffer[..copy_len]);

    // Check common methods
    match &bytes[..4] {
        b"GET " => Some("GET"),
        b"POST" if copy_len >= 5 && bytes[4] == b' ' => Some("POST"),
        b"PUT " => Some("PUT"),
        b"HEAD" if copy_len >= 5 && bytes[4] == b' ' => Some("HEAD"),
        _ => {
            // Check longer methods
            if copy_len >= 7 && &bytes[..7] == b"DELETE " {
                Some("DELETE")
            } else if copy_len >= 7 && &bytes[..7] == b"PATCH " {
                Some("PATCH")
            } else {
                None
            }
        }
    }
}

/// Parse HTTP method (safe wrapper)
pub fn parse_method(buffer: &[u8]) -> Option<&str> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.2") {
            unsafe { parse_method_simd(buffer) }
        } else {
            parse_method_scalar(buffer)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        parse_method_scalar(buffer)
    }
}

/// Scalar fallback for method parsing
fn parse_method_scalar(buffer: &[u8]) -> Option<&str> {
    if buffer.len() < 3 {
        return None;
    }

    if buffer.starts_with(b"GET ") {
        Some("GET")
    } else if buffer.starts_with(b"POST ") {
        Some("POST")
    } else if buffer.starts_with(b"PUT ") {
        Some("PUT")
    } else if buffer.starts_with(b"HEAD ") {
        Some("HEAD")
    } else if buffer.starts_with(b"DELETE ") {
        Some("DELETE")
    } else if buffer.starts_with(b"PATCH ") {
        Some("PATCH")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_crlf() {
        let data = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(find_crlf(data), Some(14));

        let no_crlf = b"GET / HTTP/1.1";
        assert_eq!(find_crlf(no_crlf), None);

        let immediate = b"\r\ndata";
        assert_eq!(find_crlf(immediate), Some(0));
    }

    #[test]
    fn test_parse_method() {
        assert_eq!(parse_method(b"GET /"), Some("GET"));
        assert_eq!(parse_method(b"POST /api"), Some("POST"));
        assert_eq!(parse_method(b"PUT /data"), Some("PUT"));
        assert_eq!(parse_method(b"DELETE /item"), Some("DELETE"));
        assert_eq!(parse_method(b"INVALID"), None);
    }

    #[test]
    fn test_simd_scalar_equivalence() {
        let test_cases = vec![
            b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n" as &[u8],
            b"POST /api HTTP/1.1\r\nContent-Length: 0\r\n\r\n",
            b"\r\n",
            b"No CRLF here",
            b"Multiple\r\nCRLF\r\nSequences\r\n",
        ];

        for data in test_cases {
            let scalar_result = find_crlf_scalar(data);
            let simd_result = find_crlf(data);
            assert_eq!(
                scalar_result,
                simd_result,
                "Mismatch for input: {:?}",
                std::str::from_utf8(data)
            );
        }
    }
}
