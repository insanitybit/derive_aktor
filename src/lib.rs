#![recursion_limit = "1024"]
#![feature(proc_macro)]
extern crate two_lock_queue;
#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;
extern crate fibers;
extern crate futures;

use proc_macro::TokenStream;
use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

#[derive(Debug, Clone)]
struct Method {
    pub name: syn::Ident,
    pub signature: syn::MethodSig
}

impl Method {
    fn return_type(&self) -> &syn::FunctionRetTy {
        return &self.signature.decl.output;
    }

    fn wrapped_return_type(&self) -> Option<syn::Ty> {
        match self.return_type() {
            &syn::FunctionRetTy::Default => None,
            &syn::FunctionRetTy::Ty(ref t) => Some(syn::parse::ty(
                quote!(::futures::sync::oneshot::Receiver< #t >).to_string().as_str()
            ).expect("could not parse type"))
        }
    }

    fn wrapped_return_type_sender(&self) -> Option<syn::Ty> {
        match self.return_type() {
            &syn::FunctionRetTy::Default => None,
            &syn::FunctionRetTy::Ty(ref t) => Some(syn::parse::ty(
                quote!(::futures::sync::oneshot::Sender< #t >).to_string().as_str()
            ).expect("could not parse type"))
        }
    }
}

#[derive(Debug, Clone)]
struct Impl {
    pub original_name: syn::Ident,
    pub impl_generics: syn::Generics,
    pub methods: Vec<Method>
}

#[proc_macro_attribute]
pub fn derive_actor(args: TokenStream, input: TokenStream) -> TokenStream {
    let source = input.to_string();

    // Foo<A, B> where B: Blah -> syn::Generics for A, B: Blah
    let src_impl = parse_impl(&source);

    //    // Generate enum for communicating with Actor
    let actor_message = gen_message(src_impl.clone());

    // The actor Struct will take a type Foo and become an ActorFoo. It will have an impl with a
    // 'new' that takes a Foo and returns an ActorFoo. It will hand that Actor to a thread/ fiber,
    // It will also hand a receiver to the fiber. The fiber will then repeatedly call 'on_message'
    // on the Foo, handing it messages off of the queue.
    let actor_struct = gen_actor_struct(src_impl.clone());

    let actor_impl = gen_actor_impl(src_impl.clone());

    let route_msg = gen_route_msg(src_impl.clone());
    //
    let parsed_input = syn::parse_item(&source).unwrap();
    quote!(#parsed_input #actor_message #actor_struct #actor_impl #route_msg).parse().unwrap()
}

fn parse_impl(source: &str) -> Impl {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let impl_name = if let syn::Ty::Path(_, path) = *ty {
            path.segments[0].ident.clone()
        } else {
            panic!("Could not find impl ident");
        };

        let methods: Vec<Method> = items.iter().cloned().filter(|item| item.vis == syn::Visibility::Public).filter_map(|item| {
            if let syn::ImplItemKind::Method(sig, _) = item.node {
                Some(Method {
                    name: item.ident,
                    signature: sig
                })
            } else {
                None
            }
        }).collect();

        Impl {
            original_name: impl_name,
            impl_generics: generics,
            methods: methods
        }
    } else {
        panic!("Called parse_impl on non impl");
    }
}


///```
/// enum FooMessage<BarT> where BarT: 'static {
///    BarVariant {baz: T}
///}
/// ```

fn gen_message(src_impl: Impl) -> quote::Tokens {
    let message_name = syn::Ident::new(format!("{}Message", capitalize(src_impl.original_name.as_ref())));

    // Generate generics for the enum
    let generic_types = gen_msg_types(src_impl.methods.clone());
    let variants = gen_variants(src_impl.methods.clone());

    quote!(pub enum #message_name #generic_types {
        #variants
    })
}

fn gen_variants(methods: Vec<Method>) -> quote::Tokens {
    methods.into_iter().fold(quote!(), |mut q_acc, method| {
        let variant_name = syn::Ident::new(format!("{}Variant", capitalize(method.name.as_ref())));

        let generic_idents: Vec<_> = method.signature.generics.ty_params.iter().cloned().map(|ty| ty.ident).collect();

        let mut variant_fields = method.signature.decl.inputs.iter()
            .fold(quote!(), |mut variant_fields, arg| {
                if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), syn::Ty::Path(_, ref ty)) = arg {
                    // If we have a generic type we need to mangle it
                    let typ = if generic_idents.contains(&ty.segments[0].ident) {
                        syn::Ident::new(format!("{}{}", capitalize(method.name.as_ref()), ty.segments[0].ident.as_ref()))
                    } else {
                        ty.segments[0].ident.clone()
                    };


                    variant_fields.append(quote! {
                        #id: #typ,
                    });
                }
                variant_fields
            });

        if let Some(typ) = method.wrapped_return_type_sender() {
            variant_fields.append(quote! {
                __return: #typ,
            });
        }

        let variant = quote! {
            #variant_name {
                #variant_fields
            },
        };

