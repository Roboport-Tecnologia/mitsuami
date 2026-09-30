# The translations tests/l10n.rs shows. US English is the fallback.

greeting = Hello, { $name }!
files = { $count ->
    [one] { $count } file
   *[other] { $count } files
}
save = Save
search = Search
    .placeholder = Search files
window-title = Documents
size = { $bytes } bytes
done = { NUMBER($fraction, style: "percent") } done
price = { NUMBER($amount, style: "currency", currency: "USD") }
year = { NUMBER($year, useGrouping: "false") }
modified = Modified { DATETIME($date, dateStyle: "long") }
modified-at = { DATETIME($date, dateStyle: "short", timeStyle: "short") }
only-in-english = Only in English
name = Name
remember = Remember me
