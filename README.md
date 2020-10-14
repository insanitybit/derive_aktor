# derive_aktor
A macro to generate Rust actors

`derive_aktor` exports a `derive_actor` macro. This macro can take the impl of a struct and generate
an actor for it, where the actor has a nomal, typed API roughly identical to that of your impl.

This makes it easy to write typed, nominal, asynchronous APIs.

This is in contrast with many actor implementations that either:

a) Don't enforce type safety of messages
b) Expose a single API for communicating with the actor, like "send", and force you to construct the messages

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

// All methods in this block form our Actor's API
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
    
    // We can use an async API that's typed and nominal
    kv_store.query("foo", Box::new(|value| println!("before {:?}", value))).await;
    kv_store.set("foo", "bar".to_owned()).await;
    kv_store.query("foo", Box::new(|value| println!("after {:?}", value))).await;

    // We must drop any references to kv_store before we await the handle, or it will leak!

    drop(kv_store);
    handle.await;
}

```

## Example - Actor Communication
```rust

struct Logger {}

#[derive_actor]
impl Logger {
    pub fn log(&self, data: String) {info!("{}", data)}
}



struct Simple{}

#[derive_actor]
impl Simple {
    pub fn takes_actor(&self, log_actor: LoggerActor) {
        info!("Logging up here");
        log_actor.log("and logging over here".to_owned()).await;
    }
}

#[tokio::main]
async fn main() {

    let (logger, l_handle) = LoggerActor::new(Logger{}).await;
    let (simple, s_handle) = SimpleActor::new(Simple{}).await;

    simple.takes_actor(logger.clone());
    
    drop(logger);
    drop(simple);

    l_handle.await;
    s_handle.await;  // you could also join!(l_handle, s_handle);
}
```

## What is an Actor?
Actors are a concurrency primitive, similar to threads, that you communicate with through message passing.


## Implementation

### Terms
* Actor - This is the generated Actor struct
* ActorImpl - This is the struct you write, which has the impl


### Actor
The Actor is essentially just a wrapper around an mpsc Sender. The Actor provides an API based on the `impl`
block that you attach the `derive_actor` macro to.

For example,

```rust
#[derive_actor]
impl MyStruct {
    pub fn my_method(&mut self) {}
}
``` 

would generate an Actor:

```rust
struct MyStructActor {
    // ..
}

impl MyStructActor {
    // ...
    async pub fn my_method(&self) {
      // package the message, place it on the internal queue, pass it along
    }
}
```

### Actor Lifecycle Management

Actors are internally reference counted.

An Actor is freed when:
* The Actor is only referenced by itself
* The Actor has no messages in its queue

Because Rust lacks an async drop, this does mean that you'll have to explicitly drop the actor in some cases.

Further, in order to ensure that an actor completely handles all messages before your program terminates,
actor construction returns a `handle`, which you can await. This is similar to a thread API. If you don't
need to rely on the actor completing, or signal completion elsewhere, you can drop the handle.

### Error Handling
In the event that an ActorImpl panics, the error is currently *swallowed*.

### Tracing
Currently all actor methods are annotated with a tracing `instrument` annotation that will log the actor by its
unique identifier. Note that very Actor gets a new identity, even a clone of an Actor has a unique identity.

### State
I'm not great with proc macros, so contributions welcome. Here are a few open issues:

[] The generics on the impl block must not use a where clause
[] Generics on the Actor are the sum of all generics that appear in your actor struct *and* method, which
   is unnecessary. It would be possible to generate an Actor that only lifts the generics that actually
   correspond to method arguments.
[] Even if you never reference your self_actor it's still there, which is unnecessary.