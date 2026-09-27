use std::fmt;
use std::rc::Rc;

use crate::{Computed, Signal};

/// A value that is either fixed or derived reactively. Widget props take
/// `impl IntoValue<T>` so callers can pass a literal, a signal, or a closure.
pub enum Value<T: 'static> {
    Static(T),
    Dynamic(Rc<dyn Fn() -> T>),
}

impl<T: 'static> Clone for Value<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Value::Static(v) => Value::Static(v.clone()),
            Value::Dynamic(f) => Value::Dynamic(f.clone()),
        }
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Static(v) => f.debug_tuple("Static").field(v).finish(),
            Value::Dynamic(_) => f.write_str("Dynamic(..)"),
        }
    }
}

impl<T: Clone + 'static> Value<T> {
    /// Current value. Tracked when dynamic.
    pub fn get(&self) -> T {
        match self {
            Value::Static(v) => v.clone(),
            Value::Dynamic(f) => f(),
        }
    }
}

/// Conversion into a [`Value`].
pub trait IntoValue<T: 'static> {
    fn into_value(self) -> Value<T>;
}

impl<T: 'static> IntoValue<T> for Value<T> {
    fn into_value(self) -> Value<T> {
        self
    }
}

impl<T: 'static, F: Fn() -> T + 'static> IntoValue<T> for F {
    fn into_value(self) -> Value<T> {
        Value::Dynamic(Rc::new(self))
    }
}

impl<T: Clone + 'static> IntoValue<T> for Signal<T> {
    fn into_value(self) -> Value<T> {
        Value::Dynamic(Rc::new(move || self.get()))
    }
}

impl<T: Clone + 'static> IntoValue<T> for Computed<T> {
    fn into_value(self) -> Value<T> {
        Value::Dynamic(Rc::new(move || self.get()))
    }
}

impl IntoValue<String> for &str {
    fn into_value(self) -> Value<String> {
        Value::Static(self.to_owned())
    }
}

impl IntoValue<String> for &String {
    fn into_value(self) -> Value<String> {
        Value::Static(self.clone())
    }
}

macro_rules! static_values {
    ($($t:ty),*) => {$(
        impl IntoValue<$t> for $t {
            fn into_value(self) -> Value<$t> {
                Value::Static(self)
            }
        }
    )*};
}

static_values!(String, bool, char, f32, f64, i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl IntoValue<(i32, i32)> for (i32, i32) {
    fn into_value(self) -> Value<(i32, i32)> {
        Value::Static(self)
    }
}

impl IntoValue<(f64, f64)> for (f64, f64) {
    fn into_value(self) -> Value<(f64, f64)> {
        Value::Static(self)
    }
}

impl<T: 'static> IntoValue<Vec<T>> for Vec<T> {
    fn into_value(self) -> Value<Vec<T>> {
        Value::Static(self)
    }
}

impl<const N: usize> IntoValue<Vec<String>> for [&str; N] {
    fn into_value(self) -> Value<Vec<String>> {
        Value::Static(self.iter().map(|s| (*s).to_owned()).collect())
    }
}

impl IntoValue<Vec<String>> for &[&str] {
    fn into_value(self) -> Value<Vec<String>> {
        Value::Static(self.iter().map(|s| (*s).to_owned()).collect())
    }
}
