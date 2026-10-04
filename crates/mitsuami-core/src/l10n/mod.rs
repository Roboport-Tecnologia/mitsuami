//! Localization, with [Project Fluent](https://projectfluent.org).
//!
//! The app's translations are Fluent files, one set per language
//! ([`Locales`], usually made by `locales!`). The core picks the app's
//! language from the user's, as the platform lists them, and every
//! message, number and date follows it:
//!
//! ```ignore
//! // locales/en-US/app.ftl:
//! //   files-selected = { $count ->
//! //       [one] One file selected
//! //      *[other] { $count } files selected
//! //   }
//! view! { <Text>{t!("files-selected", count = selected.get().len())}</Text> }
//! ```
//!
//! [`t!`](crate::t) makes a closure, so the text follows the language and
//! the arguments' signals; [`tr!`](crate::tr) gives the string once.
//! Numbers and dates in messages are written by the platform, the way the
//! user's region writes them. A right-to-left language lays the app out
//! right to left, native controls and window chrome included.

mod format;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::time::SystemTime;

use fluent_bundle::bundle::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use mitsuami_reactive::{Computed, Owner, Signal, signal};
use unic_langid::CharacterDirection;

pub use format::{
    CurrencyDisplay, DateTimeFormat, DateTimeStyle, NumberFormat, NumberStyle, NumberSymbols, PlatformLocale,
    basic_date_time, basic_number, format_date_time, format_number, format_number_with, rounded,
};
pub use unic_langid::LanguageIdentifier;

/// The language mitsuami's own strings are written in, and the last one
/// every app falls back to.
const TOOLKIT_LANGUAGE: &str = "en-US";
const TOOLKIT_STRINGS: &str = include_str!("../../locales/en-US/mitsuami.ftl");

/// The app's translations: Fluent files, per language. `locales!` makes
/// one from a folder at compile time, checking each file's syntax.
///
/// ```ignore
/// Locales::new()
///     .fallback("en-US")
///     .add("en-US", include_str!("../locales/en-US/app.ftl"))
///     .add("pt-BR", include_str!("../locales/pt-BR/app.ftl"))
/// ```
#[derive(Clone, Default)]
pub struct Locales {
    fallback: Option<LanguageIdentifier>,
    resources: Vec<(LanguageIdentifier, Rc<FluentResource>)>,
}

impl std::fmt::Debug for Locales {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locales").field("fallback", &self.fallback).field("languages", &self.languages()).finish()
    }
}

impl Locales {
    pub fn new() -> Locales {
        Locales::default()
    }

    /// The language shown when none of the user's is there, and where a
    /// message missing from theirs is looked up. Without one, or one the
    /// app has no files for, the first language added.
    ///
    /// # Panics
    /// If `language` isn't a language tag.
    pub fn fallback(mut self, language: &str) -> Locales {
        self.fallback =
            Some(parse_tag(language).unwrap_or_else(|| panic!("mitsuami: {language:?} isn't a language tag")));
        self
    }

    /// Adds a Fluent file's text to a language.
    ///
    /// # Panics
    /// If `language` isn't a language tag, or the text isn't Fluent.
    pub fn add(self, language: &str, source: impl Into<String>) -> Locales {
        self.add_file(language, "(source)", source)
    }

    /// [`add`](Locales::add), naming the file in errors.
    pub fn add_file(mut self, language: &str, file: &str, source: impl Into<String>) -> Locales {
        let tag = parse_tag(language).unwrap_or_else(|| panic!("mitsuami: {language:?} isn't a language tag"));
        let resource = FluentResource::try_new(source.into()).unwrap_or_else(|(_, errors)| {
            panic!("mitsuami: {file} ({language}) isn't valid Fluent: {errors:?}");
        });
        self.resources.push((tag, Rc::new(resource)));
        self
    }

    /// The languages it has, in the order they were added.
    pub fn languages(&self) -> Vec<LanguageIdentifier> {
        let mut languages: Vec<LanguageIdentifier> = Vec::new();
        for (language, _) in &self.resources {
            if !languages.contains(language) {
                languages.push(language.clone());
            }
        }
        languages
    }
}

/// A value passed to a message: a string, a number (which selects plural
/// forms, and which the platform writes), or a date ([`SystemTime`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Arg(FluentValue<'static>);

/// What can be passed to a message in [`t!`](crate::t) and
/// [`tr!`](crate::tr). A signal passes its value, and is tracked.
pub trait ToArg {
    fn to_arg(&self) -> Arg;
}

