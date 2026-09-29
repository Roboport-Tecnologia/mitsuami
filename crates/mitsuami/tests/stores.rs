//! Stores, resources and actions: app-wide state, and async work as
//! signals, on the test clock.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const SECOND: Duration = Duration::from_secs(1);

// ----------------------------------------------------------------- stores

#[derive(Clone, Copy)]
struct Cart {
    items: Signal<u32>,
}

impl Store for Cart {
    fn create() -> Cart {
        Cart { items: signal(0) }
    }
}

impl Cart {
    fn add(&self) {
        self.items.update(|n| *n += 1);
    }
}

#[component]
fn AddButton(#[prop(into)] label: String) -> impl View {
    let cart = use_store::<Cart>();
    view! { <Button @click=move || cart.add()>{label}</Button> }
}

#[component]
fn CartBadge() -> impl View {
    let cart = use_store::<Cart>();
    view! { <Text>{move || format!("{} in the cart", cart.items.get())}</Text> }
}

#[mitsuami_test::test]
async fn views_share_the_apps_store(app: TestApp) {
    app.mount(|| {
        view! {
            <Column>
                <AddButton label="Add an apple"/>
                <AddButton label="Add a pear"/>
                <CartBadge/>
            </Column>
        }
    });
    app.get_by_role(Role::Button, "Add an apple").click().await;
    app.get_by_role(Role::Button, "Add a pear").click().await;
    app.expect(by_text("2 in the cart")).to_exist().await;
}

#[mitsuami_test::test]
async fn a_store_outlives_the_views_that_use_it(app: TestApp) {
    let open = signal(true);
    app.mount(move || {
        view! {
            <Column>
                <Show when=open><AddButton label="Add"/></Show>
                <CartBadge/>
            </Column>
        }
    });
    app.get_by_role(Role::Button, "Add").click().await;
    open.set(false);
    app.settle().await;
    open.set(true);
    app.get_by_role(Role::Button, "Add").click().await;
    app.expect(by_text("2 in the cart")).to_exist().await;
}

#[mitsuami_test::test]
async fn a_provided_store_takes_precedence(app: TestApp) {
    app.provide(Cart { items: signal(41) });
    app.mount(|| view! { <Column><AddButton label="Add"/><CartBadge/></Column> });
    app.get_by_role(Role::Button, "Add").click().await;
    app.expect(by_text("42 in the cart")).to_exist().await;
}

/// A store whose data loads in the background, in the store's own scope.
#[derive(Clone, Copy)]
struct Catalog {
    products: Resource<Vec<&'static str>, String>,
}

impl Store for Catalog {
    fn create() -> Catalog {
        Catalog {
            products: resource(|| async {
                sleep(SECOND).await;
                Ok(vec!["apples", "pears"])
            }),
        }
    }
}

#[mitsuami_test::test]
async fn stores_can_load_data(app: TestApp) {
    let open = signal(true);
    app.mount(move || {
        view! {
            <Show when=open>
                {
                    let catalog = use_store::<Catalog>();
                    view! { <Text>{move || catalog.products.data().map_or("…".to_string(), |p| p.join(", "))}</Text> }
                }
            </Show>
        }
    });
    app.expect(by_text("…")).to_exist().await;
    // The view that created the store goes away; the store keeps loading.
    open.set(false);
    app.advance(SECOND).await;
    open.set(true);
    app.expect(by_text("apples, pears")).to_exist().await;
}

/// A store whose action saves in the background, as a rename does.
#[derive(Clone, Copy)]
struct Drafts {
    saved: Signal<u32>,
}

impl Store for Drafts {
    fn create() -> Drafts {
        Drafts { saved: signal(0) }
    }
}

impl Drafts {
    fn save(&self) {
        let drafts = *self;
        self.spawn(async move {
            sleep(SECOND).await;
            drafts.saved.update(|n| *n += 1);
        });
    }
}

/// A dialog's button calls the action and the dialog closes: the store's
/// work goes on, in the store's scope, where `spawn_local` would have tied
/// it to the dialog.
async fn save_from_a_closing_view(app: &TestApp) {
    let open = signal(true);
    app.mount(move || {
        let drafts = use_store::<Drafts>();
        view! {
            <Column>
                <Show when=open>
                    <Button @click=move || {
                        use_store::<Drafts>().save();
                        open.set(false);
                    }>"Save"</Button>
                </Show>
                <Text>{move || format!("{} saved", drafts.saved.get())}</Text>
            </Column>
        }
    });
    app.get_by_role(Role::Button, "Save").click().await;
    app.expect(by_role(Role::Button, "Save")).not_to_exist().await;
    app.advance(SECOND).await;
    app.expect(by_text("1 saved")).to_exist().await;
}

#[mitsuami_test::test]
async fn a_stores_work_outlives_the_view_that_started_it(app: TestApp) {
    save_from_a_closing_view(&app).await;
}

#[mitsuami_test::test]
async fn a_provided_stores_work_runs_where_it_was_provided(app: TestApp) {
    app.provide(Drafts { saved: signal(0) });
    save_from_a_closing_view(&app).await;
}

// -------------------------------------------------------------- resources

/// "Loading", the data, or the error, with the data kept while reloading.
fn describe(user: Resource<String, String>) -> impl Fn() -> String {
    move || {
        let data = user.data().unwrap_or_else(|| "nobody".into());
        match (user.loading(), user.error()) {
            (true, _) => format!("{data} (loading)"),
            (false, Some(error)) => format!("{data} ({error})"),
            (false, None) => data,
        }
    }
}

