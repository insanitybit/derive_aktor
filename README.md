# derive_aktor
A macro to generate Rust actors

`derive_aktor` exports a `derive_actor` macro. This macro can take the impl of a struct and generate
an actor for it, where the actor has a nomal, typed API roughly identical to that of your impl.

This makes it easy to write typed, nominal, asynchronous APIs.

## Example

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

    // We must drop any references to kv_store before we await the handle, or it will leak!

    drop(kv_store);
    handle.await;
}

```

### Implementation

Every actor is a spawned tokio task, with a channel that's polled in a loop. Messages are passed in via
the generated API on the Actor, routed through the channel, destructured, and routed to your underlying
struct's methods.

Your struct is also provided with a `self_actor` field (it must be of type Option<ActorType>), which you can
use to send your struct messages from within its own impl.

### State

I'm not great with proc macros, so contributions welcome. Here are a few open issues:

[] The generics on the impl block must not use a where clause
[] Generics on the Actor are the sum of all generics that appear in your actor struct *and* method, which
   is unnecessary. It would be possible to generate an Actor that only lifts the generics that actually
   correspond to method arguments.
[] Even if you never reference your self_actor it's still there, which is unnecessary.