impl ToArg for Arg {
    fn to_arg(&self) -> Arg {
        self.clone()
    }
}

impl<T: ToArg + ?Sized> ToArg for &T {
    fn to_arg(&self) -> Arg {
        (**self).to_arg()
    }
}

impl ToArg for str {
    fn to_arg(&self) -> Arg {
        Arg(FluentValue::String(self.to_owned().into()))
    }
}

impl ToArg for String {
    fn to_arg(&self) -> Arg {
        self.as_str().to_arg()
    }
}

impl ToArg for std::borrow::Cow<'_, str> {
    fn to_arg(&self) -> Arg {
        self.as_ref().to_arg()
    }
}

impl ToArg for char {
    fn to_arg(&self) -> Arg {
        self.to_string().to_arg()
    }
}

/// `"true"` or `"false"`, to select on: `[true] … *[false] …`.
impl ToArg for bool {
    fn to_arg(&self) -> Arg {
        self.to_string().to_arg()
    }
}

macro_rules! number_args {
    ($($t:ty),*) => {$(
        impl ToArg for $t {
            fn to_arg(&self) -> Arg {
                Arg(FluentValue::from(*self))
            }
        }
    )*};
}

number_args!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

impl ToArg for SystemTime {
    fn to_arg(&self) -> Arg {
        Arg(FluentValue::Custom(Box::new(format::DateTimeArg { time: *self, format: DateTimeFormat::default() })))
    }
}

/// `None` is no value: a selector's default variant, an empty placeable.
impl<T: ToArg> ToArg for Option<T> {
    fn to_arg(&self) -> Arg {
        match self {
            Some(value) => value.to_arg(),
            None => Arg(FluentValue::None),
        }
    }
}

impl<T: ToArg + 'static> ToArg for Signal<T> {
    fn to_arg(&self) -> Arg {
        self.with(|value| value.to_arg())
    }
}

impl<T: ToArg + Clone + 'static> ToArg for Computed<T> {
    fn to_arg(&self) -> Arg {
        self.get().to_arg()
    }
}

/// A message as a closure, for any text a view shows: it follows the app's
/// language, and the signals the arguments read.
///
/// ```ignore
/// <Text>{t!("greeting", name = user.get().name)}</Text>
/// <Button>{t!("save")}</Button>
/// <TextInput placeholder=t!("search.placeholder")/>
/// ```
///
/// `id.attribute` is one of a message's attributes. Arguments are
/// anything [`ToArg`]: strings, numbers (which select plural forms, and
/// which the platform writes, as the user's region writes them), dates
/// ([`SystemTime`]), signals. Each is evaluated again in the closure.
#[macro_export]
macro_rules! t {
    ($id:expr $(, $name:ident = $value:expr)* $(,)?) => {
        move || $crate::tr!($id $(, $name = $value)*)
    };
}

/// A message as a `String`, now, for text that isn't a view's: an alert's,
/// a file name. Read in a closure or an effect, it follows the language
/// there. See [`t!`](crate::t).
#[macro_export]
macro_rules! tr {
    ($id:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::l10n::tr(
            ::core::convert::AsRef::<str>::as_ref(&$id),
            &[$((::core::stringify!($name), $crate::l10n::ToArg::to_arg(&$value))),*],
        )
    };
}

/// A message in the app's language; `id.attribute` for an attribute. A
/// message missing from every language is its id, and an error (which
/// fails tests). Tracked: an effect that reads it runs again when the
/// language changes. Use [`t!`](crate::t) or [`tr!`](crate::tr).
pub fn tr(id: &str, args: &[(&str, Arg)]) -> String {
    match active() {
        Some(l10n) => {
            let _ = l10n.version.get();
            l10n.format(id, args)
        }
        None => id.to_owned(),
    }
}

/// The app's language: the first of the user's it has (or the app asked
/// for with [`set_language`]), else its fallback. Tracked.
pub fn language() -> LanguageIdentifier {
    match active() {
        Some(l10n) => {
            let _ = l10n.version.get();
            l10n.state.borrow().chain[0].clone()
        }
        None => toolkit_language(),
    }
}

/// Whether the app's language is written right to left, which lays the
/// app out right to left. Tracked.
pub fn is_right_to_left() -> bool {
    is_rtl(&language())
}

/// The languages the app has translations for.
pub fn available_languages() -> Vec<LanguageIdentifier> {
    active().map(|l10n| l10n.state.borrow().locales.languages()).unwrap_or_default()
}

