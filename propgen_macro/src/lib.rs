use proc_macro::TokenStream;

/// Marker attribute for propgen tool, doesn't change code in any way.
#[proc_macro_attribute]
pub fn propgen(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marker attribute for propgen tool, doesn't change code in any way.
#[proc_macro_attribute]
pub fn propgen_input(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
