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

#[proc_macro_attribute]
pub fn derive_aktor(args: TokenStream, input: TokenStream) -> TokenStream {
    let source = input.to_string();

    // Generate enum for communicating with Actor
    let actor_message = gen_message(source.clone());

    // The actor Struct will take a type Foo and become an ActorFoo. It will have an impl with a
    // 'new' that takes a Foo and returns an ActorFoo. It will hand that Actor to a thread/ fiber,
    // It will also hand a receiver to the fiber. The fiber will then repeatedly call 'on_message'
    // on the Foo, handing it messages off of the queue.
    let actor_struct = gen_actor_struct(source.clone());

    let actor_impl = gen_actor_impl(source.clone());

    let route_msg = gen_route_msg(source.clone());

    let parsed_input = syn::parse_item(&source).unwrap();
    quote!(#parsed_input #actor_message #actor_struct #actor_impl #route_msg).parse().unwrap()
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

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.0[..]); a});
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.1[..]); a});
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.2[..]); a});

        let msg_generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: ty_params.clone(),
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

        let impl_name = syn::Ident::new(impl_name);
        return quote! {
            impl #generics #impl_name #generics {
                pub fn route_msg(&mut self, msg: #message_name #msg_generics) {
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

        let mut method_generics= vec![];

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
                let q = quote!(pub fn #method_name #generics (&self, #formatted_args) {
                    let msg = #message_name::#variant_id {
                        #field_mappings
                    };

                    self.sender.send(msg);
                });

                methods.append(q);
            }
        }

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.0[..]); a});
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.1[..]); a});
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.2[..]); a});

        let msg_generics = syn::Generics {
            lifetimes: lifetimes.clone(),
            ty_params: ty_params.clone(),
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

        return quote! {
            extern crate two_lock_queue;
            extern crate fibers;
            extern crate futures;
            use futures::future::*;
            use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

            impl #generics #impl_name #generics {
                pub fn new<H>(handle: H, mut actor: #o_name) -> #impl_name #generics
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

fn gen_actor_struct(source: String) -> quote::Tokens {
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

        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.0[..]); a});
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.1[..]); a});
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.2[..]); a});

        lifetimes.extend_from_slice(&generics.lifetimes[..]);
        ty_params.extend_from_slice(&generics.ty_params[..]);
        predicates.extend_from_slice(&generics.where_clause.predicates[..]);

        let generics = syn::Generics {
            lifetimes,
            ty_params,
            where_clause: syn::WhereClause { predicates }
        };

        quote! {
            pub struct #actor_name #generics {
                sender: Sender<#msg_name #generics>,
                receiver: Receiver<#msg_name #generics>,
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

        let mut method_generics = vec![];

        for item in items.iter().filter(|item| item.vis == syn::Visibility::Public) {
            if let syn::ImplItemKind::Method(ref sig, _) = item.node {
                let variant_id = item.ident.as_ref().to_owned();
                let variant_id = syn::Ident::new(format!("{}{}Message", &variant_id[0..1].to_uppercase(), &variant_id[1..]));

                let sig_g = sig.generics.clone();
                method_generics.push((sig_g.lifetimes, sig_g.ty_params, sig_g.where_clause.predicates));

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


        let mut lifetimes: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.0[..]); a});
        let mut ty_params: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.1[..]); a});
        let mut predicates: Vec<_> = method_generics.iter().cloned().fold(Vec::new(), |mut a, g| {a.extend_from_slice(&g.2[..]); a});

        lifetimes.extend_from_slice(&generics.lifetimes[..]);
        ty_params.extend_from_slice(&generics.ty_params[..]);
        predicates.extend_from_slice(&generics.where_clause.predicates[..]);

        let generics = syn::Generics {
            lifetimes,
            ty_params,
            where_clause: syn::WhereClause { predicates }
        };

        let message_enum = syn::ItemKind::Enum(variants, generics);
        syn::Item {
            ident: syn::Ident::new(message_name),
            vis: syn::Visibility::Inherited,
            attrs: vec![],
            node: message_enum,
        }
    } else {
        panic!("")
    }
}
