extern crate time;
extern crate jsonwebtoken as jwt;
extern crate rustc_serialize;

use self::jwt::{decode, Algorithm};

#[derive(Debug, Clone, Copy, RustcEncodable, RustcDecodable)]
struct Claims {
    exp: i64
}

impl Claims {
    fn is_expired(self) -> bool {
        let timespec = time::get_time();

        self.exp < timespec.sec
    }
}

pub fn authenticate(token: &str, secret: &str) -> bool {
    match decode::<Claims>(&token, secret.as_ref(), Algorithm::HS256) {
        Ok(res) => {
            if res.claims.is_expired() {
                debug!("JWT Token Expired; exp={:?}", res.claims);

                false

            } else {
                true
            }
        }

        Err(e) => {
            debug!("JWT Auth Failed; err={:?}", e);

            false
        }
    }
}
