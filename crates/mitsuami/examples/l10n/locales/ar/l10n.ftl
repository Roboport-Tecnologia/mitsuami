app-title = اللغات
language = اللغة
system-language = النظام
name = الاسم
greeting = مرحبًا، { $name }!
files = { $count ->
    [zero] لا ملفات
    [one] ملف واحد
    [two] ملفان
    [few] { $count } ملفات
    [many] { $count } ملفًا
   *[other] { $count } ملف
}
today = اليوم { DATETIME($date, dateStyle: "full") }.
now = الساعة { DATETIME($date, timeStyle: "short") }.
size = { $bytes } بايت
done = اكتمل { NUMBER($fraction, style: "percent") }
price = السعر: { NUMBER($amount, style: "currency", currency: "EUR") }
remember = تذكرني
plan = الخطة
plan-free = مجانية
plan-pro = احترافية
save = حفظ
saved = { $count ->
    [one] تم حفظ ملف واحد.
    [two] تم حفظ ملفين.
   *[other] تم حفظ { $count } ملف.
}
menu-help = مساعدة
about = حول اللغات

# mitsuami's own strings: the app gives them in its languages.
mitsuami-menu-about = حول { $app }
mitsuami-menu-settings = الإعدادات…
mitsuami-menu-hide = إخفاء { $app }
mitsuami-menu-quit-app = إنهاء { $app }
mitsuami-menu-file = ملف
mitsuami-menu-edit = تحرير
mitsuami-menu-undo = تراجع
mitsuami-menu-redo = إعادة
mitsuami-menu-cut = قص
mitsuami-menu-copy = نسخ
mitsuami-menu-paste = لصق
mitsuami-menu-select-all = تحديد الكل
mitsuami-menu-quit = إنهاء
mitsuami-main-menu = القائمة الرئيسية
mitsuami-quit-reason = يسأل قبل الإنهاء
mitsuami-alert-ok = حسنًا
