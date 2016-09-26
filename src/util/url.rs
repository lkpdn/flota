use rustc_serialize::Encodable;
use rustc_serialize::Encoder;
use std::ops::Deref;
use url;
use url::Url as U;

#[derive(Debug, Clone)]
pub struct Url(U);

impl Encodable for Url {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_str(self.0.as_str())
    }
}

impl Deref for Url {
    type Target = U;
    fn deref(&self) -> &U {
        &self.0
    }
}

impl Url {
    pub fn parse(input: &str) -> Result<Url, url::ParseError> {
        U::parse(input).map(|u| Url(u))
    }
}
