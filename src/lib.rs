#![feature(proc_macro)]
extern crate two_lock_queue;
#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;

use proc_macro::TokenStream;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

#[proc_macro_attribute]
pub fn print_ast(args: TokenStream, input: TokenStream) -> TokenStream {
    let source = input.to_string();

    // Generate enum for communicating with Actor
    let actor_message = gen_message(source.clone());

    // The actor Struct will take a type Foo and become an ActorFoo. It will have an impl with a
    // 'new' that takes a Foo and returns an ActorFoo. It will hand that Actor to a thread/ fiber,
    // It will also hand a receiver to the fiber. The fiber will then repeatedly call 'on_message'
    // on the Foo, handing it messages off of the queue.
    let actor_struct = gen_actor_struct(source.clone());

    let actor_impl = gen_actor_impl(source.clone());

    quote!(#actor_message #actor_struct).parse().unwrap()
}

fn gen_actor_impl(source: String) -> quote::Tokens {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        for item in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            println!("{:#?}", item);
        }
        unimplemented!()
    } else {
        panic!("Actor derive only works on impl blocks")
    }
}

fn gen_actor_struct(source: String) -> quote::Tokens {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let (actor_name, msg_name) = if let syn::Ty::Path(_, path) = *ty {
            (syn::Ident::new(format!("{}Actor", path.segments[0].ident.as_ref())), syn::Ident::new(format!("{}Message", path.segments[0].ident.as_ref())))
        } else {
            panic!("Could not find impl ident");
        };
        quote! {
            use std::sync::mpsc::{Receiver, Sender};
            pub struct #actor_name {
                sender: Sender<#msg_name>,
                receiver: Receiver<#msg_name>,
                id: String,
            }
        }
    } else {
        panic!("Actor derive only owrks on impl blocks")
    }
}

fn gen_message(source: String) -> syn::Item {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let impl_name = if let syn::Ty::Path(_, path) = *ty {
            path.segments[0].ident.as_ref().to_owned()
        } else {
            panic!("Could not find impl ident");
        };
        let mut variants = vec![];
        let message_name = format!("{}Message", impl_name);
        for item in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            if let syn::ImplItemKind::Method(ref sig, _) = item.node {
                let variant_id = item.ident.as_ref().to_owned();
                let variant_id = syn::Ident::new(format!("{}{}Message", &variant_id[0..1].to_uppercase(), &variant_id[1..]));
                //                println!("{:#?}", variant_id);
                let mut variant_fields = vec![];
                for (id, ty) in sig.decl.inputs.iter().filter_map(|input| {
                    if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), ref ty) = input {
                        Some((id, ty))
                    } else {
                        None
                    }
                }) {
                    let field = syn::Field {
                        ident: Some(id.clone()),
                        vis: syn::Visibility::Inherited,
                        attrs: vec![],
                        ty: ty.clone()
                    };
                    variant_fields.push(field);
                }
                let variant_data = syn::VariantData::Struct(variant_fields);
                let variant = syn::Variant {
                    ident: variant_id,
                    attrs: vec![],
                    data: variant_data,
                    discriminant: None
                };
                variants.push(variant);
            }
        }
        let message_enum = syn::ItemKind::Enum(variants, generics);
        syn::Item {
            ident: syn::Ident::new(message_name),
            vis: syn::Visibility::Public,
            attrs: vec![],
            node: message_enum,
        }
    } else {
        panic!("")
    }
}