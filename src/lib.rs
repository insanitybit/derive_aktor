#![allow(warnings)]
#![recursion_limit = "1024"]

use syn::spanned::Spanned;

#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;

extern crate futures;

use proc_macro::TokenStream;

use quote::{TokenStreamExt, ToTokens};
use syn::{ImplItem, Visibility, FnArg};

use futures::channel::mpsc::{channel, Receiver, Sender};
use syn::punctuated::Punctuated;
use syn::GenericParam;
use syn::token::Comma;


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

    let method_generics = method_generics(items.clone());
    let method_generic_tys = method_generic_tys(items.clone());

    let all_generics = all_generics(items.clone(), o_input.clone());
    let all_generic_tys = all_generic_tys(items.clone(), o_input.clone());

//    let generics_tuple = all_generic_tys_tuple(items.clone(), o_input.clone());

    let message_variants = gen_message_variants(items.clone());

    for item in items.clone() {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let sig = method.sig.clone();
                let ident = method.sig.ident;

                let mut args = quote![];
                let mut arg_and_tys = quote![];

                for arg in method.sig.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::Typed(arg) => {
                            let arg_name = arg.pat;
                            let arg_ty = arg.ty;

                            args.extend(quote!(#arg_name ,));
                            arg_and_tys.extend(quote!(#arg_name : #arg_ty, ));
                        }
                        _ => {
                            continue;
                        }
                    }
                }


                let actor_method = quote!(

                    #[tracing::instrument(skip(#args))]
                    pub async fn #ident (&self, #arg_and_tys) {
                        tracing::trace!("{}.{}", stringify!(#actor_ty), stringify!(#ident));

                        let msg = #message_ty :: #ident { #args };

                        let mut sender = self.sender.clone();

                        let queue_len = self.queue_len.clone();

                        queue_len.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                        if let Err(e) = sender.send(msg).await {
                            panic!(
                                concat!(
                                    "Receiver has failed with {}, propagating error. ",
                                    stringify!(#actor_ty),
                                    ".",
                                    stringify!(#ident)
                                ),
                                e
                            )
                        }

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

                for arg in method.sig.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::Typed(arg) => {
                            let arg_name = arg.pat;

                            args.extend(quote!(#arg_name, ));
                        }
                        _ => {
                            continue;
                        }
                    }
                }

                let arm = if method.sig.asyncness.is_some() {
                    quote!(
                    #message_ty :: #ident { #args }
                        => self. #ident (#args) .await,
                    )
                } else {
                    quote!(
                    #message_ty :: #ident { #args }
                        => self. #ident (#args),
                    )
                };

                route_arms.extend(arm);
            }
        }
    }


    // Generate generics for the enum

    let result = quote! {
        #o_input
        // Message

        #[allow(non_camel_case_types)]
        pub enum #message_ty #all_generics {
            #message_variants
        }

        // Actor route_msg impl
        #[async_trait]
        impl #all_generics aktors::actor::Actor < #message_ty #all_generic_tys > for #self_ty
        {
            async fn route_message(&mut self, message: #message_ty #all_generic_tys ) {
                match message {
                    #route_arms
                };
            }

            fn get_actor_name(&self) -> &str {
                &self.self_actor.as_ref().unwrap().actor_name
            }

            fn close(&mut self) {
                self.self_actor = None;
            }
        }

        // Actor Struct
        pub struct #actor_ty #all_generics {
            sender: Sender<#message_ty #all_generic_tys>,
            inner_rc: std::sync::Arc<std::sync::atomic::AtomicUsize>,
            queue_len: std::sync::Arc<std::sync::atomic::AtomicUsize>,
            actor_name: String,
            actor_uuid: uuid::Uuid,
            actor_num: usize,
        }

        // Actor Impl block
        #impl_token #all_generics #actor_ty #all_generic_tys {
            pub async fn new (mut actor_impl: #self_ty) -> (Self, tokio::task::JoinHandle<()>) {
                let (sender, receiver) = channel(1);
                let inner_rc = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));
                let queue_len = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

                let actor_uuid = uuid::Uuid::new_v4();
                let actor_name = format!(
                    "{} {} {}",
                     stringify!(#actor_ty),
                     actor_uuid,
                     0,
                );
                let inner_actor = Self {
                  sender,
                  inner_rc: inner_rc.clone(),
                  queue_len: queue_len.clone(),
                  actor_name,
                  actor_uuid,
                  actor_num: 0,
                };

                let self_actor = inner_actor.clone();

                actor_impl.self_actor = Some(inner_actor);

                let handle = tokio::task::spawn(
                    aktors::actor::route_wrapper(
                        aktors::actor::Router::new(
                            actor_impl,
                            receiver,
                            inner_rc,
                            queue_len
                        )
                    )
                );

                (self_actor, handle)
            }

            #actor_methods

        }

        impl #all_generics std::fmt::Debug for #actor_ty #all_generic_tys
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!(#actor_ty))
                 .field("actor_name", &self.actor_name)
                 .finish()
            }
        }

        impl #all_generics std::clone::Clone for #actor_ty #all_generic_tys
        {
            fn clone(&self) -> Self {
                self.inner_rc.clone().fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                Self {
                    sender: self.sender.clone(),
                    inner_rc: self.inner_rc.clone(),
                    queue_len: self.queue_len.clone(),
                    actor_name: format!(
                        "{} {} {}",
                         stringify!(#actor_ty),
                         self.actor_uuid,
                         self.actor_num + 1,
                     ),
                     actor_uuid: self.actor_uuid,
                     actor_num: self.actor_num + 1,
                }
            }
        }

        impl #all_generics Drop for #actor_ty #all_generic_tys
        {
            fn drop(&mut self) {
                self.inner_rc.clone().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

    };

    // println!("{}", result);

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

                for arg in method.sig.inputs {
                    let arg: FnArg = arg;
                    match arg {
                        FnArg::Receiver(_) => {
                            continue;
                        }
                        arg => args.extend(quote!(#arg, ))
                    }
                }


                let variant = quote!(
                    #ident {
                        #args
                    },
                );


                message_variants.extend(variant);
            }
        }
    }

    message_variants
}

