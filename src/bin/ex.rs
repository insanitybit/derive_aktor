extern crate derive_aktor;
extern crate futures;
extern crate syn;
extern crate tokio;

use std::collections::HashMap;

use std::hash::Hash;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use async_trait::async_trait;
use derive_aktor::derive_actor;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};


pub struct KeyValueStore<U>
    where U: Hash + Eq + Send + 'static
{
    inner_store: HashMap<U, String>,
    self_actor: Option<KeyValueStoreActor<U>>,
}

impl<U: Hash + Eq + Send + 'static> KeyValueStore<U> {
    pub fn new() -> Self {
        Self {
            inner_store: HashMap::new(),
            self_actor: None,
        }
    }
}
#[derive_actor]
impl<U: Hash + Eq + Send + 'static> KeyValueStore<U> {
    pub fn query(&self, key: U, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
        println!("query");
        f(self.inner_store.get(&key).map(String::from))
    }

    pub fn set(&mut self, key: U, value: String) {
        println!("set");
        self.inner_store.insert(key, value);
    }
}


pub struct ApiWrapper
{
    inner_store: KeyValueStoreActor<&'static str>,
    self_actor: Option<ApiWrapperActor>,
}

impl ApiWrapper {
    pub fn new(inner_store: KeyValueStoreActor<&'static str>) -> Self {
        Self {
            inner_store,
            self_actor: None,
        }
    }
}

#[derive_actor]
impl ApiWrapper {
    pub async fn query(&self, key: &'static str, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
        println!("query");
        self.inner_store.query(key, f).await;
    }

    pub async fn set(&mut self, key: &'static str, value: String) {
        println!("set");
        self.inner_store.set(key, value);
    }
}

#[tokio::main]
async fn main() {

    let (kv_store, handle) = KeyValueStoreActor::new(KeyValueStore::new()).await;

    kv_store.query("foo", Box::new(|value| println!("before {:?}", value))).await;
    kv_store.set("foo", "bar".to_owned()).await;
    kv_store.query("foo", Box::new(|value| println!("after {:?}", value))).await;

    // Equivalent to 'drop', but necessary if we had never used 'kv_store'
    kv_store.release();
    handle.await;
}
