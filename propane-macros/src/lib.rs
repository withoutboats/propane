extern crate proc_macro;

mod elision;

use proc_macro::*;
use syn::fold::Fold;

#[proc_macro_attribute]
pub fn generator(_: TokenStream, input: TokenStream) -> TokenStream {
    if let Ok(item_fn) = syn::parse(input) {
        let item_fn = Generator { outer_fn: true, lifetimes: vec![] }.fold_item_fn(item_fn);
        quote::quote!(#item_fn).into()
    } else {
        panic!("#[generator] atribute can only be applied to functions");
    }
}

struct Generator {
    outer_fn: bool,
    lifetimes: Vec<syn::Lifetime>,
}

impl Fold for Generator {
    fn fold_item_fn(&mut self, mut i: syn::ItemFn) -> syn::ItemFn {
        if !self.outer_fn { return i }

        let inputs = elision::unelide_lifetimes(&mut i.sig.generics.params, i.sig.inputs);
        self.lifetimes = i.sig.generics.lifetimes().map(|l| l.lifetime.clone()).collect();
        let output = self.fold_return_type(i.sig.output);
        let sig = syn::Signature { output, inputs, ..i.sig };

        self.outer_fn = false;

        let inner = self.fold_block(*i.block);
        let block = Box::new(make_fn_block(&inner));

        syn::ItemFn { sig, block, ..i }
    }

    fn fold_return_type(&mut self, i: syn::ReturnType) -> syn::ReturnType {
        use syn::{ReturnType, Token};
        use proc_macro2::Span;
        if !self.outer_fn { return i; }
        let (arrow, ret) = match i {
            ReturnType::Default => (Token![->](Span::call_site()), syn::parse_str("()").unwrap()),
            ReturnType::Type(arrow, ty) => (arrow, *ty),
        };
        let lifetimes = std::mem::replace(&mut self.lifetimes, vec![]);
        let ret = syn::parse2(quote::quote!(
                (impl Iterator<Item = #ret> #(+ #lifetimes )*)
        )).unwrap();
        ReturnType::Type(arrow, Box::new(ret))
    }

    fn fold_expr(&mut self, i: syn::Expr) -> syn::Expr {
        if let syn::Expr::Try(syn::ExprTry { expr, .. }) = i {
            syn::parse2(quote::quote!(propane::gen_try!(#expr))).unwrap()
        } else {
            syn::fold::fold_expr(self, i)
        }
    }
}

fn make_fn_block(inner: &syn::Block) -> syn::Block {
    syn::parse2(quote::quote! {{
        let __ret = || {
            #inner;
            #[allow(unreachable_code)]
            {
                return;
                yield panic!();
            }
        };

        #[allow(unreachable_code)]
        propane::__internal::GenIter(__ret)
    }}).unwrap()
}
