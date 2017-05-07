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
    pub signature: syn::MethodSig,
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

    println!("{:#?}", actor_struct);
    //
    //    let actor_impl = gen_actor_impl(source.clone());
    //
    //    let route_msg = gen_route_msg(source.clone());
    //
    //    let parsed_input = syn::parse_item(&source).unwrap();
    //    quote!(#parsed_input #actor_message #actor_struct #actor_impl #route_msg).parse().unwrap()
    unimplemented!();
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

    quote!(enum #message_name #generic_types {
        #variants
    })
}

fn gen_variants(methods: Vec<Method>) -> quote::Tokens {
    methods.into_iter().fold(quote!(), |mut q_acc, method| {
        let variant_name = syn::Ident::new(format!("{}Variant", capitalize(method.name.as_ref())));

        let mut variant_fields = method.signature.decl.inputs.iter()
            .fold(quote!(), |mut variant_fields, arg|  {
            if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), syn::Ty::Path(_, ref ty)) = arg {
                let typ = syn::Ident::new(format!("{}{}", capitalize(method.name.as_ref()), ty.segments[0].ident.as_ref()));

                variant_fields.append(quote!{
                    #id: #typ,
                });
            }
            variant_fields
        });

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

fn gen_route_msg(source: String) -> quote::Tokens {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let mut route_fn = quote!();

        let impl_name = if let syn::Ty::Path(_, path) = *ty {
            path.segments[0].ident.as_ref().to_owned()
        } else {
            panic!("Could not find impl ident");
        };

        let message_name = syn::Ident::new(format!("{}Message", impl_name));
        let mut method_generics = vec![];
        let mut match_arms = quote!();
        for method in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            if let syn::ImplItemKind::Method(ref sig, _) = method.node {
                let method_name = method.ident.clone();
                let mut args: Vec<_> = sig.decl.inputs.iter().filter_map(|input| {
                    if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), ref ty) = input {
                        Some((id, ty))
                    } else {
                        None
                    }
                }).collect();


                let sig_g = sig.generics.clone();
                method_generics.push((sig_g.lifetimes, sig_g.ty_params, sig_g.where_clause.predicates));

                let mut formatted_args = quote!();
                let mut field_names = quote!();

                for (i, (id, _)) in args.clone().into_iter().enumerate() {
                    field_names.append(quote!(#id));
                    if i < args.len() - 1 {
                        field_names.append(quote!(, ));
                    }
                };

                let mut field_mappings = args.clone().into_iter().fold(quote!(), |mut t, (id, _)| {
                    t.append(quote!(#id: #id,));
                    t
                });

                let fn_id = method.ident.clone();
                let variant_id = method.ident.as_ref().to_owned();
                let variant_id = syn::Ident::new(format!("{}{}Message", &variant_id[0..1].to_uppercase(), &variant_id[1..]));

                let arm = quote!(#message_name::#variant_id { #field_mappings } => self.#fn_id(#field_names),);

                match_arms.append(arm);
            }
        }

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.0[..]);
            a
        });
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.1[..]);
            a
        });
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.2[..]);
            a
        });

        let method_names: Vec<_> = items.iter().map(|item| item.ident.as_ref().to_owned()).collect();

        let mut msg_ty_params: Vec<_> = ty_params.iter().cloned().zip(method_names.iter()).map(|(t, name)| syn::TyParam {
            attrs: t.attrs,
            ident: syn::Ident::new(format!("{}{}", name, t.ident.as_ref())),
            bounds: t.bounds,
            default: t.default
        }).collect();

        let msg_generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: msg_ty_params.clone(),
            where_clause: syn::WhereClause { predicates: predicates.clone() }
        };

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_generics.split_for_impl();

        let impl_name = syn::Ident::new(impl_name);
        return quote! {
            impl #impl_generics #impl_name #ty_generics #where_clause {
                pub fn route_msg #msg_impl_generics (&mut self, msg: #message_name #msg_ty_generics) {
                    match msg {
                        #match_arms
                    };
                }
            }
        }
    }

    unimplemented!()
}

