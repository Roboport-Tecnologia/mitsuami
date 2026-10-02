//! Testing for mitsuami apps: one API for headless integration tests and
//! end-to-end tests on the native backends.
//!
//! ```ignore
//! use mitsuami_test::prelude::*;
//!
//! #[mitsuami_test::test]
//! async fn increments(app: TestApp) {
//!     app.mount(|| Counter::new().initial(0));
//!     app.get_by_role(Role::Button, "Increment").click().await;
//!     app.expect(by_text("Count: 1")).to_be_visible().await;
//! }
//!
//! mitsuami_test::main!();
//! ```
//!
//! Stories render a view in a given state, and compare a capture of it
//! with a baseline for every size and variant:
//!
//! ```ignore
//! #[mitsuami_test::story(sizes = [(320, fit)], variants = [Light, Dark])]
//! fn counter_big_number() -> impl View {
//!     Counter::new().initial(42)
//! }
//! ```
//!
//! Test targets use `harness = false` (a `[[test]]` with
//! `harness = false` in Cargo.toml): native UI must own the main thread, so
//! mitsuami runs tests itself. `cargo test` runs them headless;
//! `MITSUAMI_NATIVE=1 cargo test` runs them on this machine's native
//! backend. Controls are found through the
//! accessibility tree, and driven with accessibility actions or synthesized
//! input, the way users and assistive technology reach them.

mod app;
mod driver;
mod exec;
mod format;
mod locator;
mod query;
mod runner;
mod snapshot;
mod story;
mod visual;

pub use app::TestApp;
pub use driver::Mode;
pub use locator::{Expectation, Locator};
pub use mitsuami_headless::{
    FakeServicesHandle, Pending, PendingAlert, PendingLaunch, PendingOpen, PendingSave, PendingTrash,
};
pub use mitsuami_test_macros::{story, test};
pub use query::{Query, by_label, by_role, by_test_id, by_text, visible_text};
pub use story::Variant;
pub use visual::VisualOptions;

pub mod prelude {
    pub use crate::{Expectation, Locator, Query, TestApp, VisualOptions, by_label, by_role, by_test_id, by_text};
    pub use mitsuami_core::{Key, Rect, Role, Size};
}

/// Defines `main` for a `harness = false` test target.
#[macro_export]
macro_rules! main {
    () => {
        fn main() {
            $crate::__private::run_main()
        }
    };
}

#[doc(hidden)]
pub mod __private {
    use std::future::Future;
    use std::pin::Pin;

    pub use inventory;

    pub use crate::runner::run_main;

    pub type TestFuture = Pin<Box<dyn Future<Output = ()>>>;
    pub type StoryFuture<'a> = Pin<Box<dyn Future<Output = ()> + 'a>>;

    pub struct TestCase {
        pub name: &'static str,
        pub manifest_dir: &'static str,
        /// Depends on the headless backend (fake metrics, simulated
        /// system changes): skipped with `--native`.
        pub headless_only: bool,
        pub run: fn(crate::TestApp) -> TestFuture,
    }

    inventory::collect!(TestCase);

    pub struct StoryCase {
        pub name: &'static str,
        pub manifest_dir: &'static str,
        /// Window content sizes, in logical units: a width, and a height
        /// or `None` to fit the content's.
        pub sizes: &'static [(f32, Option<f32>)],
        pub variants: &'static [crate::Variant],
        /// How its captures are compared with their baselines.
        pub visual: fn() -> crate::VisualOptions,
        /// Mounts the story and plays its script, if any.
        pub run: for<'a> fn(&'a crate::TestApp) -> StoryFuture<'a>,
    }

    inventory::collect!(StoryCase);
}
