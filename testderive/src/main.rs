#![feature(proc_macro)]
#![allow(warnings)]

/// https://github.com/insanitybit/aktors/issues/2
#[macro_use]
extern crate derive_aktor;

use std::fmt::Debug;

use derive_aktor::derive_actor;
use fibers::{Executor, ThreadPoolExecutor};

pub struct PrintLogger {}

#[derive_actor]
impl PrintLogger {
    pub fn info<T: Debug + Send + 'static>(&self, data: T) -> i32 {
        println!("{:?}", data);
        return 0;
    }

    pub fn error<T: Debug + Send + 'static>(&self, data: T) {
        println!("{:?}", data);
    }
}

fn main() {
    let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
    let logger = PrintLogger {};

    let log_actor = PrintLoggerActor::new(system.handle(), logger);

    let zero_future = log_actor.info("info log");
    log_actor.error("error!!".to_owned());

    std::thread::spawn(|| system.run());

    let zero = zero_future.wait().expect("zero canceled");

    assert!(zero == 0);

    println!("zero: {}", zero);
}

#[test]
fn it_works() {}
