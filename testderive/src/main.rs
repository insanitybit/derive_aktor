#![feature(proc_macro)]
#![allow(warnings)]

/// https://github.com/insanitybit/aktors/issues/2

#[macro_use]
extern crate derive_aktor;


use derive_aktor::print_ast;

//#[derive(Debug, HelloWorld)]
struct Foo {
    bar: u64
}


#[print_ast]
impl Foo {
    pub fn bar(baz: u32, blah: Vec<u8>) -> bool {
        true
    }
//    pub fn bar(baz: u64) -> bool {
//        false
//    }
}

fn main() {
    //    let f = FooActor {
    //        inner: Foo {bar:0}
    //    };

    //    println!("{:#?}", f);
}

#[test]
fn it_works() {}
