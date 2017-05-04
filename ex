pub enum FooMessage {
    BarMessage { baz: u32, blah: Vec<u8> },
}
pub struct FooActor {
    sender: Sender<FooMessage>,
    receiver: Receiver<FooMessage>,
    id: String,
}
extern crate two_lock_queue ;
extern crate fibers ;
extern crate futures ;
;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};
impl FooActor {
    pub fn new<H>(handle: H, actor: Foo) -> FooActor
        where H: Send + fibers::Spawn + Clone + 'static
    {
        let (sender, receiver) = unbounded();
        let id = "random string".to_owned();
        let recvr = receiver.clone();
        handle.spawn(futures::lazy(move || {
            loop_fn(0, move |_| match recvr.try_recv() {
                Ok(msg) => {
                    actor.route_msg(msg);
                    Ok::<_, _>(Loop::Continue(0))
                }
                Err(TryRecvError::Disconnected) => Ok::<_, _>(Loop::Break(())),
                Err(TryRecvError::Empty) => Ok::<_, _>(Loop::Continue(0)),
            })
        }));
        FooActor {
            sender: sender,
            receiver: receiver,
            id: id,
        }
    }
    pub fn bar(&self, baz: u32, blah: Vec<u8>) {
        let msg = FooMessage::BarMessage {
            baz: baz,
            blah: blah,
        };
        self.sender.send(msg);
    }
}
impl Foo {
    pub fn route_msg(&mut self, msg: FooMessage) {
        match msg {
            FooMessage::BarMessage { baz: baz, blah: blah } => self.bar(baz, blah),
        };
    }
}
