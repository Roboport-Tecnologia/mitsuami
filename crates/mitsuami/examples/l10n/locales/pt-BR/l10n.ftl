app-title = Idiomas
language = Idioma
system-language = Sistema
name = Nome
greeting = Olá, { $name }!
files = { $count ->
    [one] { $count } arquivo
   *[other] { $count } arquivos
}
today = Hoje é { DATETIME($date, dateStyle: "full") }.
now = Hora: { DATETIME($date, timeStyle: "short") }.
size = { $bytes } bytes
done = { NUMBER($fraction, style: "percent") } concluído
price = Preço: { NUMBER($amount, style: "currency", currency: "EUR") }
remember = Lembrar de mim
plan = Plano
plan-free = Gratuito
plan-pro = Pro
save = Salvar
saved = { $count ->
    [one] Um arquivo salvo.
   *[other] { $count } arquivos salvos.
}
menu-help = Ajuda
about = Sobre o Idiomas

# mitsuami's own strings: the app gives them in its languages.
mitsuami-menu-about = Sobre o { $app }
mitsuami-menu-settings = Ajustes…
mitsuami-menu-hide = Ocultar { $app }
mitsuami-menu-quit-app = Encerrar { $app }
mitsuami-menu-file = Arquivo
mitsuami-menu-edit = Editar
mitsuami-menu-undo = Desfazer
mitsuami-menu-redo = Refazer
mitsuami-menu-cut = Recortar
mitsuami-menu-copy = Copiar
mitsuami-menu-paste = Colar
mitsuami-menu-select-all = Selecionar Tudo
mitsuami-menu-quit = Sair
mitsuami-main-menu = Menu principal
mitsuami-quit-reason = Perguntando antes de encerrar
mitsuami-alert-ok = OK
