#![feature(proc_macro)]
#![allow(warnings)]

/// https://github.com/insanitybit/aktors/issues/2

#[macro_use]
extern crate derive_aktor;

use std::fmt::Debug;

use derive_aktor::derive_actor;
use fibers::{Executor, ThreadPoolExecutor};

pub struct PrintLogger {

}

#[derive_actor]
impl PrintLogger {
    pub fn info<T: Debug + Send + 'static>(&self, data: T) {
        println!("{:?}", data);
    }

    pub fn error<T: Debug + Send + 'static>(&self, data: T) {
        println!("{:?}", data);
    }
}

fn main() {
    let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
    let logger = PrintLogger{};

    let log_actor = PrintLoggerActor::new(system.handle(), logger);

    log_actor.info("info log");
    log_actor.error("error!!".to_owned());

    system.run();
}

#[test]
fn it_works() {}