#[mitsuami_test::test]
async fn a_resource_loads_and_refetches(app: TestApp) {
    let fetches = Rc::new(Cell::new(0));
    let count = fetches.clone();
    let user = Rc::new(Cell::new(None::<Resource<String, String>>));
    let handle = user.clone();
    app.mount(move || {
        let resource = resource(move || {
            let n = count.get() + 1;
            count.set(n);
            async move {
                sleep(SECOND).await;
                Ok(format!("Ada #{n}"))
            }
        });
        handle.set(Some(resource));
        view! { <Text>{describe(resource)}</Text> }
    });
    let user = user.get().unwrap();
    app.expect(by_text("nobody (loading)")).to_exist().await;

    app.advance(SECOND).await;
    app.expect(by_text("Ada #1")).to_exist().await;

    user.refetch();
    app.expect(by_text("Ada #1 (loading)")).to_exist().await;
    app.advance(SECOND).await;
    app.expect(by_text("Ada #2")).to_exist().await;
    assert_eq!(fetches.get(), 2);
}

#[mitsuami_test::test]
async fn a_new_source_cancels_the_fetch_in_flight(app: TestApp) {
    let id = signal(1u32);
    let completed = Rc::new(Cell::new(0));
    let done = completed.clone();
    app.mount(move || {
        let user = resource_on(
            move || id.get(),
            move |id| {
                let done = done.clone();
                async move {
                    sleep(SECOND).await;
                    done.set(done.get() + 1);
                    Ok(format!("user {id}"))
                }
            },
        );
        view! { <Text>{describe(user)}</Text> }
    });

    app.advance(SECOND / 2).await;
    id.set(2);
    app.settle().await;
    app.advance(SECOND / 2).await;
    // The first fetch would have finished now, but it was cancelled.
    app.expect(by_text("nobody (loading)")).to_exist().await;

    app.advance(SECOND / 2).await;
    app.expect(by_text("user 2")).to_exist().await;
    assert_eq!(completed.get(), 1);
}

#[mitsuami_test::test]
async fn an_error_keeps_the_last_data(app: TestApp) {
    let fail = Rc::new(Cell::new(false));
    let failing = fail.clone();
    let user = Rc::new(Cell::new(None::<Resource<String, String>>));
    let handle = user.clone();
    app.mount(move || {
        let failing = failing.clone();
        let resource = resource(move || {
            let fail = failing.get();
            async move { if fail { Err("offline".to_string()) } else { Ok("Ada".to_string()) } }
        });
        handle.set(Some(resource));
        view! { <Text>{describe(resource)}</Text> }
    });
    let user = user.get().unwrap();
    app.expect(by_text("Ada")).to_exist().await;

    fail.set(true);
    user.refetch();
    app.expect(by_text("Ada (offline)")).to_exist().await;

    fail.set(false);
    user.refetch();
    app.expect(by_text("Ada")).to_exist().await;
}

#[mitsuami_test::test]
async fn disposing_the_view_cancels_its_fetch(app: TestApp) {
    let open = signal(true);
    let completed = Rc::new(Cell::new(false));
    let done = completed.clone();
    app.mount(move || {
        let done = done.clone();
        view! {
            <Show when=open>
                {
                    let done = done.clone();
                    let user = resource(move || {
                        let done = done.clone();
                        async move {
                            sleep(SECOND).await;
                            done.set(true);
                            Ok::<_, String>("Ada".to_string())
                        }
                    });
                    view! { <Text>{describe(user)}</Text> }
                }
            </Show>
        }
    });
    open.set(false);
    app.advance(2 * SECOND).await;
    assert!(!completed.get());
    assert_eq!(app.ui().pending_tasks(), 0);
}

// ---------------------------------------------------------------- actions

#[mitsuami_test::test]
async fn an_action_tracks_pending_and_keeps_the_latest_value(app: TestApp) {
    app.mount(|| {
        let draft = signal(0u32);
        let save = action(|n: u32| async move {
            // Earlier saves take longer, so they finish last.
            sleep(SECOND * (4 - n)).await;
            format!("saved {n}")
        });
        view! {
            <Column>
                <Button enabled=move || !save.pending() @click=move || {
                    draft.update(|n| *n += 1);
                    save.dispatch(draft.get_untracked());
                }>"Save"</Button>
                <Button @click=move || {
                    draft.update(|n| *n += 1);
                    save.dispatch(draft.get_untracked());
                }>"Save anyway"</Button>
                <Text>{move || match (save.pending(), save.value()) {
                    (true, _) => "saving".to_string(),
                    (false, value) => value.unwrap_or_else(|| "not saved".into()),
                }}</Text>
            </Column>
        }
    });
    app.expect(by_text("not saved")).to_exist().await;

    app.get_by_role(Role::Button, "Save").click().await;
    app.expect(by_text("saving")).to_exist().await;
    app.expect(by_role(Role::Button, "Save")).to_be_disabled().await;

    // Two saves in flight: 1 takes 3s, 2 takes 2s.
    app.get_by_role(Role::Button, "Save anyway").click().await;
    app.advance(2 * SECOND).await;
    app.expect(by_text("saving")).to_exist().await;

    app.advance(SECOND).await;
    app.expect(by_text("saved 2")).to_exist().await;
    app.expect(by_role(Role::Button, "Save")).to_be_enabled().await;
}

mitsuami_test::main!();