/// Shows the app in this language (the one of its languages that best
/// matches it), in place of the user's: an in-app language setting, as
/// some apps have. `None` goes back to the user's. Every message, the
/// direction and mitsuami's own strings follow right away.
pub fn set_language(language: Option<&str>) {
    if let Some(l10n) = active() {
        l10n.update(|state| state.requested = language.map(|l| vec![l.to_owned()]));
    }
}

/// Replaces the app's translations (`App::locales` gives the first), and
/// chooses its language from them again: translations the app loads
/// itself, say, or a story's.
pub fn set_locales(locales: Locales) {
    if let Some(l10n) = active() {
        l10n.set_locales(locales);
    }
}

/// Tracks the language in the running effect, for code that builds text
/// without [`tr`] (menus, which backends title).
pub(crate) fn track() {
    if let Some(l10n) = active() {
        let _ = l10n.version.get();
    }
}

/// Parses a BCP 47 tag, or a POSIX locale name (`pt_BR.UTF-8@euro`), as
/// Linux lists them.
pub fn parse_tag(tag: &str) -> Option<LanguageIdentifier> {
    let tag = tag.split(['.', '@']).next().unwrap_or(tag).replace('_', "-");
    if tag.is_empty() || tag == "C" || tag == "POSIX" {
        return None;
    }
    tag.parse().ok()
}

pub(crate) fn is_rtl(language: &LanguageIdentifier) -> bool {
    language.character_direction() == CharacterDirection::RTL
}

fn toolkit_language() -> LanguageIdentifier {
    TOOLKIT_LANGUAGE.parse().expect("a valid tag")
}

thread_local! {
    /// The localization of the app on this thread: the last `Ui` made.
    /// Backends' own strings and `tr!` outside any view look it up here.
    static ACTIVE: RefCell<Weak<Localization>> = const { RefCell::new(Weak::new()) };
    static TOOLKIT: Rc<FluentResource> =
        Rc::new(FluentResource::try_new(TOOLKIT_STRINGS.to_owned()).expect("mitsuami.ftl is valid Fluent"));
}

fn active() -> Option<Rc<Localization>> {
    ACTIVE.with(|a| a.borrow().upgrade())
}

pub(crate) fn platform_locale() -> Option<Rc<dyn PlatformLocale>> {
    active().map(|l10n| l10n.platform.clone())
}

type Bundle = FluentBundle<Rc<FluentResource>, intl_memoizer::IntlLangMemoizer>;

/// `(language, right to left)`.
type OnChange = Rc<dyn Fn(&LanguageIdentifier, bool)>;

/// A `Ui`'s localization: the app's translations, the languages chosen
/// from them, and the bundles to format messages with.
pub(crate) struct Localization {
    platform: Rc<dyn PlatformLocale>,
    state: RefCell<State>,
    /// Bumped when the language or the translations change: every message
    /// read in an effect tracks it.
    version: Signal<u64>,
    /// `version`'s scope, disposed with the `Ui`.
    scope: Owner,
    /// Tells the `Ui` the language changed.
    on_change: RefCell<Option<OnChange>>,
}

pub(crate) struct State {
    locales: Locales,
    /// The app's (or a test's) choice, in place of the user's languages.
    requested: Option<Vec<String>>,
    /// The languages messages are looked up in, in order; never empty.
    chain: Vec<LanguageIdentifier>,
    bundles: Vec<Bundle>,
    /// Missing messages and formatting errors, for tests to fail on.
    errors: Vec<String>,
    reported: HashSet<String>,
}

impl Localization {
    pub(crate) fn new(platform: Rc<dyn PlatformLocale>) -> Rc<Localization> {
        let scope = Owner::new_root();
        let version = scope.with(|| signal(0));
        let mut state = State {
            locales: Locales::new(),
            requested: None,
            chain: Vec::new(),
            bundles: Vec::new(),
            errors: Vec::new(),
            reported: HashSet::new(),
        };
        state.chain = negotiate(&state, &*platform);
        state.bundles = bundles(&mut state);
        let l10n = Rc::new(Localization {
            platform,
            state: RefCell::new(state),
            version,
            scope,
            on_change: RefCell::new(None),
        });
        ACTIVE.with(|a| *a.borrow_mut() = Rc::downgrade(&l10n));
        l10n
    }

    pub(crate) fn set_on_change(&self, f: impl Fn(&LanguageIdentifier, bool) + 'static) {
        *self.on_change.borrow_mut() = Some(Rc::new(f));
    }

    pub(crate) fn language(&self) -> LanguageIdentifier {
        self.state.borrow().chain[0].clone()
    }

