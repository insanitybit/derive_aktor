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
    pub fn info<T: Debug + Send + 'static>(&self, data: T) {






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
pub enum PrintLoggerMessage<infoT: Debug + Send + 'static, errorU: Debug + Send + 'static> {
    InfoMessage { data: T },
    ErrorMessage { data: U },
}
pub struct PrintLoggerActor<infoT: Debug + Send + 'static, errorU: Debug + Send + 'static> {
    sender: Sender<PrintLoggerMessage<infoT, errorU>>,
    receiver: Receiver<PrintLoggerMessage<infoT, errorU>>,
    id: String,
}
extern crate two_lock_queue;
extern crate fibers;
extern crate futures;
use futures::future::*;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};
impl<infoT: Debug + Send + 'static, errorU: Debug + Send + 'static> PrintLoggerActor<infoT,
    errorU> {
    pub fn new<H>(handle: H, mut actor: PrintLogger) -> PrintLoggerActor<infoT, errorU>
        where H: Send + fibers::Spawn + Clone + 'static
    {
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
    pub fn info(&self, data: T) {
        let msg = PrintLoggerMessage::InfoMessage { data: data };
        self.sender.send(msg);
    }
    pub fn error(&self, data: U) {
        let msg = PrintLoggerMessage::ErrorMessage { data: data };
        self.sender.send(msg);
    }
}
impl PrintLogger {
    pub fn route_msg<infoT: Debug + Send + 'static, errorU: Debug + Send +
    'static>(&mut self,
             msg: PrintLoggerMessage<infoT, errorU>) {
        match msg {
            PrintLoggerMessage::InfoMessage { data: data } => self.info(data),
            PrintLoggerMessage::ErrorMessage { data: data } => self.error(data),
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