        q_acc.append(variant);
        q_acc
    })
}

/// Flattens a Vec of Method into a single generic encompassing each method's generic parameters
fn gen_msg_types(methods: Vec<Method>) -> syn::Generics {
    methods.into_iter().map(|method| {
        let ty_params = method.signature.generics.ty_params.iter().cloned().map(|typ| syn::TyParam {
            attrs: typ.attrs,
            ident: syn::Ident::new(format!("{}{}", capitalize(method.name.as_ref()), typ.ident.as_ref())),
            bounds: typ.bounds,
            default: typ.default,
        }).collect::<Vec<_>>();

        syn::Generics {
            lifetimes: method.signature.generics.lifetimes,
            ty_params: ty_params,
            where_clause: method.signature.generics.where_clause,
        }
    }).fold(
        syn::Generics {
            lifetimes: vec![],
            ty_params: vec![],
            where_clause: syn::WhereClause { predicates: vec![] },
        }, |mut acc, g| {
            acc.lifetimes.extend_from_slice(&g.lifetimes[..]);
            acc.ty_params.extend_from_slice(&g.ty_params[..]);
            acc.where_clause.predicates.extend_from_slice(&g.where_clause.predicates[..]);
            acc
        })
}

fn capitalize(s: &str) -> String {
    let char_0 = &s[0..1].to_uppercase();

    format!("{}{}", char_0, &s[1..])
}

fn gen_route_msg(src_impl: Impl) -> quote::Tokens {
    let o_name = src_impl.original_name.clone();
    let actor_name = syn::Ident::new(format!("{}Actor", src_impl.original_name.clone()));
    let msg_name = syn::Ident::new(format!("{}Message", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_types.split_for_impl();
    let (o_impl_generics, o_ty_generics, o_where_clause) = src_impl.impl_generics.split_for_impl();

    let match_arms = route_match_arms(msg_name.clone(), src_impl.clone());

    quote! {
        impl #o_impl_generics #o_name #o_ty_generics #o_where_clause {
            pub fn route_msg #msg_impl_generics (&mut self, msg: #msg_name #msg_ty_generics ) {
                match msg {
                    #match_arms
                };
            }
        }
    }
}

fn route_match_arms(msg_name: syn::Ident, src_impl: Impl) -> quote::Tokens {
    src_impl.methods.into_iter().fold(quote!(), |mut q_acc, method| {
        let variant_name = syn::Ident::new(format!("{}Variant", capitalize(method.name.as_ref())));
        let mut args = quote!();
        let mut variant_fields = method.signature.decl.inputs.iter()
            .fold(quote!(), |mut variant_fields, arg| {
                if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), syn::Ty::Path(_, ref ty)) = arg {
                    args.append(quote!(#id, ));

                    variant_fields.append(quote! {
                        #id: #id,
                    });
                }
                variant_fields
            });

        let mut has_return = false;
        if let Some(typ) = method.wrapped_return_type_sender() {
            has_return = true;
            variant_fields.append(quote! {
                __return: __return,
            });
        }

        let method_name = method.name;

        let arm = if has_return {
            quote! {
                #msg_name :: #variant_name {
                    #variant_fields
                } => __return.complete(self. #method_name ( #args )),
            }
        } else {
            quote! {
                #msg_name :: #variant_name {
                    #variant_fields
                } => self. #method_name ( #args ),
            }
        };

        q_acc.append(arm);
        q_acc
    })
}

