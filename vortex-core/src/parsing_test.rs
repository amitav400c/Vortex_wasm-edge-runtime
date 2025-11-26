#[cfg(test)]
mod tests {

    #[test]
    fn test_zero_copy_parsing() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);

        let status = req.parse(request).unwrap();
        assert!(status.is_complete());
        assert_eq!(req.method, Some("GET"));
        assert_eq!(req.path, Some("/"));

        // Verify we didn't allocate strings (conceptually, this test just checks correctness)
    }
}
