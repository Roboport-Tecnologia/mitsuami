app-title = Sprachen
language = Sprache
system-language = System
name = Name
greeting = Hallo, { $name }!
files = { $count ->
    [one] { $count } Datei
   *[other] { $count } Dateien
}
today = Heute ist { DATETIME($date, dateStyle: "full") }.
now = Es ist { DATETIME($date, timeStyle: "short") }.
size = { $bytes } Bytes
done = { NUMBER($fraction, style: "percent") } erledigt
price = Preis: { NUMBER($amount, style: "currency", currency: "EUR") }
remember = Angemeldet bleiben
plan = Tarif
plan-free = Kostenlos
plan-pro = Pro
save = Speichern
saved = { $count ->
    [one] Eine Datei gespeichert.
   *[other] { $count } Dateien gespeichert.
}
menu-help = Hilfe
about = Über Sprachen

# mitsuami's own strings: the app gives them in its languages.
mitsuami-menu-about = Über { $app }
mitsuami-menu-settings = Einstellungen …
mitsuami-menu-hide = { $app } ausblenden
mitsuami-menu-quit-app = { $app } beenden
mitsuami-menu-file = Datei
mitsuami-menu-edit = Bearbeiten
mitsuami-menu-undo = Widerrufen
mitsuami-menu-redo = Wiederholen
mitsuami-menu-cut = Ausschneiden
mitsuami-menu-copy = Kopieren
mitsuami-menu-paste = Einsetzen
mitsuami-menu-select-all = Alles auswählen
mitsuami-menu-quit = Beenden
mitsuami-main-menu = Hauptmenü
mitsuami-quit-reason = Fragt vor dem Beenden nach
mitsuami-alert-ok = OK
