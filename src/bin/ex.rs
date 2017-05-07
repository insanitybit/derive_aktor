#![feature(prelude_import)]
#![no_std]
#![feature(proc_macro)]
#![allow(warnings)]
#[prelude_import]
use std::prelude::v1::*;
#[macro_use]
extern crate std as std;


/// https://github.com/insanitybit/aktors/issues/2
#[macro_use]
extern crate derive_aktor;

use std::fmt::Debug;

use derive_aktor::derive_actor;
use fibers::{Executor, ThreadPoolExecutor};

pub struct PrintLogger {}

impl PrintLogger {
    pub fn info<T: Debug + Send + 'static>(&self, data: T, msg: u64) {






        ::io::_print(::std::fmt::Arguments::new_v1({
                                                       static __STATIC_FMTSTR:
                                                       &'static [&'static str]
                                                       =
                                                           &["", "\n"];
                                                       __STATIC_FMTSTR
                                                   },
                                                   &match (&data,) {
                                                       (__arg0,) =>
                                                           [::std::fmt::ArgumentV1::new(__arg0,
                                                                                        ::std::fmt::Debug::fmt)],
                                                   }));
    }
    pub fn error<U: Debug + Send + 'static>(&self, data: U) {
        ::io::_print(::std::fmt::Arguments::new_v1({
                                                       static __STATIC_FMTSTR:
                                                       &'static [&'static str]
                                                       =
                                                           &["", "\n"];
                                                       __STATIC_FMTSTR
                                                   },
                                                   &match (&data,) {
                                                       (__arg0,) =>
                                                           [::std::fmt::ArgumentV1::new(__arg0,
                                                                                        ::std::fmt::Debug::fmt)],
                                                   }));
    }
}
enum PrintLoggerMessage<InfoT: Debug + Send + 'static, ErrorU: Debug + Send + 'static> {
    InfoVariant { data: InfoT, msg: u64 },
    ErrorVariant { data: ErrorU },
}
pub struct PrintLoggerActor<InfoT: Debug + Send + 'static, ErrorU: Debug + Send + 'static> {
    sender: Sender<PrintLoggerMessage<InfoT, ErrorU>>,
    receiver: Receiver<PrintLoggerMessage<InfoT, ErrorU>>,
    id: String,
}
extern crate two_lock_queue;
extern crate fibers;
extern crate futures;
use futures::future::*;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};
impl<InfoT: Debug + Send + 'static, ErrorU: Debug + Send + 'static> PrintLoggerActor<InfoT,
    ErrorU> {
    pub fn new<H: Send + fibers::Spawn + Clone + 'static>(handle: H,
                                                          actor: PrintLogger)
                                                          -> PrintLoggerActor<InfoT, ErrorU> {
        let mut actor = actor;
        let (sender, receiver) = unbounded();
        let id = "random string".to_owned();
        let recvr = receiver.clone();
        handle.spawn(futures::lazy(move || {
            loop_fn(0, move |_| match recvr.try_recv() {
                Ok(msg) => {
                    actor.route_msg(msg);
                    Ok::<_, _>(futures::future::Loop::Continue(0))
                }
                Err(TryRecvError::Disconnected) => Ok::<_, _>(futures::future::Loop::Break(())),
                Err(TryRecvError::Empty) => Ok::<_, _>(futures::future::Loop::Continue(0)),
            })
        }));
        PrintLoggerActor {
            sender: sender,
            receiver: receiver,
            id: id,
        }
    }
}
impl PrintLogger {
    pub fn route_msg(&mut self, msg: PrintLoggerMessage<InfoT, ErrorU>) {
        match msg {
            PrintLoggerMessage::InfoVariant { data: data, msg: msg } => self.info(data, msg),
            PrintLoggerMessage::ErrorVariant { data: data } => self.error(data),
        };
    }
}
fn main() {
    let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
    let logger = PrintLogger {};
    let log_actor = PrintLoggerActor::new(system.handle(), logger);
    log_actor.info("info log");
    log_actor.error("error!!".to_owned());
    system.run();
}
