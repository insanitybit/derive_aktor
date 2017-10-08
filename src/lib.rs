#![recursion_limit = "1024"]
#![feature(proc_macro)]

#[macro_use]
extern crate quote;
extern crate proc_macro;
extern crate syn;
extern crate futures;
extern crate channel;
extern crate string_morph;

use proc_macro::TokenStream;

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

    let actor_impl = gen_actor_impl(src_impl.clone());

    let route_msg = gen_route_msg(src_impl.clone());
    //
    let parsed_input = syn::parse_item(&source).unwrap();
    quote!(#parsed_input #actor_message #actor_struct #actor_impl #route_msg).parse().unwrap()
}

#[proc_macro_attribute]
pub fn derive_actor_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    let source = input.to_string();
    //    panic!("{}", &args.to_string()[2..args.to_string().len() - 2]);
    let trait_name = syn::Ident::new(&args.to_string()[2..args.to_string().len() - 2]);
    // Foo<A, B> where B: Blah -> syn::Generics for A, B: Blah
    let src_impl = parse_impl_trait(&source);

    let actor_impl = gen_actor_impl_trait(src_impl.clone(), trait_name.clone());

    let parsed_input = syn::parse_item(&source).unwrap();
    quote!(#parsed_input #actor_impl).parse().unwrap()
}

