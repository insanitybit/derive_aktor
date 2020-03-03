# derive_aktor
A macro to generate Rust actors

This library is in progress, there may be breaking changes.

`derive_aktor` provides a macro that turns your normal synchronous rust code into actor oriented async code, with
just a line of code. You can think of actors sort of like Queues that hold state and, in the case of this library,
provide a richer API than just `send` and `recv`.

Actors can also have generic methods or state.

Here's a simple example of a "KeyValueStore". We can interact with it asynchronously,
share it across threads, 
```rust
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

```

