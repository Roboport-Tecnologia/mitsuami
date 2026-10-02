//! `view!`, `#[component]`, `locales!` and `#[derive(IntoValue)]`. Use them
//! through `mitsuami`, not directly.
//!
//! `view!` and `#[component]` are sugar over the builder API: they expand to
//! builder calls, so anything they write can be written by hand. `locales!`
//! builds the app's Fluent files in, and `IntoValue` lets a type be passed
//! where a reactive value is taken.

use proc_macro::TokenStream;

mod component;
mod locales;
mod view;

/// The app's translations, from a folder of Fluent files: one folder per
/// language, named by its tag, with any number of `.ftl` files.
///
/// ```ignore
/// // locales/en-US/app.ftl, locales/pt-BR/app.ftl, …, beside Cargo.toml;
/// // in src/main.rs:
/// App::new().locales(locales!("../locales"))
/// App::new().locales(locales!("../locales", fallback = "pt-BR"))
/// ```
///
/// The path is from the file that calls it, as `include_str!`'s. The
/// files are built into the app, and checked as it's compiled: a syntax error, or the same
/// message in two files of a language, is a compile error. The fallback
/// (the language shown when the user reads none of the app's, and where
/// a message missing from one is looked up) is `en-US` when there is one,
/// else the only language. A new file needs a rebuild of the crate to be
/// seen (touch the file that calls `locales!`); edits to a file don't.
#[proc_macro]
pub fn locales(input: TokenStream) -> TokenStream {
    let file = proc_macro::Span::call_site().local_file();
    locales::expand(input.into(), file).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// JSX-like views, expanded to builder calls.
///
/// ```ignore
/// view! {
///     <Column gap=Spacing::Md padding=16>
///         <Text text_style=TextStyle::Title>{move || format!("Count: {}", count.get())}</Text>
///         <Button role=ButtonRole::Default @click=move || count.update(|c| *c += 1)>"Increment"</Button>
///         <Show when={move || count.get() > 10}>
///             <Text>"That's a big number"</Text>
///         </Show>
///         <For each=items key=|item| item.id let:item>
///             <Text>{item.name}</Text>
///         </For>
///     </Column>
/// }
/// ```
///
/// A tag `<Tag a=x @e=h flag>children</Tag>` becomes
/// `Tag::__tag().a(x).on_e(h).flag().__children(|| children)`:
///
/// - `name=value` calls the builder method `name`. The value is a literal,
///   a path, a call or method chain, a closure, or any expression in
///   braces. Comparisons and generics need the braces: `when={a > b}`.
/// - `@event=handler` calls `on_event`: `@click` is `on_click`.
/// - A bare `name` calls `name()`: `<Row wrap>`.
/// - `let:item` names the item of a `For` row.
/// - Children are string literals, `{expressions}` and tags. One child is
///   passed as is (the text of a `Text`, the label of a `Button`); several
///   become a tuple.
/// - Children are built right away, as with the builder API, except in
///   `Show` and `For`, which rebuild them: there they're in a `move`
///   closure, which owns what it uses (clone a `String` before, as you
///   would for `Show::new`).
///
/// Several root nodes, or `<>…</>`, make a tuple.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    view::expand(input.into()).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Turns a function into a component: a builder with one method per
/// parameter, which runs the function once, when the view is built, in a
/// reactive scope of its own.
///
/// ```ignore
/// #[component]
/// fn Counter(
///     initial: i32,                        // required
///     #[prop(default = 1)] step: i32,      // optional
///     #[prop(into)] label: String,         // accepts anything Into<String>
///     title: Value<String>,                // static or reactive
///     on_change: Callback<i32>,            // `@change=…`, optional
///     children: Slot,                      // what goes between the tags
/// ) -> impl View { /* … */ }
///
/// view! { <Counter initial=3 label="Clicks" title={move || /* … */} @change=move |n| { /* … */ }/> }
/// Counter::new().initial(3).label("Clicks").title(move || /* … */).on_change(move |n| { /* … */ })   // the same, as a builder
/// ```
///
/// - Parameters are required unless they have `#[prop(default)]`,
///   `#[prop(default = expr)]`, or an `Option<T>`, `Callback<T>` or `Slot`
///   type. A missing required prop is a compile error: the builder isn't a
///   `View` until every one is set.
/// - `Value<T>` parameters take a literal, a signal or a closure.
/// - `Callback<T>` parameters take a closure, and default to doing nothing.
/// - A `children: Slot` parameter receives the children.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::expand(attr.into(), item.into()).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Lets a type be passed as it is where a reactive value is taken
/// (`impl IntoValue<T>`), as literals are: a custom widget's props, say,
/// in `Rating::view(RatingProps { .. })` or `props=RatingProps { .. }`. A
/// closure or a signal still makes them reactive.
///
/// ```ignore
/// #[derive(Clone, Debug, PartialEq, IntoValue)]
/// pub struct RatingProps { pub value: u8 }
/// ```
#[proc_macro_derive(IntoValue)]
pub fn derive_into_value(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.cloned().unwrap_or_else(|| syn::parse_quote!(where));
    where_clause.predicates.push(syn::parse_quote!(Self: 'static));
    quote::quote! {
        impl #impl_generics ::mitsuami::reactive::IntoValue<#name #ty_generics> for #name #ty_generics #where_clause {
            fn into_value(self) -> ::mitsuami::reactive::Value<Self> {
                ::mitsuami::reactive::Value::Static(self)
            }
        }
    }
    .into()
}
