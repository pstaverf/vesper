# Vesper — синтаксис (этап 0)

## Файл

UTF-8, расширение `.vx`. `#` — комментарий до конца строки. Блоки — отступами (4 пробела,
табы запрещены), после заголовка блока `:`.

## Типы данных

| Тип | Литерал | Пример |
|---|---|---|
| null | `null` | `x = null` |
| bool | `true` / `false` | `ok = true` |
| int | цифры, `_` для читаемости | `n = 1_000` |
| float | с точкой или экспонентой | `p = 19.99` |
| string | `"..."` с `\n \t \" \\ \{` и подстановкой `{путь}` | `"Привет, {msg.from.first_name}!"` |
| list | `[a, b,]` | `ids = [1, 2, 3]` |
| dict | `{"k": v,}` | `p = {"chat_id": chat}` |
| inline-кнопки | `inline[[ "Текст" -> "data" ]]` | см. ниже |
| reply-кнопки | `menu[["Да", "Нет"]]` | см. ниже |

Подстановка в строке: `{имя}`, `{имя.поле}`, `{имя[0]}`. Сложные выражения — конкатенацией `+`.

В `inline` каждая строка клавиатуры — список кнопок:

```
inline[
    ["Купить" -> "buy", "Отмена" -> "no"],
    ["Сайт" -> link "https://example.com"],
    ["Позже" -> switch "напомни"]
]
```

- `"Текст" -> "data"` — кнопка с `callback_data`;
- `"Текст" -> link "url"` — кнопка-ссылка;
- `"Текст" -> switch "запрос"` — `switch_inline_query`.

`menu[["Да", "Нет"], ["Отмена"]]` — reply-клавиатура (`resize_keyboard = true` по умолчанию).
Пустой `menu[]` = убрать клавиатуру (`ReplyKeyboardRemove`).

## Ключевые слова

`on`, `state`, `if`, `elif`, `else`, `while`, `for`, `in`, `break`, `continue`, `func`, `return`,
`and`, `or`, `not`, `true`, `false`, `null`, `bot`, `inline`, `menu`, `link`, `switch`.

Всё остальное — идентификаторы: `[A-Za-z_][A-Za-z0-9_]*` (латиница, для читаемости кода;
текст для пользователя — в строках, любые языки).

## Операторы

Приоритет от высокого к низкому:

```
.  []  ()                       доступ к полю, индексу, вызов
-  not                          унарные
*  /  %  //
+  -
<  <=  >  >=  in
==  !=
??
and
or
```

`/` всегда даёт float, `//` — целочисленное деление. `+` склеивает строки и списки.
`??` возвращает правый операнд, если левый `null`. Присваивание — только отдельный оператор `=`.

Истинность: `null`, `false`, `0`, `0.0`, `""`, `[]`, `{}` — ложь, остальное — истина.
Сравнение разных типов: `==`/`!=` работают (разные типы не равны), `<`/`>` — ошибка.
Обращение к отсутствующему полю словаря или выход за границы списка даёт `null`.

## Операторы-блоки

```
x = 10
if x > 5:
    send("много")
elif x > 0:
    send("мало")
else:
    send("ноль")

for user in users:
    send("Привет, {user.name}")

while n > 0:
    n = n - 1
    if n == 2:
        continue
    if n == 1:
        break
```

Цикл `for` обходит список (значения) или словарь (ключи).

## Функции

```
func greet(name, punct = "!"):
    return "Привет, " + name + punct

send(greet("Аня"))
```

Параметры с `=` — необязательные. Вызов с именованными аргументами: `send("ok", chat = 42)`.
Функции видят глобальные переменные, но присваивание внутри функции создаёт локальную.
Рекурсия разрешена. Область видимости блока не создаётся.

## Обработчики

```
bot "Имя бота"

on command "start":
    ...
```

| Форма | Когда срабатывает |
|---|---|
| `on command "start"` | текст вида `/start ...`; аргументы в `args` (список строк) |
| `on text` | любое текстовое сообщение |
| `on photo` / `on document` / `on video` / `on audio` / `on voice` / `on sticker` | сообщение с этим вложением |
| `on animation` / `on location` / `on contact` / `on poll` | соответствующие вложения |
| `on callback "buy"` | `callback_data == "buy"`; `on callback` — любой callback |
| `on inline` | inline-запрос |
| `on pre_checkout` | `pre_checkout_query` (платежи) |
| `on payment` | `successful_payment` |
| `on any` | любое обновление, запасной вариант |

Порядок выбора: обработчики активного `state` → глобальные обработчики в порядке объявления →
`on any`. Выполняется только первый подошедший.

Доступные в обработчике переменные:

| Имя | Значение |
|---|---|
| `upd` | обновление целиком (`Map`, все поля как есть) |
| `msg` | сообщение (`upd.message`, иначе `upd.callback_query.message`) |
| `cb` | `callback_query`, если есть |
| `chat` | `chat_id` |
| `user` | объект `from` |
| `args` | аргументы команды |
| `text` | текст сообщения или `""` |

## Состояния и данные

