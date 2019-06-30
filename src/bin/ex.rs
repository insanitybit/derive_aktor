extern crate futures;
extern crate tokio;
extern crate syn;

use futures::{Future, Poll, Async, task};
use futures::lazy;
use futures::sync::mpsc::{Receiver, Sender, channel};
use futures::stream::Stream;
use futures::sink::Sink;
use tokio::runtime::Runtime;
use futures::task::Task;
use syn::export::fmt::Display;


#[derive(Default)]
pub struct CountLogger {
    count: u32
}

#[derive_aktor::derive_actor]
impl CountLogger {
    pub fn count(&mut self) {
        self.count += 1;

        println!("[COUNT] - {}", self.count);
    }
}


pub struct PrintLogger {}

#[derive_aktor::derive_actor]
impl PrintLogger {
    pub fn foo<T: Display + Send + 'static>(&self, bar: T, counter: CountLoggerActor) {
        println!("bar {}", bar);
        counter.count();
    }
}


fn main() {
    let mut rt: Runtime = Runtime::new().unwrap();

    rt.spawn(lazy(|| -> Result<(), ()> {
        let mut log_actor = PrintLoggerActor::new(PrintLogger {});
        let mut count_actor = CountLoggerActor::new(CountLogger {count: 0});

        for i in 0..10 {
            log_actor.foo(0, count_actor.clone());
        }

        Ok(())
    }));

    rt.shutdown_on_idle().wait().unwrap();
}


