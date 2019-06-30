#![recursion_limit = "1024"]

use syn::spanned::Spanned;

#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;
extern crate fibers;
extern crate futures;

use proc_macro::TokenStream;

use quote::{TokenStreamExt, ToTokens};
use syn::{ImplItem, Visibility, FnArg};

#[proc_macro_attribute]
pub fn derive_actor(args: TokenStream, item: TokenStream) -> TokenStream
{
    let o_item = item.clone();
    let input: syn::ItemImpl = syn::parse_macro_input!(item as syn::ItemImpl);
    let o_input: syn::ItemImpl = syn::parse_macro_input!(o_item as syn::ItemImpl);

    let attrs = input.attrs;
    let defaultness = input.defaultness;
    let unsafety = input.unsafety;
    let impl_token = input.impl_token;
    let generics = input.generics;
    let ttrait = input.trait_;
    let self_ty = input.self_ty;
    let brace_token = input.brace_token;
    let items: Vec<ImplItem> = input.items;

    let mut actor_methods = quote!();

    let type_name = format!("{}", quote!(#self_ty));

    let type_name: &str = type_name.split("<").next().unwrap_or(&type_name);
    let type_name = type_name.trim();

    let actor_ty = syn::Ident::new(&format!("{}Actor", type_name), self_ty.span());
    let message_ty = syn::Ident::new(&format!("{}Message", type_name), self_ty.span());
    let router_ty = syn::Ident::new(&format!("{}Router", type_name), self_ty.span());

    let all_generics = method_generics(items.clone());
    let all_generic_tys = method_generic_tys(items.clone());

    let message_variants = gen_message_variants(items.clone());

    for item in items.clone() {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let sig = method.sig.clone();
                let ident = method.sig.ident;

                let mut args = quote![];
                let mut arg_and_tys = quote![];

                for arg in method.sig.decl.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::Captured(arg) => {
                            let arg_name = arg.pat;
                            let arg_ty = arg.ty;

                            args.extend(quote!(#arg_name ,));
                            arg_and_tys.extend(quote!(#arg_name : #arg_ty, ));

                        }
                        _ => {
                            continue
                        }
                    }
                }


                let actor_method = quote!(
                    pub fn #ident (&self, #arg_and_tys) {

                        let msg = #message_ty :: #ident { #args };

                        tokio::spawn(self.sender.clone().send(msg).map(|_|()).map_err(|_|()));
                    }
                );

                actor_methods.extend(actor_method);
            }
        }
    }

    let mut route_arms = quote!();

    for item in items.clone() {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let ident = method.sig.ident;

                let mut args = quote![];

                for arg in method.sig.decl.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::Captured(arg) => {
                            let arg_name = arg.pat;

                            args.extend(quote!(#arg_name, ));

                        }
                        _ => {
                            continue
                        }
                    }
                }


                let arm = quote!(
                #message_ty :: #ident { #args }
                    => self. #ident (#args),
                );

                route_arms.extend(arm);
            }
        }
    }


    // Generate generics for the enum

    let result = quote! {
        #o_input
        // Message

        pub enum #message_ty #all_generics {
            #message_variants
        }

        // route_msg impl
        impl #self_ty {
            pub fn route_message #all_generics (&mut self, msg: #message_ty #all_generic_tys ) {
                match msg {
                    #route_arms
                };
            }
        }

        // Actor Struct
        #[derive(Clone)]
        pub struct #actor_ty #all_generics {
            sender: Sender<#message_ty #all_generic_tys>,
        }
        // Actor Impl block
        #impl_token #all_generics #actor_ty #all_generic_tys {
            pub fn new(actor_impl: #self_ty) -> Self {
                let (sender, receiver) = channel(0);
                let id = "random string".to_owned();

                tokio::spawn(#router_ty {
                    receiver,
                    id,
                    actor_impl
                });

                Self {
                    sender
                }
            }

            #actor_methods
        }

        // Router Struct

        pub struct #router_ty #all_generics {
            receiver: Receiver<#message_ty #all_generic_tys>,
            id: String,
            actor_impl: #self_ty

        }

        // Router Future impl

        impl #all_generics Future for #router_ty #all_generic_tys {
            type Item = ();
            type Error = ();

            fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
                match self.receiver.poll() {
                    Ok(Async::Ready(Some(msg))) => {
                        task::current().notify(); // we should poll on receiver again

                        self.actor_impl.route_message(msg);
                        Ok(Async::NotReady)
                    },
                    Ok(Async::Ready(None)) => {
                        self.receiver.close();

                        Ok(Async::Ready(())) // we're done; disconnect
                    },
                    _ => {
                        Ok(Async::NotReady)
                    }
                }
            }
        }


    };

    println!("{}", result.to_string());

    result.into()
}

fn capitalize(s: &str) -> String {
    let char_0 = &s[0..1].to_uppercase();

    format!("{}{}", char_0, &s[1..])
}

fn gen_message_variants(items: Vec<ImplItem>) -> impl quote::ToTokens {
    let mut message_variants = quote!();
    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {

                let ident = method.sig.ident;

                let mut args = quote![];

                for arg in method.sig.decl.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::SelfRef(_) | FnArg::SelfValue(_) => {
                            continue
                        }
                        arg => args.extend(quote!(#arg, ))
                    }
                }


                let variant = quote!(
                    #ident {
                        #args
                    }
                );


                message_variants.extend(variant);
            }
        }
    }

    message_variants
}

fn method_generics(items: Vec<ImplItem>) -> impl quote:: ToTokens {
    let mut generic_types = quote!();
    for item in items {

        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let generics = method.sig.decl.generics;
                generic_types.extend(quote!(#generics));
            }
        }
    }

//    println!("{}", generic_types.to_string());

    generic_types
}


fn method_generic_tys(items: Vec<ImplItem>) -> impl quote:: ToTokens {
    let mut generic_types = quote!();
    for item in items {

        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let mut generics = method.sig.decl.generics;
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                generic_types.extend(quote!(#ty_generics));
            }
        }
    }

//    println!("{}", generic_types.to_string());

    generic_types
}
