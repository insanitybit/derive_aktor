#![feature(proc_macro)]
#![allow(warnings)]

/// https://github.com/insanitybit/aktors/issues/2

#[macro_use]
extern crate derive_aktor;


use derive_aktor::derive_actor;

pub struct Foo {
    bar: u64
}



#[derive_actor]
impl Foo {
    pub fn bar<T: 'static>(&self, baz: u32, blah: T) -> bool {
        true
    }
}

fn main() {
}

#[test]
fn it_works() {}
