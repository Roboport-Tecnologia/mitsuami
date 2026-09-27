//! `view!`: parses JSX-like tags and lowers them to builder calls.

use proc_macro2::{Delimiter, Ident, Spacing, Span, TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::parse::{ParseStream, Parser};
use syn::{Block, Error, Expr, Result, Stmt};

/// Tuples implement `Children` up to this many elements; longer child lists
/// are nested.
const MAX_TUPLE: usize = 12;

pub fn expand(input: TokenStream) -> Result<TokenStream> {
    let mut cursor = Cursor { tokens: input.into_iter().collect(), pos: 0 };
    let mut nodes = Vec::new();
    while !cursor.at_end() {
        nodes.push(cursor.node()?);
    }
    Ok(tuple(nodes))
}

/// Several nodes as one value: nothing is `()`, one node is itself, more
/// are a tuple (nested past `MAX_TUPLE`).
fn tuple(mut nodes: Vec<TokenStream>) -> TokenStream {
    match nodes.len() {
        0 => quote!(()),
        1 => nodes.pop().unwrap(),
        n if n <= MAX_TUPLE => quote!((#(#nodes,)*)),
        _ => {
            let chunks = nodes.chunks(MAX_TUPLE).map(|chunk| tuple(chunk.to_vec())).collect();
            tuple(chunks)
        }
    }
}

enum Attr {
    /// `name=value`, or `@event=handler` (already renamed to `on_event`).
    Value(Ident, Expr),
    /// A bare `name`.
    Flag(Ident),
}

struct Cursor {
    tokens: Vec<TokenTree>,
    pos: usize,
}

impl Cursor {
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self, ahead: usize) -> Option<&TokenTree> {
        self.tokens.get(self.pos + ahead)
    }

    fn is_punct(&self, ahead: usize, ch: char) -> bool {
        matches!(self.peek(ahead), Some(TokenTree::Punct(p)) if p.as_char() == ch)
    }

    fn next(&mut self) -> Option<TokenTree> {
        let token = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        token
    }

    /// Where to point errors about what comes next.
    fn span(&self) -> Span {
        self.peek(0).or(self.tokens.last()).map_or_else(Span::call_site, TokenTree::span)
    }

    fn error(&self, message: &str) -> Error {
        Error::new(self.span(), message)
    }

    fn expect_punct(&mut self, ch: char) -> Result<()> {
        if self.is_punct(0, ch) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected `{ch}`")))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<Ident> {
        match self.peek(0) {
            Some(TokenTree::Ident(ident)) => {
                let ident = ident.clone();
                self.pos += 1;
                Ok(ident)
            }
            _ => Err(self.error(&format!("expected {what}"))),
        }
    }

    /// A child: a tag, a literal or a `{…}` expression.
    fn node(&mut self) -> Result<TokenStream> {
        match self.peek(0) {
            Some(TokenTree::Literal(literal)) => {
                let literal = literal.to_token_stream();
                self.pos += 1;
                Ok(literal)
            }
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                let group = group.clone();
                self.pos += 1;
                braced(group.stream(), group.span())
            }
            Some(TokenTree::Punct(p)) if p.as_char() == '<' => self.element(),
            _ => Err(self.error(
                "expected a tag, a string literal, or an expression in braces: `{expression}`; \
                 if this is part of an attribute value, wrap the value in braces: `when={a > b}`",
            )),
        }
    }

    fn element(&mut self) -> Result<TokenStream> {
        self.expect_punct('<')?;
        if self.is_punct(0, '>') {
            self.pos += 1;
            let children = self.children(None)?;
            return Ok(tuple(children));
        }
        let (path, name) = self.path()?;
        let tag_span = path.clone().into_iter().next().map_or_else(Span::call_site, |t| t.span());

        let mut attrs = Vec::new();
        let mut let_pattern: Option<TokenTree> = None;
        let self_closing = loop {
            if self.is_punct(0, '>') {
                self.pos += 1;
                break false;
            }
            if self.is_punct(0, '/') && self.is_punct(1, '>') {
                self.pos += 2;
                break true;
            }
            if self.is_punct(0, '@') {
                self.pos += 1;
                let event = self.expect_ident("an event name after `@`")?;
                self.expect_punct('=')?;
                let method = format_ident!("on_{}", event, span = event.span());
                attrs.push(Attr::Value(method, self.value()?));
                continue;
            }
            let name = self.expect_ident("an attribute, `>` or `/>`")?;
            if name == "let" && self.is_punct(0, ':') {
                self.pos += 1;
                let pattern = self.next().ok_or_else(|| self.error("expected a name after `let:`"))?;
                let_pattern = Some(pattern);
            } else if self.is_punct(0, '=') {
                self.pos += 1;
                attrs.push(Attr::Value(name, self.value()?));
            } else {
                attrs.push(Attr::Flag(name));
            }
        };
        let children = if self_closing { Vec::new() } else { self.children(Some((&name, tag_span)))? };

        let mut out = quote_spanned!(tag_span=> #path::__tag());
        for attr in attrs {
            out = match attr {
                Attr::Value(method, value) => quote!(#out.#method(#value)),
                Attr::Flag(method) => quote!(#out.#method()),
            };
        }
        let children_method = Ident::new("__children", tag_span);
        match (let_pattern, children.is_empty()) {
            (Some(pattern), false) => {
                let children = tuple(children);
                out = quote!(#out.#children_method(move |#pattern| #children));
            }
            (Some(pattern), true) => {
                return Err(Error::new(pattern.span(), "`let:` names the item for the children, but there are none"));
            }
            (None, false) => {
                let children = tuple(children);
                // `Show` and `Window` keep their children to rebuild them,
                // so they own what they use. Every other tag builds them
                // right away, like the builder API, so siblings can share
                // variables.
                let rebuilt = matches!(name.rsplit("::").next(), Some("Show" | "Window"));
                out = if rebuilt {
                    quote!(#out.#children_method(move || #children))
                } else {
                    quote!(#out.#children_method(|| #children))
                };
            }
            (None, true) => {}
        }
        Ok(out)
    }

    /// A tag name: `Column`, `widgets::Column`, `::crate_name::Widget`.
    /// Returns the path and its text, to match the closing tag.
    fn path(&mut self) -> Result<(TokenStream, String)> {
        let mut path = TokenStream::new();
        let mut name = String::new();
        if self.is_punct(0, ':') && self.is_punct(1, ':') {
            path.extend(self.tokens[self.pos..self.pos + 2].iter().cloned());
            name.push_str("::");
            self.pos += 2;
        }
        loop {
            let ident = self.expect_ident("a tag name")?;
            name.push_str(&ident.to_string());
            path.extend([TokenTree::Ident(ident)]);
            if self.is_punct(0, ':') && self.is_punct(1, ':') {
                path.extend(self.tokens[self.pos..self.pos + 2].iter().cloned());
                name.push_str("::");
                self.pos += 2;
            } else {
                return Ok((path, name));
            }
        }
    }

    /// Children up to the closing tag: `</Name>`, or `</>` for fragments.
    fn children(&mut self, tag: Option<(&str, Span)>) -> Result<Vec<TokenStream>> {
        let mut children = Vec::new();
        loop {
            if self.at_end() {
                let (message, span) = match tag {
                    Some((name, span)) => (format!("`<{name}>` is never closed; expected `</{name}>`"), span),
                    None => ("`<>` is never closed; expected `</>`".to_string(), self.span()),
                };
                return Err(Error::new(span, message));
            }
            if self.is_punct(0, '<') && self.is_punct(1, '/') {
                self.pos += 2;
                match tag {
                    Some((name, _)) => {
                        let closing_span = self.span();
                        let (_, closing) = self.path()?;
                        if closing != name {
                            return Err(Error::new(
                                closing_span,
                                format!("expected `</{name}>`, found `</{closing}>`"),
                            ));
                        }
                    }
                    None if !self.is_punct(0, '>') => return Err(self.error("expected `</>`")),
                    None => {}
                }
                self.expect_punct('>')?;
                return Ok(children);
            }
            children.push(self.node()?);
        }
    }

    /// An attribute value. Scans ahead to the end of the opening tag (a `>`
    /// or `/>` that can't belong to an expression), then parses the longest
    /// expression that starts here; what follows is the next attribute.
    fn value(&mut self) -> Result<Expr> {
        let end = self.tag_end();
        let slice: TokenStream = self.tokens[self.pos..end].iter().cloned().collect();
        if slice.is_empty() {
            return Err(self.error("expected a value: a literal, a path, a closure, or `{expression}`"));
        }
        let parser = |input: ParseStream| {
            let expr: Expr = input.parse()?;
            let rest: TokenStream = input.parse()?;
            Ok((expr, rest.into_iter().count()))
        };
        let (expr, rest) = parser.parse2(slice).map_err(|e| {
            Error::new(
                e.span(),
                format!("{e}; wrap values that aren't a literal, a path, a call or a closure in braces: `{{…}}`"),
            )
        })?;
        self.pos = end - rest;
        Ok(unbrace(expr))
    }

    /// Index of the token that ends the opening tag, from the current one.
    fn tag_end(&self) -> usize {
        let mut turbofish = 0usize;
        let mut i = self.pos;
        while i < self.tokens.len() {
            if let TokenTree::Punct(p) = &self.tokens[i] {
                let prev = if i > self.pos { self.tokens.get(i - 1) } else { None };
                let joined_after = |ch: char| matches!(prev, Some(TokenTree::Punct(q)) if q.as_char() == ch && q.spacing() == Spacing::Joint);
                match p.as_char() {
                    '<' if joined_after(':') => turbofish += 1,
                    '>' if turbofish > 0 => turbofish -= 1,
                    // `->`, `=>`, `>=`, `>>` belong to expressions.
                    '>' if joined_after('-') || joined_after('=') || joined_after('>') => {}
                    '>' if matches!(self.tokens.get(i + 1), Some(TokenTree::Punct(q)) if p.spacing() == Spacing::Joint && matches!(q.as_char(), '=' | '>')) =>
                        {}
                    '>' => return i,
                    '/' if p.spacing() == Spacing::Joint
                        && matches!(self.tokens.get(i + 1), Some(TokenTree::Punct(q)) if q.as_char() == '>') =>
                    {
                        return i;
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        i
    }
}

/// `{expr}` as an attribute value is `expr`: the braces only delimit it.
fn unbrace(expr: Expr) -> Expr {
    if let Expr::Block(block) = &expr
        && block.attrs.is_empty()
        && block.label.is_none()
        && let [Stmt::Expr(inner, None)] = block.block.stmts.as_slice()
    {
        return inner.clone();
    }
    expr
}

/// A `{…}` child: the expression inside, or the block when it has
/// statements.
fn braced(stream: TokenStream, span: Span) -> Result<TokenStream> {
    if stream.is_empty() {
        return Err(Error::new(span, "empty braces; put an expression inside, or remove them"));
    }
    let block: Block = syn::parse2(quote_spanned!(span=> { #stream }))?;
    if let [Stmt::Expr(expr, None)] = block.stmts.as_slice() {
        Ok(expr.to_token_stream())
    } else {
        Ok(block.to_token_stream())
    }
}