    /// The language, tracked.
    pub(crate) fn current(&self) -> LanguageIdentifier {
        let _ = self.version.get();
        self.language()
    }

    /// Changes the translations or the languages asked for, and chooses
    /// again.
    pub(crate) fn update(&self, f: impl FnOnce(&mut State)) {
        let language = {
            let mut state = self.state.borrow_mut();
            f(&mut state);
            state.chain = negotiate(&state, &*self.platform);
            state.bundles = bundles(&mut state);
            state.chain[0].clone()
        };
        let on_change = self.on_change.borrow().clone();
        if let Some(on_change) = on_change {
            on_change(&language, is_rtl(&language));
        }
        self.version.update(|v| *v += 1);
    }

    pub(crate) fn set_locales(&self, locales: Locales) {
        self.update(|state| state.locales = locales);
    }

    pub(crate) fn set_requested(&self, languages: Option<Vec<String>>) {
        self.update(|state| state.requested = languages);
    }

    pub(crate) fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut self.state.borrow_mut().errors)
    }

    fn format(&self, id: &str, args: &[(&str, Arg)]) -> String {
        let (message_id, attribute) = match id.split_once('.') {
            Some((message, attribute)) => (message, Some(attribute)),
            None => (id, None),
        };
        let mut fluent_args = FluentArgs::with_capacity(args.len());
        for (name, Arg(value)) in args {
            fluent_args.set(*name, value.clone());
        }
        let found = {
            let state = self.state.borrow();
            state.bundles.iter().find_map(|bundle| {
                let message = bundle.get_message(message_id)?;
                let pattern = match attribute {
                    Some(attribute) => message.get_attribute(attribute)?.value(),
                    None => message.value()?,
                };
                let mut errors = Vec::new();
                let text = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors).into_owned();
                Some((text, errors))
            })
        };
        match found {
            Some((text, errors)) => {
                if !errors.is_empty() {
                    self.report(format!("message {id:?}: {errors:?}"));
                }
                text
            }
            None => {
                let chain = self.state.borrow().chain.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ");
                self.report(format!("no message {id:?} in {chain}"));
                id.to_owned()
            }
        }
    }

    fn report(&self, error: String) {
        // Once each: a message shown by an effect that runs often would
        // grow the list for as long as the app runs.
        let mut state = self.state.borrow_mut();
        if state.reported.insert(error.clone()) {
            eprintln!("mitsuami: {error}");
            state.errors.push(error);
        }
    }
}

impl Drop for Localization {
    fn drop(&mut self) {
        self.scope.dispose();
    }
}

/// The app's languages the user prefers, in the user's order, then the
/// app's fallback, then mitsuami's own language for its strings.
fn negotiate(state: &State, platform: &dyn PlatformLocale) -> Vec<LanguageIdentifier> {
    let requested = state.requested.clone().unwrap_or_else(|| platform.languages());
    let requested: Vec<LanguageIdentifier> = requested.iter().filter_map(|tag| parse_tag(tag)).collect();
    let available = state.locales.languages();
    // A fallback the app has no translations for would show every
    // message's id: its first language stands in.
    let fallback =
        state.locales.fallback.clone().filter(|f| available.contains(f)).or_else(|| available.first().cloned());
    let mut chain: Vec<LanguageIdentifier> = match &fallback {
        Some(fallback) => negotiate_languages(&requested, &available, Some(fallback), NegotiationStrategy::Filtering)
            .into_iter()
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let toolkit = toolkit_language();
    if !chain.contains(&toolkit) {
        chain.push(toolkit);
    }
    chain
}

fn bundles(state: &mut State) -> Vec<Bundle> {
    let toolkit = toolkit_language();
    let mut errors = Vec::new();
    let bundles = state
        .chain
        .iter()
        .map(|language| {
            let mut bundle = Bundle::new(vec![language.clone()]);
            bundle.set_formatter(Some(format::format_value));
            let _ = bundle.add_builtins();
            let _ = bundle.add_function("DATETIME", format::DATETIME);
            for (_, resource) in state.locales.resources.iter().filter(|(l, _)| l == language) {
                if let Err(e) = bundle.add_resource(resource.clone()) {
                    errors.push(format!("{language}: {e:?}"));
                }
            }
            // The app's own messages win over mitsuami's.
            if *language == toolkit {
                let _ = bundle.add_resource(TOOLKIT.with(Rc::clone));
            }
            bundle
        })
        .collect();
    state.errors.extend(errors);
    bundles
}
