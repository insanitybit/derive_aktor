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
    where U: Hash + Debug + Eq + Send + Sync + 'static
{
    inner_store: HashMap<U, String>,
    self_actor: Option<KeyValueStoreActor<U>>,
}

impl<U: Hash + Debug + Eq + Send + Sync + 'static> KeyValueStore<U> {
    pub fn new() -> Self {
        Self {
            inner_store: HashMap::new(),
            self_actor: None,
        }
    }
}

use std::any::Any;

#[derive_actor(on_error)]
impl<U: Hash + Debug + Eq + Send + Sync + 'static> KeyValueStore<U> {
    // #[tracing::instrument(skip(self, key, f))]
    pub async fn query(&self, key: U, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
        // info!("query");
        f(self.inner_store.get(&key).map(String::from))
    }

    // #[tracing::instrument(skip(self, key, value))]
    pub fn set(&mut self, key: U, value: String) {
        // info!("set");
        self.inner_store.insert(key, value.clone());
        panic!(value);
    }

    async fn on_error(
        &mut self,
        panicked_with: Box<dyn Any + Send>,
    ) {
        if let Some(e) = panicked_with.downcast_ref::<Box<dyn std::fmt::Debug>>() {
            dbg!(e);
        }
    }
}


// pub struct ApiWrapper
// {
//     inner_store: KeyValueStoreActor<&'static str>,
//     self_actor: Option<ApiWrapperActor>,
// }
//
// impl ApiWrapper {
//     pub fn new(inner_store: KeyValueStoreActor<&'static str>) -> Self {
//         Self {
//             inner_store,
//             self_actor: None,
//         }
//     }
// }
//
// #[derive_actor]
// impl ApiWrapper {
//     // #[tracing::instrument(skip(self, key, f))]
//     pub async fn query(&self, key: &'static str, f: Box<dyn Fn(Option<String>) + Send + 'static>) {
//         // info!("query");
//         tokio::time::delay_for(Duration::from_secs(2)).await;
//
//         self.inner_store.query(key, f).await;
//     }
//
//     // #[tracing::instrument(skip(self, key, value))]
//     pub async fn set(&mut self, key: &'static str, value: String) {
//         // info!("set");
//         self.inner_store.set(key, value).await;
//     }
// }

#[tokio::main]
// #[tracing::instrument]
async fn main() {
    // let filter = tracing_subscriber::EnvFilter::from_default_env();
    // tracing_subscriber::fmt()
    //     // .json()
    //     .with_max_level(tracing::Level::TRACE)
    //     // .with_env_filter(filter)
    //     .init();

    let (kv_store, kv_store_handle) = KeyValueStoreActor::new(KeyValueStore::new()).await;

    kv_store.query("foo", Box::new(|value| info!("before {:?}", value))).await;
    kv_store.set("foo", "bar".to_owned()).await;
    kv_store.query("foo", Box::new(|value| info!("after {:?}", value))).await;
    //
    // let (api, api_handle) = ApiWrapperActor::new(ApiWrapper::new(kv_store.clone())).await;
    //
    // api.query("baz", Box::new(|value| info!("api.baz {:?}", value))).await;
    // api.query("foo", Box::new(|value| info!("api.foo {:?}", value))).await;

    // drop(api);
    // drop(kv_store);
    // api_handle.await;
    // kv_store_handle.await;
    info!("Done");
    dbg!("done");
}


#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    pub struct Ping {
        self_actor: Option<PingActor>
    }

    #[derive_actor]
    impl Ping {
        pub async fn ping(&self, pong: PongActor) {
            println!("ping");
            pong.pong(self.self_actor.clone().unwrap()).await;
        }
    }

    #[derive(Default)]
    pub struct Pong {
        self_actor: Option<PongActor>
    }

    #[derive_actor]
    impl Pong {
        pub async fn pong(&self, ping: PingActor) {
            println!("pong");
        }
    }

    #[tokio::test]
    async fn test_termination() {
        let (ping, ping_handle) = PingActor::new(Ping::default()).await;
        drop(ping);
        ping_handle.await;
    }


    // Given two actors that temporarily cycle, ensure that they eventually terminate
    #[tokio::test]
    async fn test_cycle_termination() {
        let (ping, ping_handle) = PingActor::new(Ping::default()).await;
        let (pong, pong_handle) = PongActor::new(Pong::default()).await;
        ping.ping(pong).await;
        drop(ping);
        pong_handle.await;
        ping_handle.await;
    }
}