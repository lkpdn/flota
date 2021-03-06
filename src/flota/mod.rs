use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::intrinsics;

pub trait Cypherable {
    fn label(&self) -> String {
        unsafe {
            intrinsics::type_name::<Self>()
                .split(':').last().unwrap()
                .to_string()
        }
    }
    fn cypher_ident(&self) -> String;
}

macro_rules! save_child_rel {
    ( $tx:expr, $parent:expr, $child:expr, $rel:tt ) => {{
        $tx.exec(
            format!("MERGE (p: {}) MERGE (c: {})
                     MERGE (p)-[:{}]-(c)",
                    $parent.cypher_ident(),
                    $child.cypher_ident(),
                    $rel,
            ).as_ref()
        )
    }}
}

macro_rules! save_child_ll {
    ( $tx:expr, $parent:expr, $child:expr, $rel:tt ) => {{
        try!($tx.exec(
            format!("MERGE (p: {p})
                     MERGE (c: {c})
                     MERGE (c)-[ptr:{rel}]->(p)
                     WITH p, c
                     MATCH (p)-[ptr:TAIL]->(tail:{tail})
                     WHERE NOT id(tail) = id(c)
                     DELETE ptr
                     MERGE (c)-[:PREV]->(tail)",
                    p = $parent.cypher_ident(),
                    c = $child.cypher_ident(),
                    rel = $rel,
                    tail = $child.label()
            ).as_ref()
        ));
        $tx.exec(
            format!("MATCH (p: {p})
                     MATCH (c: {c})
                     MERGE (p)-[:TAIL]->(c)",
                    p = $parent.cypher_ident(),
                    c = $child.cypher_ident()
            ).as_ref()
        ).map(|_| true)
    }}
}

macro_rules! is_tail {
    ( $parent:expr, $child:expr ) => {{
        let graph = GraphClient::connect(::NEO4J_ENDPOINT).unwrap();
        graph.cypher().exec(
            format!("MATCH (c: {})<-[:TAIL]-(p: {}) RETURN c",
                   $child.cypher_ident(),
                   $parent.cypher_ident(),
            ).as_ref()
        ).map(|r| r.rows().count() > 0)
    }}
}

pub mod config;
pub mod entity;
pub mod manager;
pub mod test;

pub fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[cfg(test)]
mod tests {
    use serde_json;
    use std::mem;
    use std::path::PathBuf;
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct TestStruct {
        pub field1: String,
    }
    impl From<Vec<u8>> for TestStruct {
        fn from(v: Vec<u8>) -> Self {
            let buf = String::from_utf8(v).unwrap();
            serde_json::from_str(&buf).unwrap()
        }
    }
    impl Storable for TestStruct {
        fn db_path() -> PathBuf {
            PathBuf::from("/tmp/test")
        }
        fn key(&self) -> Vec<u8> {
            unsafe { mem::transmute::<i64, [u8; 8]>(self.created_at).to_vec() }
        }
    }
    #[test]
    fn test_save_and_get() {
        let mut s = String::with_capacity(10240);
        for _ in 0..10240 {
            s.push_str("x");
        }
        let test1 = TestStruct { field1: s };
        test1.save();
        let records = TestStruct::get_all();
        for record in records.iter() {
            assert_eq!(*record, test1.clone());
        }
    }
}
