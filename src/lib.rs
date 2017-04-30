#![feature(proc_macro)]
extern crate two_lock_queue;

#[macro_use]
extern crate quote;


extern crate proc_macro;
extern crate syn;

use proc_macro::TokenStream;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

#[proc_macro_derive(HelloWorld)]
pub fn hello_world(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let s = input.to_string();

    // Parse the string representation
    let ast = syn::parse_macro_input(&s).unwrap();

    // Build the impl
    let gen = impl_hello_world(&ast);

    // Return the generated impl
    gen.parse().unwrap()
}

#[proc_macro_attribute]
pub fn print_ast(args: TokenStream, input: TokenStream) -> TokenStream {
    let source = input.to_string();

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
                        vis: syn::Visibility::Public,
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

                println!("{:#?}", variant);
            }
        }

        let mut message_enum = syn::ItemKind::Enum(variants, generics);
    }
    //    println!("{:#?}", ast);


    //
    //    println!("{:#?}", ast.attrs);
    //    unimplemented!()

    args
}

//fn function_attr(ast: &syn::MacroInput) -> quote::Tokens {
//    println!("{:#?}", ast);
//
//    quote! {
//    }
//}

fn impl_hello_world(ast: &syn::MacroInput) -> quote::Tokens {
    let name = &ast.ident;
    let actor_name = syn::Ident::from(format!("{}{}", name, "Actor"));
    let actor_msg_name = syn::Ident::from(format!("{}{}", name, "Message"));
    println!("{:#?}", ast);
    quote! {

        enum #actor_msg_name {

        }

        #[derive(Debug)]
        struct #actor_name {
            inner: #name,
//                sender: Sender<#actor_msg_name>,
//                receiver: Receiver<#actor_msg_name>,
//                id: String,
        }

    }
}


//
//#[cfg(test)]
//mod tests {
//    use super::*;
//
////    #[derive(HelloWorld)]
//    struct foo;
//
//    #[test]
//    fn it_works() {
//        foo::hello_world();
//    }
//}