fn gen_actor_impl(src_impl: Impl) -> quote::Tokens {
    let o_generics = src_impl.impl_generics.clone();
    let o_name = src_impl.original_name.clone();

    let actor_name = syn::Ident::new(format!("{}Actor", src_impl.original_name.clone()));
    let msg_name = syn::Ident::new(format!("{}Message", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_types.split_for_impl();

    let (o_impl_generics, o_ty_generics, o_where_clause) = o_generics.split_for_impl();

    // Remove the <>'s around any parameters that may exist
    let s = quote!(#o_impl_generics).to_string();
    let h_o_impl_generics = if s.is_empty() {
        quote!(H: Send + fibers::Spawn + Clone + 'static)
    } else {
        let s = syn::Ident::new(&s[1..s.len() - 1]);
        quote!(#s, H: Send + fibers::Spawn + Clone + 'static)
    };

    let actor_methods = gen_actor_methods(src_impl.clone());

    quote! {
        extern crate two_lock_queue;
        extern crate fibers;
        extern crate futures;
        use futures::future::*;
        use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

        impl #msg_impl_generics #actor_name #msg_ty_generics #msg_where_clause {

            pub fn new <#h_o_impl_generics> (handle: H, actor: #o_name #o_ty_generics) -> #actor_name #msg_ty_generics
                #o_where_clause {
                    let mut actor = actor;
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

                    #actor_name {
                        sender: sender,
                        receiver: receiver,
                        id: id
                    }
                }

            #actor_methods
        }
    }
}


// fn foo<T: Bar>(baz: T) -> oneshot::Receiver<R>
fn gen_actor_methods(src_impl: Impl) -> quote::Tokens {
    let mut actor_methods = quote!();

    for method in src_impl.methods.clone() {
        let mut args = quote!();

        let generic_idents: Vec<_> = method.signature.generics.ty_params.iter().cloned().map(|ty| ty.ident).collect();

        let mut variant_fields = method.signature.decl.inputs.iter()
            .fold(quote!(), |mut variant_fields, arg| {
                if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), syn::Ty::Path(_, ref ty)) = arg {
                    // If we have a generic type we need to mangle it
                    let typ = if generic_idents.contains(&ty.segments[0].ident) {
                        syn::Ident::new(format!("{}{}", capitalize(method.name.as_ref()), ty.segments[0].ident.as_ref()))
                    } else {
                        ty.segments[0].ident.clone()
                    };

                    args.append(quote!(#id: #typ, ));

                    variant_fields.append(quote! {
                        #id: #id,
                    });
                }
                variant_fields
            });

        let method_name = method.name.clone();
        let msg_name = syn::Ident::new(format!("{}Message", src_impl.original_name));
        let variant_name = syn::Ident::new(format!("{}Variant", capitalize(method.name.as_ref())));

        let transformed_type = method.wrapped_return_type();

        let method = if let Some(transformed_type) = transformed_type {
            quote! {
                pub fn #method_name ( &self, #args ) -> #transformed_type {
                    let (sender, future) = ::futures::sync::oneshot::channel();
                    let msg = #msg_name :: #variant_name { #variant_fields __return: sender };
                    self.sender.send( msg );
                    future
                }
            }
        } else {
            quote! {
                pub fn #method_name ( &self, #args ) {
                    let msg = #msg_name :: #variant_name { #variant_fields };
                    self.sender.send( msg );
                }
            }
        };
        actor_methods.append(method);
    }

    actor_methods
}

///struct FooActor<MT, MU> {
///    sender: Sender<MT>,
///    receiver: Receiver<MT>,
///
///}

fn gen_actor_struct(src_impl: Impl) -> quote::Tokens {
    let actor_name = syn::Ident::new(format!("{}Actor", src_impl.original_name));
    let msg_name = syn::Ident::new(format!("{}Message", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (impl_generics, ty_generics, where_clause) = msg_types.split_for_impl();

    quote! {
        pub struct #actor_name #impl_generics #where_clause {
            sender: Sender < #msg_name #ty_generics >,
            receiver: Receiver < #msg_name #ty_generics >,
            id: String
        }
    }
}