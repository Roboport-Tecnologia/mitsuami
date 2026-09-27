//! The screen: the same on every platform. Each of its three custom
//! widgets comes from one platform and is native wherever the platform has
//! the control; elsewhere it's built ad hoc from the platform's widgets,
//! drawn or composed. Labels say which.

use mitsuami::prelude::*;

use crate::lock::{Lock, LockEvent, LockProps};
use crate::pips_pager::{PipsPager, PipsPagerEvent, PipsPagerProps};
use crate::rating::{Rating, RatingEvent, RatingProps};
use crate::store::Review;

/// What the pager pages through: where each widget comes from.
pub const NOTES: [&str; 3] = [
    "The lock is GTK's GtkLockButton; elsewhere it's a plain button.",
    "The rating is macOS's NSLevelIndicator and WinUI's RatingControl; GTK builds it from star buttons, like GNOME Software.",
    "The pager is WinUI's PipsPager; elsewhere it's drawn.",
];

/// "Rating (native)" where the widget is the platform's own control,
/// "Rating" where it's a stand-in.
pub fn caption<W: Render>(name: &str) -> String {
    if W::renderer().is_native() { format!("{name} (native)") } else { name.to_string() }
}

#[component]
pub fn Screen() -> impl View {
    let review = use_store::<Review>();
    let page = signal(0u8);
    view! {
        <Column gap=Spacing::Lg>
            <Text text_style=TextStyle::Title>"Rate mitsuami"</Text>
            <Grid
                columns=[Track::MaxContent, Track::Size(1.fr())]
                column_gap=Spacing::Md
                row_gap=Spacing::Md
                align=Align::Center
            >
                <Text>{caption::<Lock>("Lock")}</Text>
                <Row>
                    <Lock
                        props=move || LockProps { locked: review.locked.get() }
                        a11y_label="Lock"
                        @event=move |event| match event {
                            LockEvent::UnlockRequested => review.unlock(),
                            LockEvent::LockRequested => review.lock(),
                        }
                    />
                </Row>
                <Text>{caption::<Rating>("Rating")}</Text>
                <Row>
                    <Rating
                        props=move || RatingProps { editable: !review.locked.get(), ..RatingProps::new(review.stars.get()) }
                        a11y_label="Rating"
                        @event=move |RatingEvent::Changed(stars)| review.rate(*stars)
                    />
                </Row>
                <Text>{caption::<PipsPager>("Page")}</Text>
                <Row>
                    <PipsPager
                        props=move || PipsPagerProps { count: NOTES.len() as u8, selected: page.get() }
                        a11y_label="Page"
                        @event=move |PipsPagerEvent::Selected(p)| page.set(*p)
                    />
                </Row>
            </Grid>
            <Text text_style=TextStyle::Caption>{move || NOTES[page.get() as usize].to_string()}</Text>
            <Row gap=Spacing::Md align=Align::Center>
                <Text grow=1.0>{move || review.status()}</Text>
                <Button
                    role=ButtonRole::Default
                    enabled=move || review.can_submit()
                    @click=move || review.submit()
                >"Submit"</Button>
            </Row>
        </Column>
    }
}