fn parse_impl_trait(source: &str) -> Impl {
    if let syn::ItemKind::Impl(unsafety, polarity, generics, path, ty, items) = syn::parse_item(&source).unwrap().node {
        let impl_name = if let syn::Ty::Path(_, path) = *ty {
            path.segments[0].ident.clone()
        } else {
            panic!("Could not find impl ident");
        };

        let methods: Vec<Method> = items.iter().cloned().filter_map(|item| {
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
    let system_message_name = syn::Ident::new(format!("{}SystemMessage", capitalize(src_impl.original_name.as_ref())));

    // Generate generics for the enum
    let generic_types = gen_msg_types(src_impl.methods.clone());
    let variants = gen_variants(src_impl.methods.clone());
    #[allow(non_shorthand_field_patterns)]
        quote!(
            #[derive(Clone)]
            pub enum #message_name #generic_types {
                #variants
            }

            pub enum #system_message_name #generic_types {
                Inner(#message_name #generic_types),
                Kill,
                HardKill
            }
        )
}


fn gen_variants(methods: Vec<Method>) -> quote::Tokens {
    methods.into_iter().fold(quote!(), |mut q_acc, method| {
        let variant_name = syn::Ident::new(format!("{}Variant", prettify_variant(method.name.as_ref())));

        let generic_idents: Vec<_> = method.signature.generics.ty_params.iter().cloned().map(|ty| ty.ident).collect();

        let variant_fields = method.signature.decl.inputs.iter()
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
            #[allow(non_shorthand_field_patterns)]
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
        let variant_name = syn::Ident::new(format!("{}Variant", prettify_variant(method.name.as_ref())));
        let mut args = quote!();
        let variant_fields = method.signature.decl.inputs.iter()
            .fold(quote!(), |mut variant_fields, arg| {
                if let &syn::FnArg::Captured(syn::Pat::Ident(_, ref id, _), syn::Ty::Path(_, ref ty)) = arg {
                    args.append(quote!(#id, ));

                    variant_fields.append(quote! {
                    #id: #id,
                });
                }
                variant_fields
            });

        let method_name = method.name;

        let arm = quote! {
            #msg_name :: #variant_name {
                #variant_fields
            } => self. #method_name ( #args ),
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
    let system_msg_name = syn::Ident::new(format!("{}SystemMessage", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_types.split_for_impl();

    let (o_impl_generics, o_ty_generics, o_where_clause) = o_generics.split_for_impl();

    let actor_methods = gen_actor_methods(src_impl.clone());

    let s = quote!(#o_impl_generics).to_string();
    let h_o_impl_generics = if s.is_empty() {
        quote!(Init: (Fn(#actor_name, SystemActor) -> #o_name) + Send + Sync + 'static)
    } else {
        let s = syn::Ident::new(&s[1..s.len() - 1]);
        quote!(#s, Init: Fn(#actor_name, SystemActor) -> #o_name #o_ty_generics + Send + Sync + 'static)
    };

    quote! {
        impl #msg_impl_generics #actor_name #msg_ty_generics #msg_where_clause {

            pub fn new <#h_o_impl_generics>(init: Init, system: ::aktors::actor::SystemActor, timeout: std::time::Duration) -> #actor_name #msg_ty_generics
                #o_where_clause {
                    let (sender, receiver) = ::channel::unbounded();
                    let s = sender.clone();
                    let a = ::std::sync::Arc::new(());
                    let id = ::std::sync::Arc::new(::uuid::Uuid::new_v4().to_string());
                    let actor_ref = #actor_name {
                        sender,
                        ref_count: a.clone(),
                        id: id.clone()
                    };


                    let mut actor = init(actor_ref.clone(), system);

                    let init = ::std::sync::Arc::new(init);
                    let init = init.clone();
                    std::thread::spawn(
                        move || {
                            loop {
                                let init = init.clone();
                                match receiver.recv_timeout(timeout) {
                                    Ok(msg) => {
//                                                println!("{} {}", stringify!(#actor_name),
//                                                ::std::sync::Arc::strong_count(&a));
                                        match msg {
                                            #system_msg_name :: Inner(msg) => {
//                                                println!("msg {}", stringify!(#actor_name));
                                                let _msg = msg.clone();
                                                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                                    actor.route_msg(msg);
                                                }));

                                                if let Err(e) = result {
                                                    println!("{} panicked", stringify!(#actor_name));
                                                    actor.on_error(e, _msg, init.clone());
                                                }
                                            },
                                            #system_msg_name :: Kill => {
                                                // We still have messages, so it's possible that we'll hand out new references
                                                if receiver.len() > 1 {
//                                                   println!("received kill msg, {} sending kill {}", stringify!(#actor_name), receiver.len());
                                                   let _ = s.send(#system_msg_name :: Kill);
                                                } else {
                                                    // If we're out of messages but there are still references to us
                                                    // don't die
                                                   if ::std::sync::Arc::strong_count(&a) <= 2 {
//                                                       println!("received kill msg, {} disconnecting {} {}", stringify!(#actor_name), receiver.len(),
//                                                       id.clone());
                                                       let _ = s.send(#system_msg_name :: Kill);
                                                       drop(actor);
                                                       break
                                                   } else {
//                                                       println!("here {}", ::std::sync::Arc::strong_count(&a));
                                                   }
                                                }
                                            },
                                            #system_msg_name :: HardKill => {
                                            drop(actor);
                                                break
                                            }
                                        }
                                    }
                                    Err(::channel::RecvTimeoutError::Disconnected) => {
                                    drop(actor);
//                                        println!("disconnected {}", stringify!(#actor_name));
                                        break
                                    }
                                    Err(::channel::RecvTimeoutError::Timeout) => {
//                                        println!("timed out, {} disconnecting {} {}", stringify!(#actor_name), receiver.len(),
//                                            ::std::sync::Arc::strong_count(&a));
                                        actor.on_timeout()
                                    }
                                }
                            }
                        });
                    actor_ref
                }

                pub fn kill(&self) {
                    self.sender.send( #system_msg_name :: HardKill );
                }

                #actor_methods
        }
    }
}

fn prettify_variant(variant_name: &str) -> String {
    use string_morph::Morph;
    variant_name.to_camel_case()
}

fn gen_actor_impl_trait(src_impl: Impl, trait_name: syn::Ident) -> quote::Tokens {
    let o_generics = src_impl.impl_generics.clone();
    let o_name = src_impl.original_name.clone();

    let actor_name = syn::Ident::new(format!("{}Actor", src_impl.original_name.clone()));
    let msg_name = syn::Ident::new(format!("{}Message", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (msg_impl_generics, msg_ty_generics, msg_where_clause) = msg_types.split_for_impl();

    let (o_impl_generics, o_ty_generics, o_where_clause) = o_generics.split_for_impl();

    let actor_methods = gen_actor_trait_methods(src_impl.clone());

    quote! {
        impl #msg_ty_generics #trait_name for #actor_name  #msg_ty_generics #msg_where_clause {
            #actor_methods
        }
    }
}

// fn foo<T: Bar>(baz: T)
fn gen_actor_trait_methods(src_impl: Impl) -> quote::Tokens {
    let mut actor_methods = quote!();

    for method in src_impl.methods.clone() {
        let mut args = quote!();

        let generic_idents: Vec<_> = method.signature.generics.ty_params.iter().cloned().map(|ty| ty.ident).collect();

        let variant_fields = method.signature.decl.inputs.iter()
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
        let system_msg_name = syn::Ident::new(format!("{}SystemMessage", src_impl.original_name));
        let variant_name = syn::Ident::new(format!("{}Variant", capitalize(method.name.as_ref())));

        let method = quote! {
            fn #method_name ( &self, #args ) {
                let msg = #msg_name :: #variant_name { #variant_fields };
                let msg = #system_msg_name :: Inner ( msg );
                self.sender.send( msg );
            }
        };
        actor_methods.append(method);
    }

    actor_methods
}


// fn foo<T: Bar>(baz: T)
fn gen_actor_methods(src_impl: Impl) -> quote::Tokens {
    let mut actor_methods = quote!();

    for method in src_impl.methods.clone() {
        let mut args = quote!();

        let generic_idents: Vec<_> = method.signature.generics.ty_params.iter().cloned().map(|ty| ty.ident).collect();

        let variant_fields = method.signature.decl.inputs.iter()
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
        let system_msg_name = syn::Ident::new(format!("{}SystemMessage", src_impl.original_name));
        let variant_name = syn::Ident::new(format!("{}Variant", prettify_variant(method.name.as_ref())));

        let method = quote! {
            pub fn #method_name ( &self, #args ) {
                let msg = #msg_name :: #variant_name { #variant_fields };
                let msg = #system_msg_name :: Inner ( msg );
                let _ = self.sender.send( msg );
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
    let system_msg_name = syn::Ident::new(format!("{}SystemMessage", src_impl.original_name));
    let msg_types = gen_msg_types(src_impl.methods.clone());

    let (impl_generics, ty_generics, where_clause) = msg_types.split_for_impl();

    quote! {
        #[derive(Clone)]
        pub struct #actor_name #impl_generics #where_clause {
            sender: ::channel::Sender < #system_msg_name #ty_generics >,
            ref_count: ::std::sync::Arc<()>,
            pub id: ::std::sync::Arc<String>
        }

        impl Drop for #actor_name #impl_generics #where_clause {
            fn drop(&mut self) {
                if ::std::sync::Arc::strong_count(&self.ref_count) <= 3 {
//                    println!("Sending kill");
                    let _ = self.sender.send( #system_msg_name :: Kill );
                };
            }
        }

    }
}