fn all_generics(items: Vec<ImplItem>, item_impl: syn::ItemImpl) -> impl quote::ToTokens {
    let impl_generics = item_impl.generics;

    let mut all_generics = impl_generics.clone();

    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let mut generics = method.sig.generics;
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                for param in generics.params {
                    all_generics.params.push(param);
                }
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = all_generics.split_for_impl();
    let all_generics = quote!(#impl_generics);
    // println!("all_generic_tys {}", all_generics.to_string());

    all_generics
}


fn all_generic_tys(items: Vec<ImplItem>, item_impl: syn::ItemImpl) -> impl quote::ToTokens {
    let impl_generics = item_impl.generics;

    let mut all_generics = impl_generics.clone();

    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let mut generics = method.sig.generics;
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

                for param in generics.params {
                    all_generics.params.push(param);
                }
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = all_generics.split_for_impl();
    let all_generics = quote!(#ty_generics);
    // println!("all_generic_tys {}", all_generics.to_string());

    all_generics
}

fn all_generic_tys_tuple(items: Vec<ImplItem>, item_impl: syn::ItemImpl) -> impl quote::ToTokens {
    let impl_generics = item_impl.generics;

    let mut all_generics = impl_generics.clone();
    let mut tuple_type: syn::TypeTuple = syn::TypeTuple {
        paren_token: Default::default(),
        elems: Default::default(),
    };

    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let mut generics = method.sig.generics;

                for param in generics.params {
                    if let syn::GenericParam::Type(t) = param {
                        let t = t.ident;
                        tuple_type.elems.push(syn::Type::Verbatim(quote!(#t)));
                    }
                }
            }
        }
    }

    for param in impl_generics.params {
        if let syn::GenericParam::Type(t) = param {
            let t = t.ident;
            tuple_type.elems.push(syn::Type::Verbatim(quote!(#t)));
        }
    }

//    tuple_type.elems.push(syn::Type::Verbatim(quote!(#ty_generics)));
    let tuple_type = quote!(#tuple_type);


    tuple_type
}


fn method_generics(items: Vec<ImplItem>) -> impl quote::ToTokens {
    let mut generic_types = quote!();
    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let generics = method.sig.generics;
                generic_types.extend(quote!(#generics));
            }
        }
    }

    // println!("method_generics {}", generic_types.to_string());

    generic_types
}


fn method_generic_tys(items: Vec<ImplItem>) -> impl quote::ToTokens {
    let mut generic_types = quote!();
    for item in items {
        if let ImplItem::Method(method) = item {
            if let Visibility::Public(vis) = method.vis {
                let mut generics = method.sig.generics;
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                generic_types.extend(quote!(#ty_generics));
            }
        }
    }

//    println!("{}", generic_types.to_string());

    generic_types
}