```
on command "name":
    set_state("ask_name")
    send("Как тебя зовут?")

state "ask_name":
    on text:
        save("name", text)
        set_state(null)
        send("Привет, {text}!")
```

- `set_state(состояние | null)` — состояние текущего пользователя (по умолчанию `null`);
- `state` — текущее состояние, доступно для чтения в любом обработчике;
- `save(key, value)` / `load(key)` / `forget(key)` — данные пользователя, хранилище задаётся
  при запуске: `vesper run bot.vx --store memory` (по умолчанию) или `--store data.json`.

## Встроенные функции

Ядро: `len`, `str`, `int`, `float`, `bool`, `type`, `keys`, `values`, `has`, `push`, `pop`,
`join`, `split`, `trim`, `lower`, `upper`, `replace`, `find`, `slice`, `range`, `random`,
`now`, `json`, `parse_json`, `log`, `exit`.

Telegram (этап 6):

| Функция | Метод Bot API |
|---|---|
| `send(text, inline = ..., menu = ..., parse_mode = ..., chat = ...)` | `sendMessage` |
| `reply(text, ...)` | `sendMessage` с `reply_parameters` |
| `edit(text, ...)` | `editMessageText` |
| `delete(id = null)` | `deleteMessage` |
| `forward(id, chat)` | `forwardMessage` |
| `send_photo(файл, caption = "", ...)` | `sendPhoto` |
| `send_document`, `send_video`, `send_audio`, `send_voice`, `send_location`, `send_contact`, `send_poll`, `send_invoice` | одноимённые методы |
| `save_photo()` / `save_document()` | сохранение присланного файла на диск |
| `answer(text = "")` | `answerCallbackQuery` |
| `answer_inline(результаты)` | `answerInlineQuery` |
| `answer_pre_checkout(ok, текст = "")` | `answerPreCheckoutQuery` |
| `typing()` | `sendChatAction` |
| `call(method, params)` | любой метод Bot API |

`файл` — строка: `http(s)://...` уходит как URL, иначе путь к локальному файлу (multipart-загрузка).

## Ошибки

```
bot.vx:12:5: ошибка: неизвестная функция 'sned'
  12 |     sned("Привет")
     |     ^^^^
```

Лексер: недопустимый символ, незакрытая строка, таб в отступе, неверный DEDENT.
Парсер: неожиданный токен, блок без отступа, `return` вне функции, `break` вне цикла.
Интерпретатор: неизвестное имя, деление на ноль, неверный тип аргумента, ошибка Bot API.

## Полная грамматика (EBNF)

```
program     = { stmt } ;
stmt        = bot_decl | handler | state_block | func_def | assign | if | while | for
            | return | break | continue | expr_stmt ;
bot_decl    = "bot" STRING ;
handler     = "on" trigger ":" block ;
trigger     = "command" STRING | "callback" STRING | "callback" | "text" | "photo"
            | "document" | "video" | "audio" | "voice" | "sticker" | "animation"
            | "location" | "contact" | "poll" | "inline" | "pre_checkout" | "payment" | "any" ;
state_block = "state" STRING ":" { handler } ;
func_def    = "func" IDENT [ "(" [ param { "," param } ] ")" ] ":" block ;
param       = IDENT [ "=" expr ] ;
block       = INDENT { stmt } DEDENT ;
assign      = postfix "=" expr ;
if          = "if" expr ":" block { "elif" expr ":" block } [ "else" ":" block ] ;
while       = "while" expr ":" block ;
for         = "for" IDENT "in" expr ":" block ;
return      = "return" [ expr ] ;
expr_stmt   = expr ;
expr        = or_expr ;
or_expr     = and_expr { "or" and_expr } ;
and_expr    = coalesce { "and" coalesce } ;
coalesce    = equality { "??" equality } ;
equality    = compare { ("==" | "!=") compare } ;
compare     = add { ("<" | "<=" | ">" | ">=" | "in") add } ;
add         = mul { ("+" | "-") mul } ;
mul         = unary { ("*" | "/" | "//" | "%") unary } ;
unary       = [ "-" | "not" ] postfix ;
postfix     = primary { "." IDENT | "[" expr "]" | call_args } ;
call_args   = "(" [ arg { "," arg } [ "," ] ] ")" ;
arg         = [ IDENT "=" ] expr ;
primary     = NUMBER | STRING | "true" | "false" | "null" | IDENT
            | "(" expr ")" | list | dict | inline_kb | menu_kb ;
list        = "[" [ expr { "," expr } [ "," ] ] "]" ;
dict        = "{" [ pair { "," pair } [ "," ] ] "}" ;
pair        = ( STRING | IDENT ) ":" expr ;
inline_kb   = "inline" "[" [ row { "," row } [ "," ] ] "]" ;
row         = "[" [ button { "," button } ] "]" ;
button      = STRING "->" ( STRING | "link" expr | "switch" expr ) ;
menu_kb     = "menu" "[" [ row_str { "," row_str } [ "," ] ] "]" ;
row_str     = "[" [ STRING { "," STRING } ] "]" ;
```
