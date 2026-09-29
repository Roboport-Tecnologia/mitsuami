//! The contacts screen: a search field, the list, and the selected
//! contact's details.

use mitsuami::prelude::*;

#[derive(Clone, PartialEq)]
pub struct Contact {
    pub id: u32,
    pub name: String,
    pub email: String,
    pub city: &'static str,
}

const FIRST: [&str; 20] = [
    "Ada",
    "Alan",
    "Grace",
    "Edsger",
    "Barbara",
    "Donald",
    "Frances",
    "John",
    "Margaret",
    "Ken",
    "Dennis",
    "Radia",
    "Tim",
    "Hedy",
    "Niklaus",
    "Katherine",
    "Linus",
    "Sophie",
    "Guido",
    "Anita",
];
const LAST: [&str; 25] = [
    "Lovelace",
    "Turing",
    "Hopper",
    "Dijkstra",
    "Liskov",
    "Knuth",
    "Allen",
    "Backus",
    "Hamilton",
    "Thompson",
    "Ritchie",
    "Perlman",
    "Berners-Lee",
    "Lamarr",
    "Wirth",
    "Johnson",
    "Torvalds",
    "Wilson",
    "van Rossum",
    "Borg",
    "Kay",
    "Floyd",
    "Hoare",
    "Lamport",
    "Goldberg",
];
const CITIES: [&str; 8] = ["Lisbon", "São Paulo", "Tokyo", "Oslo", "Nairobi", "Montréal", "Kyoto", "Porto"];

pub fn contacts() -> Vec<Contact> {
    (0..10_000u32)
        .map(|id| {
            let (first, last) = (FIRST[id as usize % FIRST.len()], LAST[id as usize / FIRST.len() % LAST.len()]);
            Contact {
                id,
                name: format!("{first} {last}"),
                email: format!("{}.{}{id}@example.com", first.to_lowercase(), last.to_lowercase().replace(' ', "")),
                city: CITIES[id as usize % CITIES.len()],
            }
        })
        .collect()
}

#[component]
fn ContactRow(contact: Contact) -> impl View {
    view! {
        <Column padding_x=Spacing::Md padding_y=Spacing::Sm>
            <Text>{contact.name}</Text>
            <Text text_style=TextStyle::Caption>{contact.email}</Text>
        </Column>
    }
}

/// The selected contact's details, following the selection.
#[component]
fn Details(contact: Computed<Option<Contact>>) -> impl View {
    let field = move |f: fn(&Contact) -> String| move || contact.get().as_ref().map(f).unwrap_or_default();
    view! {
        <Column gap=Spacing::Sm>
            <Text text_style=TextStyle::Title>{field(|c| c.name.clone())}</Text>
            <Text>{field(|c| c.email.clone())}</Text>
            <Text text_style=TextStyle::Caption>{field(|c| c.city.to_owned())}</Text>
        </Column>
    }
}

#[component]
pub fn Contacts() -> impl View {
    let all = contacts();
    let total = all.len();
    let query = signal(String::new());
    let shown = computed(move || {
        let query = query.get().to_lowercase();
        all.iter().filter(|c| query.is_empty() || c.name.to_lowercase().contains(&query)).cloned().collect::<Vec<_>>()
    });
    let selected = signal(Vec::<u32>::new());
    let opened = signal(None::<String>);
    let handle = ListHandle::new();
    let current = computed(move || {
        let id = selected.get().first().copied()?;
        shown.get().into_iter().find(|c| c.id == id)
    });
    let jump = handle.clone();
    view! {
        <Column gap=Spacing::Md padding=Spacing::Lg grow=1.0>
            <Row gap=Spacing::Sm align=Align::Center>
                <SearchInput placeholder="Search" a11y_label="Search" grow=1.0 @search=move |q| query.set(q)/>
                <Text text_style=TextStyle::Caption>{move || format!("{} of {total}", shown.get().len())}</Text>
            </Row>
            <Row gap=Spacing::Lg grow=1.0 shrink=1.0 basis=0>
                <List each=move || shown.get() key=|c: &Contact| c.id selected=selected handle=handle
                    estimated_row_height=40 grow=1.0 a11y_label="Contacts"
                    @activate=move |id| opened.set(Some(format!("Opened #{id}"))) let:contact>
                    <ContactRow contact=contact/>
                </List>
                <Column width=240 gap=Spacing::Md>
                    <Show when=move || current.get().is_some() fallback=|| view! { <Text>"No contact selected"</Text> }>
                        <Details contact=current/>
                    </Show>
                    <Button enabled=move || current.get().is_some()
                        @click=move || if let Some(id) = selected.get().first() { jump.scroll_to(id) }>
                        "Show in list"
                    </Button>
                    <Text text_style=TextStyle::Caption>{move || opened.get().unwrap_or_default()}</Text>
                </Column>
            </Row>
        </Column>
    }
}
