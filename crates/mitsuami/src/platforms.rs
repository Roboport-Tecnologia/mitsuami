//! `platform!`: compile-time choice between per-platform code.

/// Picks an expression by target platform, at compile time: code for other
/// platforms is never compiled. Arms are `macos`, `windows`, `linux`, several
/// of them joined with `|`, and a final `_` for everything else. The first
/// matching arm wins. Arms may have different types, since only one exists.
///
/// On Linux, `gtk` and `kde` name the toolkit: `kde` matches when mitsuami
/// is built with its `kde` feature (Qt Quick and Kirigami), `gtk` when it
/// isn't. `linux` matches either, so put `kde` or `gtk` arms before it.
/// On Windows, `win32` matches when mitsuami is built with its `win32`
/// feature (the classic controls), `winui` when it isn't; `windows`
/// matches either.
///
/// ```ignore
/// fn preferences() -> impl View {
///     let model = use_preferences();
///     platform! {
///         macos => mac::preferences_window(model),
///         windows | linux => preferences_page(model),
///     }
/// }
/// ```
///
/// Without a `_` arm, building for a platform no arm names is an error, so a
/// missing screen can't ship by accident.
#[macro_export]
macro_rules! platform {
    ($($arms:tt)+) => {
        $crate::__platform!(@arms __platform_value [] [] $($arms)+)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __platform {
    // `_` arm: everything no earlier arm matched.
    (@arms $v:ident [$($seen:tt)*] [$($out:tt)*] _ => $e:expr $(,)?) => {{
        $($out)*
        #[cfg(not(any($($seen)*)))]
        let $v = $e;
        $v
    }};
    // No arms left and no `_`.
    (@arms $v:ident [$($seen:tt)*] [$($out:tt)*]) => {{
        $($out)*
        #[cfg(not(any($($seen)*)))]
        ::core::compile_error!("platform!: no arm for this platform; add one, or a `_` arm");
        $v
    }};
    // Start of an arm: collect its platforms.
    (@arms $v:ident $seen:tt $out:tt $($rest:tt)+) => {
        $crate::__platform!(@names $v $seen $out [] $($rest)+)
    };

    (@names $v:ident $seen:tt $out:tt [$($p:tt)*] macos $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "macos",] $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt [$($p:tt)*] windows $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "windows",] $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt [$($p:tt)*] linux $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "linux",] $($rest)*)
    };
    // The toolkits: what they mean was settled when mitsuami was built.
    (@names $v:ident $seen:tt $out:tt $p:tt gtk $($rest:tt)*) => {
        $crate::__platform_gtk!($v $seen $out $p $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt $p:tt kde $($rest:tt)*) => {
        $crate::__platform_kde!($v $seen $out $p $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt $p:tt win32 $($rest:tt)*) => {
        $crate::__platform_win32!($v $seen $out $p $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt $p:tt winui $($rest:tt)*) => {
        $crate::__platform_winui!($v $seen $out $p $($rest)*)
    };
    (@names $v:ident $seen:tt $out:tt $p:tt | $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out $p $($rest)*)
    };
    // End of an arm: emit it, guarded against earlier arms.
    (@names $v:ident [$($seen:tt)*] [$($out:tt)*] [$($p:tt)+] => $e:expr $(, $($rest:tt)*)?) => {
        $crate::__platform!(
            @arms $v [$($seen)* $($p)*]
            [
                $($out)*
                #[cfg(all(any($($p)*), not(any($($seen)*))))]
                let $v = $e;
            ]
            $($($rest)*)?
        )
    };
    (@names $v:ident $seen:tt $out:tt $p:tt $other:tt $($rest:tt)*) => {
        ::core::compile_error!(::core::concat!(
            "platform!: unknown platform `",
            ::core::stringify!($other),
            "`; use macos, windows, linux, gtk, kde, win32, winui, `|` between them, or `_`"
        ))
    };
}

// `kde` is Linux with the `kde` feature; `gtk` is Linux without it. A
// toolkit that isn't built in adds `any()`, which never matches.

#[cfg(feature = "kde")]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_kde {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "linux",] $($rest)*)
    };
}

#[cfg(not(feature = "kde"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_kde {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* any(),] $($rest)*)
    };
}

#[cfg(all(feature = "gtk", not(feature = "kde")))]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_gtk {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "linux",] $($rest)*)
    };
}

#[cfg(not(all(feature = "gtk", not(feature = "kde"))))]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_gtk {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* any(),] $($rest)*)
    };
}

// `win32` is Windows with the `win32` feature; `winui` is Windows without it.

#[cfg(feature = "win32")]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_win32 {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "windows",] $($rest)*)
    };
}

#[cfg(not(feature = "win32"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_win32 {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* any(),] $($rest)*)
    };
}

#[cfg(not(feature = "win32"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_winui {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* target_os = "windows",] $($rest)*)
    };
}

#[cfg(feature = "win32")]
#[doc(hidden)]
#[macro_export]
macro_rules! __platform_winui {
    ($v:ident $seen:tt $out:tt [$($p:tt)*] $($rest:tt)*) => {
        $crate::__platform!(@names $v $seen $out [$($p)* any(),] $($rest)*)
    };
}
