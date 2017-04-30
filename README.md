# derive_aktor
A macro to generate Rust actors

This library is currently unstable, and in progress. It does not generate *any* code, it simply prints out some of the code it would generate.

The goals are laid out here: https://github.com/insanitybit/aktors/issues/2

Essentially, the goal is to allow you to write simple, synchronous rust structs and then, given only some macro annotations, generate an Actor
version of that struct. The Actor will handle dispatching events to your underlying, synchronous code. And this will all happen
using strongly typed, compile time generated interfaces.

For example:


```rust
struct Foo {
    state: u64
}

impl Foo {
    fn do_a_thing(thing: u64) {
        // do stuff
    }
}
```

and generate something like this:

```rust
enum FooActorMessage {
    DoAThing{ thing: u64 }
}

struct FooActor {
    mailbox: Queue
}

impl FooActor {
    fn do_a_thing(thing: u64) {
        let msg = FooActorMessage{ thing: thing };
        mailbox.send(msg);
    }
}

impl Foo {
    fn on_message(&self, msg: FooActorMessage) {
        match msg {
            FooActorMessage::DoAThing{ thing } => self.do_a_thing(thing),
        }
    }
}
```

You would then generate FooActor's, rather than Foo's, but you would work with a very similar interface.