fn gen_actor_impl(source: String) -> quote::Tokens {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let mut methods = quote!();


        let impl_name = if let syn::Ty::Path(_, path) = *ty {
            path.segments[0].ident.as_ref().to_owned()
        } else {
            panic!("Could not find impl ident");
        };

        let message_name = syn::Ident::new(format!("{}Message", impl_name));
        let o_name = syn::Ident::new(impl_name.clone());
        let impl_name = syn::Ident::new(format!("{}Actor", impl_name));

        let mut method_generics = vec![];

        for method in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            if let syn::ImplItemKind::Method(ref sig, _) = method.node {
                let method_name = method.ident.clone();

                let mut args: Vec<_> = sig.decl.inputs.iter().filter_map(|input| {
                    if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), ref ty) = input {
                        Some((id, ty))
                    } else {
                        None
                    }
                }).collect();


                let sig_g = sig.generics.clone();
                method_generics.push((sig_g.lifetimes, sig_g.ty_params, sig_g.where_clause.predicates));

                let mut formatted_args = quote!();

                let mut field_mappings = args.clone().into_iter().fold(quote!(), |mut t, (id, _)| {
                    t.append(quote!(#id: #id,));
                    t
                });

                for (i, arg) in args.iter().enumerate() {
                    let (id, ty) = arg.clone();
                    formatted_args.append(quote!(#id: #ty));
                    if i < args.len() - 1 {
                        formatted_args.append(quote!(,));
                    }
                }

                let variant_id = method.ident.as_ref().to_owned();
                let variant_id = syn::Ident::new(format!("{}{}Message", &variant_id[0..1].to_uppercase(), &variant_id[1..]));
                let generics = sig.generics.clone();
                let q = quote!(pub fn #method_name (&self, #formatted_args) {
                    let msg = #message_name::#variant_id {
                        #field_mappings
                    };

                    self.sender.send(msg);
                });

                methods.append(q);
            }
        }

        let actor_generics = generics.clone();
        let (actor_impl_generics, actor_ty_generics, actor_where_clause) = actor_generics.split_for_impl();

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.0[..]);
            a
        });
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.1[..]);
            a
        });
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.2[..]);
            a
        });


        let method_names: Vec<_> = items.iter().map(|item| item.ident.as_ref().to_owned()).collect();

        let mut msg_ty_params: Vec<_> = ty_params.iter().cloned().zip(method_names.iter()).map(|(t, name)| syn::TyParam {
            attrs: t.attrs,
            ident: syn::Ident::new(format!("{}{}", name, t.ident.as_ref())),
            bounds: t.bounds,
            default: t.default
        }).collect();

        let msg_generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: msg_ty_params.clone(),
            where_clause: syn::WhereClause { predicates: predicates.clone() }
        };

        let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_generics.split_for_impl();

        lifetimes.extend_from_slice(&generics.lifetimes[..]);
        ty_params.extend_from_slice(&generics.ty_params[..]);
        predicates.extend_from_slice(&generics.where_clause.predicates[..]);

        let generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: ty_params.clone(),
            where_clause: syn::WhereClause { predicates: predicates.clone() }
        };

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        //        let (mut new_impl_generics, mut new_ty_generics, mut new_where_clause) = actor_generics.split_for_impl();

        let mut new_generics = actor_generics.clone();
        new_generics.ty_params.push(syn::TyParam {
            attrs: vec![],
            ident: syn::Ident::new("H"),
            bounds: vec![],
            default: None
        });


        let (new_impl_generics, _, _) = new_generics.split_for_impl();

        return quote! {
            extern crate two_lock_queue;
            extern crate fibers;
            extern crate futures;
            use futures::future::*;
            use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

            impl #msg_impl_generics #impl_name #msg_ty_generics #msg_where_clause {
                pub fn new #new_impl_generics(handle: H, mut actor: #o_name #actor_ty_generics) -> #impl_name #msg_ty_generics
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

                    #impl_name {
                        sender: sender,
                        receiver: receiver,
                        id: id
                    }
                }

                #methods
            }
        };
    } else {
        panic!("Actor derive only works on impl blocks")
    }

    unimplemented!()
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
            sender: Sender #msg_name #ty_generics,
            receiver: Receiver #msg_name #ty_generics,
            id: String
        }
    }
}

fn old_gen_actor_struct(source: String) -> quote::Tokens {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let (actor_name, msg_name) = if let syn::Ty::Path(_, path) = *ty {
            (syn::Ident::new(format!("{}Actor", path.segments[0].ident.as_ref())), syn::Ident::new(format!("{}Message", path.segments[0].ident.as_ref())))
        } else {
            panic!("Could not find impl ident");
        };

        let mut method_generics = vec![];

        for item in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            if let syn::ImplItemKind::Method(ref sig, _) = item.node {
                let sig_g = sig.generics.clone();
                method_generics.push((sig_g.lifetimes, sig_g.ty_params, sig_g.where_clause.predicates));
            }
        }

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.0[..]);
            a
        });
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.1[..]);
            a
        });
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {
            a.extend_from_slice(&g.2[..]);
            a
        });

        let method_names: Vec<_> = items.iter().map(|item| item.ident.as_ref().to_owned()).collect();

        let mut msg_ty_params: Vec<_> = ty_params.iter().cloned().zip(method_names.iter()).map(|(t, name)| syn::TyParam {
            attrs: t.attrs,
            ident: syn::Ident::new(format!("{}{}", name, t.ident.as_ref())),
            bounds: t.bounds,
            default: t.default
        }).collect();

        let msg_generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: msg_ty_params.clone(),
            where_clause: syn::WhereClause { predicates: predicates.clone() }
        };

        lifetimes.extend_from_slice(&generics.lifetimes[..]);
        ty_params.extend_from_slice(&generics.ty_params[..]);
        predicates.extend_from_slice(&generics.where_clause.predicates[..]);

        let generics = syn::Generics {
            lifetimes,
            ty_params,
            where_clause: syn::WhereClause { predicates }
        };

        let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_generics.split_for_impl();

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        quote! {
            pub struct #actor_name #msg_generics #msg_where_clause {
                sender: Sender<#msg_name #msg_ty_generics>,
                receiver: Receiver<#msg_name #msg_ty_generics>,
                id: String,
            }
        }
    } else {
        panic!("Actor derive only owrks on impl blocks")
    }
}