use http::{HeaderMap, HeaderValue};

pub(crate) trait GetIgnoreCase<K: ToString> {
    fn get_ignore_case(&self, key: K) -> Option<&HeaderValue>;
}

impl<K: ToString> GetIgnoreCase<K> for HeaderMap {
    fn get_ignore_case(&self, key: K) -> Option<&HeaderValue> {
        self.get(key.to_string())
            .or(self.get(key.to_string().to_lowercase()))
            .or(self.get(key.to_string().to_uppercase()))
    }
}
