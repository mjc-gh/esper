extern crate time;
extern crate jsonwebtoken as jwt;
extern crate rustc_serialize;
extern crate esper;

#[cfg(test)]
mod tests {
    #[derive(Debug, RustcEncodable, RustcDecodable)]
    struct Token {
        pub exp: i64
    }

    use esper::auth::authenticate;

    #[test]
    fn invalid_token() {
        assert_eq!(false, authenticate("blah", "secret"));
    }

    #[test]
    fn token_with_bad_signature() {
        // generated using a key of "helloworld" instead
        assert_eq!(false, authenticate("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE0Njg0NDI3MzB9.Lg2RcMa2ZT4ztsmfx54fcY_XR_ELWytSDs7w_eYxT5k", "secret"));
    }

    #[test]
    fn token_thats_has_expired() {
        // generated with an exp from decades ago
        assert_eq!(false, authenticate("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOjE0Njg0MDAwMH0.ityHse4p_vRVuCaPNakAA7NbV0iMgkSGkExFGcISp68", "secret"));
    }

    #[test]
    fn valid_token() {
        use jwt::{encode, Header};
        use time::{get_time};

        let token = encode(Header::default(), &Token {
            exp: get_time().sec + 3600

        }, "secret".as_ref()).unwrap();

        assert_eq!(true, authenticate(&token, "secret"));
    }
}
