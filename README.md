# derive_aktor
A macro to generate Rust actors

This library is in progress, there may be breaking changes.

`derive_aktor` provides a macro that turns your normal synchronous rust code into actor oriented async code, with
just a line of code. You can think of actors sort of like Queues that hold state and, in the case of this library,
provide a richer API than just `send` and `recv`.

Here's a minimal example, we'll write a "CounterLogger" that is asynchronous.
```rust
#[derive(Default)]
pub struct CountLogger {
    count: u32
}

#[derive_actor]
impl CountLogger {
    pub fn count(&mut self) {
        self.count += 1;
        println!("[COUNT] - {}", self.count);
    }
}

fn main() {
    let mut rt: Runtime = Runtime::new().unwrap();

    rt.spawn(lazy(|| -> Result<(), ()> {
        let count_actor = CountLoggerActor::new(Default::default());
        for i in 0..10 {
            count_actor.count(i);
        }

        Ok(())
    }));

    rt.shutdown_on_idle().wait().unwrap();
}
```

Outside of some boilerplate to set up the runtime, there is no need to think about sync vs async. We wrote code
exactly how we would in a synchronous context, and the async code is generated for it.

Actors can also have generic methods or state.

```rust
#[derive(Default)]
pub struct CountLogger {
    count: u32
}

#[derive_actor]
impl CountLogger {
    pub fn count<T: Display + Send + 'static>(&mut self, msg: T) {
        self.count += 1;
        println!("[COUNT] - {} - {}", self.count, msg);
    }
}

fn main() {
    let mut rt: Runtime = Runtime::new().unwrap();

    rt.spawn(lazy(|| -> Result<(), ()> {
        let count_actor = CountLoggerActor::new(Default::default());
        for i in 0..10 {
            count_actor.count(i, "some message");
        }

        Ok(())
    }));

    rt.shutdown_on_idle().wait().unwrap();
}
```

