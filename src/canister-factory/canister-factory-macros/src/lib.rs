extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ImplItem, ItemImpl, Pat, PatType, ReturnType, Type};

#[proc_macro_attribute]
pub fn canister_client(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let struct_name = if let Type::Path(type_path) = &*input.self_ty {
        type_path
            .path
            .get_ident()
            .map(|ident| ident.to_string())
            .unwrap_or_default()
    } else {
        return TokenStream::from(quote! {
            compile_error!("canister_client attribute can only be applied to impl blocks with a named struct type");
        });
    };

    let client_name = format_ident!("{}Client", struct_name);

    let mut query_methods = vec![];
    let mut update_methods = vec![];

    for item in &input.items {
        if let ImplItem::Fn(method) = item {
            let is_query = method
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("query"));
            let is_update = method
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("update"));

            if is_query || is_update {
                let method_name = &method.sig.ident;
                let args = method
                    .sig
                    .inputs
                    .iter()
                    .skip(1) // Skip self
                    .map(|arg| {
                        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
                            if let Pat::Ident(pat_ident) = &**pat {
                                let ident = &pat_ident.ident;
                                quote!(#ident: #ty)
                            } else {
                                quote!(#arg)
                            }
                        } else {
                            quote!(#arg)
                        }
                    })
                    .collect::<Vec<_>>();

                let arg_names = method
                    .sig
                    .inputs
                    .iter()
                    .skip(1)
                    .map(|arg| {
                        if let FnArg::Typed(PatType { pat, .. }) = arg {
                            if let Pat::Ident(pat_ident) = &**pat {
                                let ident = &pat_ident.ident;
                                quote!(#ident)
                            } else {
                                quote!()
                            }
                        } else {
                            quote!()
                        }
                    })
                    .collect::<Vec<_>>();

                let return_type = match &method.sig.output {
                    ReturnType::Default => quote!(()),
                    ReturnType::Type(_, ty) => quote!(#ty),
                };

                let args_tuple = if arg_names.is_empty() {
                    quote!(())
                } else {
                    quote!( (#(#arg_names,)*) )
                };

                let client_method = if is_query {
                    quote! {
                        pub async fn #method_name(&self, #(#args),*) -> CanisterClientResult<#return_type> {
                            self.client.query(stringify!(#method_name), #args_tuple).await
                        }
                    }
                } else {
                    quote! {
                        pub async fn #method_name(&self, #(#args),*) -> CanisterClientResult<#return_type> {
                            self.client.update(stringify!(#method_name), #args_tuple).await
                        }
                    }
                };

                if is_query {
                    query_methods.push(client_method);
                } else {
                    update_methods.push(client_method);
                }
            }
        }
    }

    let expanded = quote! {
        #input

        #[derive(Debug, Clone)]
        pub struct #client_name<C: CanisterClient> {
            client: C,
        }

        impl<C: CanisterClient> #client_name<C> {
            pub fn new(client: C) -> Self {
                Self { client }
            }

            #(#query_methods)*

            #(#update_methods)*
        }
    };

    TokenStream::from(expanded)
}
