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
use tracing::info;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::Debug;
use std::time::Duration;

pub struct KeyValueStore<U>
    where U: Hash + Debug + Eq + Send + 'static
{
    inner_store: HashMap<U, String>,
    self_actor: Option<KeyValueStoreActor<U>>,
}

impl<U: Hash + Debug + Eq + Send + 'static> KeyValueStore<U> {
    pub fn new() -> Self {
        Self {
            inner_store: HashMap::new(),
            self_actor: None,
        }
    }
}
#[derive_actor]
impl<U: Hash + Debug + Eq + Send + 'static> KeyValueStore<U> {
    #[tracing::instrument(skip(self, key, f))]
    pub fn query(&self, key: U, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
        info!("query");
        f(self.inner_store.get(&key).map(String::from))
    }

    #[tracing::instrument(skip(self, key, value))]
    pub fn set(&mut self, key: U, value: String) {
        info!("set");
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
    #[tracing::instrument(skip(self, key, f))]
    pub async fn query(&self, key: &'static str, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
        info!("query");
        tokio::time::delay_for(Duration::from_secs(2)).await;

        self.inner_store.query(key, f).await;
    }

    #[tracing::instrument(skip(self, key, value))]
    pub async fn set(&mut self, key: &'static str, value: String) {
        info!("set");
        self.inner_store.set(key, value).await;
    }
}

#[tokio::main]
#[tracing::instrument]
async fn main() {
    // let filter = tracing_subscriber::EnvFilter::from_default_env();
    tracing_subscriber::fmt()
        // .json()
        .with_max_level(tracing::Level::TRACE)
        // .with_env_filter(filter)
        .init();

    let (kv_store, kv_store_handle) = KeyValueStoreActor::new(KeyValueStore::new()).await;

    kv_store.query("foo", Box::new(|value| info!("before {:?}", value))).await;
    kv_store.set("foo", "bar".to_owned()).await;
    kv_store.query("foo", Box::new(|value| info!("after {:?}", value))).await;

    let (api, api_handle) = ApiWrapperActor::new(ApiWrapper::new(kv_store.clone())).await;

    api.query("baz", Box::new(|value| info!("api.baz {:?}", value))).await;
    api.query("foo", Box::new(|value| info!("api.foo {:?}", value))).await;

    drop(api);
    drop(kv_store);
    api_handle.await;
    kv_store_handle.await;
    info!("Done");
    dbg!("done");
}
