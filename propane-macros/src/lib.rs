extern crate proc_macro;

mod elision;

use proc_macro::*;
use syn::fold::Fold;
use syn::parse::{Parse, ParseStream, Result};
use syn::{ReturnType, Token};

use proc_macro2::Span;

#[proc_macro_attribute]
pub fn generator(args: TokenStream, input: TokenStream) -> TokenStream {
    let fehler_info = if !args.is_empty() {
        let args: Args = syn::parse(args).unwrap();
        Some(args.throws)
    } else {
        None
    };

    let mut folder = Generator {
        outer_fn: true,
        is_async: false,
        is_move: true,
        lifetimes: vec![],
        fehler_info,
    };
    if let Ok(item_fn) = syn::parse(input.clone()) {
        let item_fn = folder.fold_item_fn(item_fn);
        quote::quote!(#item_fn).into()
    } else if let Ok(method) = syn::parse(input) {
        let method = folder.fold_impl_item_method(method);
        quote::quote!(#method).into()
    } else {
        panic!("#[generator] atribute can only be applied to functions");
    }
}

#[proc_macro]
pub fn gen(input: TokenStream) -> TokenStream {
    let mut folder = Generator {
        outer_fn: false,
        is_async: false,
        is_move: false,
        fehler_info: None,
        lifetimes: vec![]
    };
    let block = folder.block(input.into());
    quote::quote!(#block).into()
}

#[proc_macro]
pub fn gen_move(input: TokenStream) -> TokenStream {
    let mut folder = Generator {
        outer_fn: false,
        is_async: false,
        is_move: true,
        fehler_info: None,
        lifetimes: vec![]
    };
    let block = folder.block(input.into());
    quote::quote!(#block).into()
}

#[proc_macro]
pub fn async_gen(input: TokenStream) -> TokenStream {
    let mut folder = Generator {
        outer_fn: false,
        is_async: true,
        is_move: false,
        fehler_info: None,
        lifetimes: vec![]
    };
    let block = folder.block(input.into());
    quote::quote!(#block).into()
}

#[proc_macro]
pub fn async_gen_move(input: TokenStream) -> TokenStream {
    let mut folder = Generator {
        outer_fn: false,
        is_async: true,
        is_move: true,
        fehler_info: None,
        lifetimes: vec![]
    };
    let block = folder.block(input.into());
    quote::quote!(#block).into()
}

struct Args {
    throws: proc_macro2::TokenStream,
}

impl Parse for Args {
    #[cfg(fehler)]
    fn parse(input: ParseStream) -> Result<Args> {
        let ident: syn::Ident = input.parse()?;
        assert_eq!(ident, "throws", "propane::generator does not take arguments other than `throws`");
        Ok(Args {
            throws: input.cursor().token_stream(),
        })
    }

    #[cfg(not(fehler))]
    fn parse(input: ParseStream) -> Result<Args> {
        let ident: syn::Ident = input.parse()?;
        assert_eq!(ident, "throws", "propane::generator does not take arguments other than `throws`");
        panic!("propane::generator only takes `throws` argument with the `fehler` feature turned on");
    }
}

struct Generator {
    outer_fn: bool,
    is_async: bool,
    is_move: bool,
    fehler_info: Option<proc_macro2::TokenStream>,
    lifetimes: Vec<syn::Lifetime>,
}

impl Generator {
    fn block(&mut self, input: proc_macro2::TokenStream) -> syn::Block {
        let block = syn::parse2(quote::quote!({ #input })).unwrap();
        let block = self.fold_block(block);
        self.finish(&block)
    }

    fn visit_fn_attrs(&mut self, attrs: &mut Vec<syn::Attribute>) {
        struct OuterAttributes(Vec<syn::Attribute>);
        impl Parse for OuterAttributes {
            fn parse(input: ParseStream) -> Result<OuterAttributes> {
                input.call(syn::Attribute::parse_outer).map(OuterAttributes)
            }
        }

        if let Some(args) = self.fehler_info.take() {
            let attr: OuterAttributes = syn::parse2(quote::quote! {
                #[::propane::__internal::fehler::throws(@__internal_propane_integration #args)]
            }).unwrap();
            attrs.extend(attr.0);
        }
    }

    fn finish(&self, block: &syn::Block) -> syn::Block {
        let move_token = match self.is_move {
            true    => Some(Token![move](Span::call_site())),
            false   => None,
        };
        if !self.is_async {
            syn::parse2(quote::quote! {{
                let __ret = #move_token || {
                    #block;
                    #[allow(unreachable_code)]
                    {
                        return;
                        yield panic!();
                    }
                };

                #[allow(unreachable_code)]
                ::propane::__internal::GenIter(__ret)
            }}).unwrap()
        } else {
            syn::parse2(quote::quote! {{
                let __ret = static #move_token |mut __propane_stream_ctx| {
                    #block;
                    #[allow(unreachable_code)]
                    {
                        return;
                        yield panic!();
                    }
                };

                #[allow(unreachable_code)]
                unsafe { ::propane::__internal::GenStream::new(__ret) }
            }}).unwrap()
        }
    }
}

impl Fold for Generator {
    fn fold_item_fn(&mut self, mut i: syn::ItemFn) -> syn::ItemFn {
        if !self.outer_fn { return i }

        self.visit_fn_attrs(&mut i.attrs);

        let inputs = elision::unelide_lifetimes(&mut i.sig.generics.params, i.sig.inputs);
        self.lifetimes = i.sig.generics.lifetimes().map(|l| l.lifetime.clone()).collect();

        self.is_async = i.sig.asyncness.is_some();

        let output = self.fold_return_type(i.sig.output);
        let sig = syn::Signature { output, inputs, asyncness: None, ..i.sig };

        self.outer_fn = false;

        let inner = self.fold_block(*i.block);
        let block = Box::new(self.finish(&inner));

        syn::ItemFn { sig, block, ..i }
    }

    fn fold_impl_item_method(&mut self, mut i: syn::ImplItemMethod) -> syn::ImplItemMethod {
        if !self.outer_fn { return i }
        self.visit_fn_attrs(&mut i.attrs);

        let inputs = elision::unelide_lifetimes(&mut i.sig.generics.params, i.sig.inputs);
        self.lifetimes = i.sig.generics.lifetimes().map(|l| l.lifetime.clone()).collect();

        self.is_async = i.sig.asyncness.is_some();

        let output = self.fold_return_type(i.sig.output);
        let sig = syn::Signature { output, inputs, asyncness: None, ..i.sig };

        let inner = self.fold_block(i.block);
        let block = self.finish(&inner);

        syn::ImplItemMethod { sig, block, ..i }
    }

    fn fold_return_type(&mut self, i: syn::ReturnType) -> syn::ReturnType {
        if !self.outer_fn { return i; }
        
        let (arrow, ret) = match i {
            ReturnType::Default => (Token![->](Span::call_site()), syn::parse_str("()").unwrap()),
            ReturnType::Type(arrow, ty) => (arrow, *ty),
        };
        let lifetimes = std::mem::replace(&mut self.lifetimes, vec![]);

        let ret = if self.is_async {
            syn::parse2(quote::quote!((impl ::propane::__internal::Stream<Item = #ret> #(+ #lifetimes )*))).unwrap()
        } else {
            syn::parse2(quote::quote!((impl Iterator<Item = #ret> #(+ #lifetimes )*))).unwrap()
        };
        ReturnType::Type(arrow, Box::new(ret))
    }

    fn fold_expr(&mut self, i: syn::Expr) -> syn::Expr {
        match i {
            // Stream modifiers
            syn::Expr::Try(syn::ExprTry { expr, .. }) if self.is_async      => {
                let expr = self.fold_expr(*expr);
                syn::parse2(quote::quote!(propane::async_gen_try!(#expr))).unwrap()
            }
            syn::Expr::Yield(syn::ExprYield { expr: Some(expr), .. }) if self.is_async  => {
                let expr = self.fold_expr(*expr);
                syn::parse2(quote::quote!(propane::async_gen_yield!(#expr))).unwrap()
            }
            syn::Expr::Yield(syn::ExprYield { expr: None, .. }) if self.is_async  => {
                syn::parse2(quote::quote!(propane::async_gen_yield!(()))).unwrap()
            }
            syn::Expr::Await(syn::ExprAwait { base: expr, ..}) if self.is_async   => {
                let expr = self.fold_expr(*expr);
                syn::parse2(quote::quote!(propane::async_gen_await!(#expr, __propane_stream_ctx))).unwrap()
            }

            // Iterator modifiers
            syn::Expr::Try(syn::ExprTry { expr, .. })                       => {
                let expr = self.fold_expr(*expr);
                syn::parse2(quote::quote!(propane::gen_try!(#expr))).unwrap()
            }

            // Everything else
            _   => syn::fold::fold_expr(self, i)
        }
    }
}
