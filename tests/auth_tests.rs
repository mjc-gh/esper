extern crate time;
extern crate jsonwebtoken as jwt;
extern crate rustc_serialize;
extern crate esper;

#[cfg(test)]
mod tests {
    #[derive(Debug, RustcEncodable, RustcDecodable)]
    struct Token {
        pub exp: i64,
        pub sub: String
    }

    use esper::auth::authenticate;

    #[test]
    fn invalid_token() {
        assert_eq!(false, authenticate("abcdef123", "blah", "secret"));
    }

    #[test]
    fn token_with_bad_signature() {
        // generated using a key of "helloworld" instead
        assert_eq!(false, authenticate("abcdef123", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE0Njg0NDI3MzB9.Lg2RcMa2ZT4ztsmfx54fcY_XR_ELWytSDs7w_eYxT5k", "secret"));
    }

    #[test]
    fn token_thats_has_expired() {
        // generated with an exp from decades ago
        assert_eq!(false, authenticate("abcdef123", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE0NjkwMDAwMCwic3ViIjoiYWJjZGVmMTIzIn0.-gKdtM6ZURUD6Oe_OB_zAuGFUtrzr_0iE1CBqxA2zIA", "secret"));
    }

    #[test]
    fn token_with_invalid_subject() {
        // generated for sub of "testing"
        assert_eq!(false, authenticate("abcdef123", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE0NjkwMDAwMCwic3ViIjoidGVzdGluZyJ9.MCuyF3_MKEh04vQ3I7ypJqv9K1t06HzytqQae0BPswM", "secret"));
    }

    #[test]
    fn valid_token() {
        use jwt::{encode, Header};
        use time::{get_time};

        let token = encode(Header::default(), &Token {
            exp: get_time().sec + 3600,
            sub: "abcdef123".to_owned()

        }, "secret".as_ref()).unwrap();

        assert_eq!(true, authenticate("abcdef123", &token, "secret"));
    }
}
