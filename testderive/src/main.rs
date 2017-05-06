#![feature(proc_macro)]
#![allow(warnings)]

/// https://github.com/insanitybit/aktors/issues/2

#[macro_use]
extern crate derive_aktor;


use derive_aktor::derive_actor;

pub struct Foo<A: 'static + Send, B: 'static + Send>{
    bar: A,
    baz: B
}



#[derive_actor]
impl<A: 'static + Send, B: 'static + Send> Foo<A, B> {

    pub fn bar<T: 'static, U>(&self, baz: u32, blah: T, blahh: U) -> bool
        where U: 'static {
        true
    }
}

fn main() {
}

#[test]
fn it_works() {}
