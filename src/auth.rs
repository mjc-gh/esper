extern crate time;
extern crate jsonwebtoken as jwt;
extern crate rustc_serialize;

use self::jwt::{decode, Algorithm};

#[derive(Debug, RustcEncodable, RustcDecodable)]
struct Claims {
    exp: i64,
    sub: String
}

impl Claims {
    fn is_valid(self, topic_id: &str) -> bool {
        let timespec = time::get_time();

        if self.exp < timespec.sec {
            debug!("JWT expired");

            false
        } else if self.sub != topic_id {
            debug!("JWT invalid subject");

            false
        } else {
            debug!("JWT is valid");

            true
        }
    }
}

pub fn authenticate(topic_id: &str, token: &str, secret: &str) -> bool {
    match decode::<Claims>(&token, secret.as_ref(), Algorithm::HS256) {
        Ok(res) => res.claims.is_valid(topic_id),
        Err(e) => {
            debug!("JWT parse failure; err={:?}", e);

            false
        }
    }
}
