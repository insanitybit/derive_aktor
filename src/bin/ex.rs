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


use derive_aktor::derive_actor;

pub struct Foo<A: 'static + Send, B>
    where B: 'static + Send
{
    bar: A,
    baz: B,
}



impl<A: 'static + Send, B> Foo<A, B>
    where B: 'static + Send
{
    pub fn bar<T: 'static, U>(&self, baz: u32, blah: T, blahh: U) -> bool
        where U: 'static
    {




        true
    }
}
pub enum FooMessage<T: 'static, U>
    where U: 'static
{
    BarMessage { baz: u32, blah: T, blahh: U },
}
pub struct FooActor<T: 'static, U>
    where U: 'static
{
    sender: Sender<FooMessage<T, U>>,
    receiver: Receiver<FooMessage<T, U>>,
    id: String,
}
extern crate two_lock_queue;
extern crate fibers;
extern crate futures;
use futures::future::*;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};
impl<T: 'static, U> FooActor<T, U>
    where U: 'static
{
    pub fn new<A: 'static + Send, B: 'static + Send, H>(handle: H, mut actor: Foo<A, B>) -> FooActor<T, U>
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
        FooActor {
            sender: sender,
            receiver: receiver,
            id: id,
        }
    }
    pub fn bar(&self, baz: u32, blah: T, blahh: U) {
        let msg = FooMessage::BarMessage {
            baz: baz,
            blah: blah,
            blahh: blahh,
        };
        self.sender.send(msg);
    }
}
impl<A: 'static + Send, B> Foo<A, B>
    where B: 'static + Send
{
    pub fn route_msg<T: 'static, U>(&mut self, msg: FooMessage<T, U>) {
        match msg {
            FooMessage::BarMessage { baz: baz, blah: blah, blahh: blahh } => {
                self.bar(baz, blah, blahh)
            }
        };
    }
}
fn main() {}
