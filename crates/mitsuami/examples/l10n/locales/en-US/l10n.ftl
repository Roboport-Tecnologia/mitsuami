app-title = Languages
language = Language
system-language = System
name = Name
greeting = Hello, { $name }!
files = { $count ->
    [one] { $count } file
   *[other] { $count } files
}
today = Today is { DATETIME($date, dateStyle: "full") }.
now = The time is { DATETIME($date, timeStyle: "short") }.
size = { $bytes } bytes
done = { NUMBER($fraction, style: "percent") } done
price = Price: { NUMBER($amount, style: "currency", currency: "EUR") }
remember = Remember me
plan = Plan
plan-free = Free
plan-pro = Pro
save = Save
saved = { $count ->
    [one] One file saved.
   *[other] { $count } files saved.
}
menu-help = Help
about = About Languages
