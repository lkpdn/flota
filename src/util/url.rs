use rustc_serialize::Encodable;
use rustc_serialize::Encoder;
use serde::de;
use serde::ser;
use std::ops::Deref;
use url;
use url::Url as U;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url(U);

impl Encodable for Url {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_str(self.0.as_str())
    }
}

impl ser::Serialize for Url {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: ser::Serializer {
        serializer.serialize_str(self.as_str())
    }
}

impl de::Deserialize for Url {
    fn deserialize<D>(deserializer: &mut D) -> Result<Url, D::Error>
        where D: de::Deserializer {
        deserializer.deserialize_str(UrlVisitor)
    }
}

struct UrlVisitor;

impl de::Visitor for UrlVisitor {
    type Value = Url;
    fn visit_str<E>(&mut self, v: &str) -> Result<Self::Value, E>
        where E: de::Error {
        Ok(Url::parse(v).unwrap())
    }
    fn visit_string<E>(&mut self, v: String) -> Result<Self::Value, E>
        where E: de::Error {
        Ok(Url::parse(&v).unwrap())
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

#[cfg(test)]
mod tests {
    use toml::{Encoder, Value};
    use rustc_serialize::Encodable;
    use url;
    use super::Url;

    #[derive(RustcEncodable)]
    struct TestUrlStruct { url: Url }
    #[test]
    fn test_url() {
        let mut e = Encoder::new();
        let test_url = "ftp://example.com/foo";
        let url = Url(url::Url::parse(&test_url).unwrap());
        let test_url_struct = TestUrlStruct { url: url };
        test_url_struct.encode(&mut e).unwrap();
        assert_eq!(e.toml.get(&"url".to_string()),
                   Some(&Value::String(test_url.to_string())));
    }
}
