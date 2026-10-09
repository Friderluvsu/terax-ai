# План доработки Terax (личный форк)

Версия 1, 2026-10-09. Основа: клон `crynta/terax-ai` 0.9.0-beta, HEAD `3301de2`. Доказательная база (файлы и строки) в `fork-plan/scouts/*.md`; карточки ниже ссылаются на них, чтобы исполнителю не приходилось заново исследовать код.

Как читать: раздел 1 содержит принятые решения, раздел 2 правила для моделей и экономии, раздел 3 протокол выполнения, раздел 4 волны, раздел 5 карточки задач. Карточки самодостаточны: их текст целиком уходит в промпт субагента.

---

## 0. Что можно попробовать уже сейчас

В установленном Terax новая вкладка по **Cmd+Shift+T** открывается в режиме Blocks. Там строка ввода сделана как редактор: клик ставит курсор, выделение мышью, Cmd+Z, Option+стрелки, автоподсказки из истории (стрелка вправо принимает). Ограничения: такую вкладку нельзя разделить на панели, а внутри `ssh` строка ввода отключается (на сервере нет shell-integration; это чинит E3-T08).

---

## 1. Принятые решения

Взяты рекомендованные варианты. Любое можно поменять до старта соответствующей волны.

| # | Вопрос | Решение | Альтернатива |
|---|---|---|---|
| D1 | Жест для клика по курсору в обычных вкладках | Простой клик по строке ввода, когда есть shell-integration (OSC 133); Option+клик работает везде как запасной вариант (ssh, REPL). Ссылки внутри редактируемой строки открываются по Cmd+клику, вне её как раньше. Есть переключатель в настройках | Только Option+клик, как в iTerm2 и Ghostty |
| D2 | Blocks по умолчанию | Добавить настройку "режим новых вкладок", по умолчанию Classic. Включишь сам, если понравится | Сразу Blocks по умолчанию |
| D3 | Вид HTML | Вкладка редактора с режимами Code / Split / Preview (буфер и undo не теряются, можно смотреть рядом) | Отдельный тип вкладки, как у Markdown |
| D4 | SSH-транспорт | Системный OpenSSH: `ssh -tt` в обычном PTY, свой ControlMaster, SFTP через `russh-sftp` поверх `ssh -s sftp` (+~167 KB). macOS и Linux; на Windows компилируется заглушка | Встроенный `russh` (+~1.6 MB, работает на Windows, но свой UI паролей и разбор ssh_config) |
| D5 | Shell-integration на сервере | Загружать скрипты Terax в `~/.cache/terax/shell-integration` на сервере через уже открытое соединение (как сделано для WSL). Можно отключить в настройках | Без интеграции: на сервере нет Blocks, клика по строке и cwd |
| D6 | AI-инструменты в SSH-пространстве | Только чтение по умолчанию; запись можно включить настройкой. Команды AI на сервере в v1 недоступны вообще (E3-T01 делает их ошибкой; удалённый `run_command` это отдельная будущая задача) | Полный доступ |
| D7 | Русский интерфейс | Не делаем сейчас (это ~110 файлов, большой конфликт с апстримом). Делаем хоткеи, работающие на русской раскладке (E4-T02) | Полная локализация (набросок в E4-T10) |
| D8 | Своя сборка | Dev: `app.crynta.terax.dev` / "Terax Dev". Релиз: `dev.terax.fork` / "Terax". Апдейтер в форке выключен. Keychain общий с установленным Terax (одни и те же API-ключи) | Отдельный keychain-сервис |
| D9 | Cmd+G в редакторе | Внутри вкладки редактора Cmd+G отдаётся CodeMirror (найти следующее); вне редактора остаётся Source Control | Оставить как есть |
| D11 | `.command` из Finder | Запускать в новой терминальной вкладке в папке скрипта; карантин спрашивает подтверждение; права молча не меняются. Клик в проводнике Terax по-прежнему открывает редактор, запуск через "Run in Terminal" и кнопку Run | Всегда открывать в редакторе |
| D10 | Где хранить план | Закоммитить `fork-plan/` в ветку `fork/main`: worktree-агенты не видят неотслеживаемых файлов | Ссылаться по абсолютному пути |

---

## 2. Модели и правила экономии

| Модель | Когда | Усилие |
|---|---|---|
| **Haiku 5.5** | Механика по готовому шаблону в том же файле: настройки и переключатели, таблицы хоткеев, конфиги, скрипты, docs, добавить кейсы в табличный тест | low |
| **Sonnet 5.5** | Обычные фичи в 1-3 модулях с ясным дизайном: компоненты, хуки, чистые функции с тестами, Tauri-команда по образцу соседней, проводка через существующие швы | medium |
| **Opus 5.5** | Безопасность, конкурентность, сквозные рефакторинги: SSH-транспорт и ControlMaster, SFTP, fail-closed рефакторинг окружений, обработчик `terax-preview://`, атомарная запись на сервер. Плюс ревью всех задач, задевающих IPC, fs, spawn и AI-инструменты | high |

Правила, чтобы не жечь ресурсы:

1. Задачи меньше ~15 минут работы делает главный агент сам, без субагента (например E0-T01).
2. Haiku-задачи не ревьюятся отдельным агентом: их проверяет гейт и слияние.
3. Ревьюер читает только diff ветки задачи и карточку, не весь код. Ревью есть только там, где это указано в карточке.
4. Исправления по ревью: максимум один раунд, тем же уровнем модели; дальше решает главный агент.
5. В промпт исполнителя идут карточка и путь к нужному разделу отчёта, а не просьба "изучи код".
6. Полный набор тестов гоняется на слиянии волны; внутри задачи нужны только гейты из карточки.
7. Rust-задач параллельно не больше двух (каждой нужен свой `CARGO_TARGET_DIR`, холодная сборка ~3 мин и ~4 GB).

Итого по ядру (W0-W6 без опциональных): Haiku 6, Sonnet 25, Opus 6, плюс E0-T01 у главного агента; ревью Opus 14, ревью Sonnet 2. Опциональные: Haiku 2, Sonnet 10, Opus 2.

---

## 3. Протокол выполнения

Каждая волна запускается одним workflow. Для каждой задачи последовательно: исполнитель, затем ревью (если указано), затем один раунд исправлений (если ревью нашло critical/major). Слияние делает главный агент.

**Окружение.** Любая команда node/pnpm/cargo начинается с `source fork-plan/tools/env.sh` (после E0-T02 это `source scripts/fork/agent-env.sh`).

**Изоляция.**
- Каждый исполнитель работает в своём git worktree на ветке `task/<ID>` от `fork/main`.
- В worktree первым делом `pnpm install --frozen-lockfile` (~30 с, store уже прогрет).
- Rust-задачам назначается слот `CARGO_TARGET_DIR=$HOME/Library/Caches/terax-fork/target-A` или `target-B`; слоты переиспользуются между волнами и остаются тёплыми.
- `src-tauri/target` основного дерева только для главного агента.
- `pnpm tauri dev` (порт 1420) запускает только главный агент.

**Гейты.**
- FE: `pnpm lint && pnpm check-types && pnpm test` (база: 0 ошибок lint, 90 предупреждений, не больше; 1209 тестов).
- RS: `cd src-tauri && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked` (база 373 passed).
- SIZE: `pnpm build && pnpm size` (общий JS 1.47 из 1.5 MB gzip; всё новое только через `lazy()`).
- Форматировать только свои файлы: `pnpm exec biome format --write <files>`, `rustfmt <files>`. Форматировать весь репозиторий нельзя.

**Конвенции репозитория (TERAX.md).**
- Без em-dash и эмодзи.
- Минимум комментариев.
- Импорты через `@/`.
- Только pnpm.
- Логика в чистых функциях, команды и компоненты тонкие.
- Для изменений в spawn, workspace auth, fs, IPC и AI-инструментах обязателен тест, фиксирующий инвариант.
- Новый код форка класть в новые модули, правки горячих файлов делать минимальными.

**Шаблон промпта исполнителя.**

```text
Ты реализуешь задачу <ID> в форке Terax. Рабочая копия: <worktree>, ветка task/<ID> от fork/main.
Сначала: source scripts/fork/agent-env.sh && pnpm install --frozen-lockfile. Rust-слот: CARGO_TARGET_DIR=<slot>.
Карточка задачи (выполни целиком, ничего вне её не меняй):
<текст карточки>
Справка по коду: <путь к отчёту и раздел>.
Перед завершением: прогони гейты из карточки, отформатируй только свои файлы, сделай один коммит
"<ID>: <title>" с трейлером Co-Authored-By. Верни: что сделано, список файлов, результаты гейтов,
что проверить руками, известные риски.
```

**Шаблон промпта ревьюера.**

```text
Ревью задачи <ID>. Прочитай `git diff fork/main...task/<ID>` и карточку. Ищи: нарушения критериев приёмки,
дыры безопасности (инъекции аргументов, обход путей, fail-open, утечки через IPC), потерю данных, гонки,
утечки процессов, eager-импорты тяжёлого кода, отсутствие теста на инвариант. Только находки с severity
critical/major/minor и конкретным исправлением. Не переписывай код.
```

**Слияние (главный агент).**
1. Задача вливается в `fork/main` сразу после того, как закончила (и прошла ревью, если оно есть): `git merge --no-ff task/<ID>` и FE/RS-гейт по затронутым частям. Порядок слияния внутри волны берётся из таблицы волн.
2. Задача, которая зависит от задачи той же волны, стартует только после слияния зависимости и ответвляется от обновлённого `fork/main`. Пример: E1-T06 ждёт слияния E1-T04 и E1-T05.
3. Конфликты в горячих файлах (раздел 6) разрешает главный агент.
4. После волны полный FE + RS гейт, SIZE если менялся фронт, запуск `pnpm fork:dev` и список ручных проверок для тебя.

---

## 4. Волны

| Волна | Цель | Задачи (порядок слияния) | Параллельно | Выход из волны |
|---|---|---|---|---|
| W0 Фундамент | Безопасная песочница для форка, агенты видят TERAX.md | E0-T01 (главный), E0-T02, E0-T03, E0-T06, E0-T05, E0-T04 (опц.) | T02 и T06 параллельно; T03 Rust в слоте A; T05 после T03 | `pnpm fork:dev` стартует как "Terax Dev"; `scripts/fork/release.sh` собирает "Terax Fork.app"; апдейтер не активен |
| W1 Быстрая польза | Видимые улучшения за день | Партия 1: E4-T01, E1-T02, E2-T01 (готово). Партия 2: E1-T01, E3-T06, E1-T10 | Все 5; Rust: E2-T01 слот A, E3-T06 слот B; E1-T02 гоняет только FE, Rust-тест скриптов запускает главный агент при слиянии | Ручные проверки W1 (раздел 8) |
| W2 Три трека | Клик в Classic, HTML-превью, подготовка к SSH | Трек A: E1-T04 и E1-T05 параллельно, затем E1-T06. Трек C: E3-T01, затем E3-T02. Трек B: E2-T02, затем E2-T03, затем E2-T04. E3-T01 вливается раньше E2-T02 (обе правят `fs/watch.rs`; E2-T02 при слиянии переводится на `local_path`) | Треки параллельно; Rust: E3-T01 слот A, E2-T02 слот B | Клик работает в zsh/bash/fish; HTML открывается в Split и обновляется при сохранении; гейты зелёные |
| W3 Доводка | Хоткеи Option, прокрутка в less, Markdown, превью SVG, провайдер ФС | E1-T03, E1-T07, E2-T05, E2-T06, E3-T04, E3-T05, затем E3-T03, E4-T02 (если ручная проверка раскладки показала проблему) | E1 и E2 параллельно с E3-T04; E3-T03 после E3-T04 | Ручные проверки W3 |
| W4 SSH-терминал | Удалённый терминал с Blocks и кликом | E3-T07, затем E3-T08 и E3-T09, затем E3-T08b | T08 и T09 параллельно (слоты A и B) | "SSH: <alias> (remote shell)" для OrbStack-машины и `oracle` открывает SSH-space; cwd отслеживается, Blocks и клик работают на сервере |
| W5 SSH-файлы | Дерево и редактирование файлов сервера | E3-T10, затем E3-T11 и E3-T13, затем E3-T12 | T11 и T13 параллельно | Открыть, отредактировать и сохранить файл на сервере; поиск по файлам на Linux и macOS-хосте |
| W6 SSH-интерфейс | Удобный Remote-SSH | E3-T14, E3-T15, E3-T16, E4-T05 | T14 и T15 параллельно, T16 последней | Полный сценарий из `docs/fork/ssh-qa.md`; релизная сборка установлена |
| W7 По желанию | Опции | E3-T17..T20, E4-T03, E4-T04, E4-T06..T09, E1-T08, E2-T07, E1-T09 | По выбору | По карточкам |

Перед W4 тебе нужно создать тестовую Linux-машину в OrbStack и проверить, что `ssh` до неё работает (шаг U2 в разделе 8).

---

## 5. Карточки задач

Сокращения: **FE**, **RS**, **SIZE** это гейты из раздела 3. "Отчёт" это `fork-plan/scouts/<файл>`.

### E0. Фундамент

#### E0-T01 · Git для форка
- Исполнитель: главный агент, без субагента · Размер S · Зависит: нет
- Шаги:
  1. `git remote rename origin upstream`.
  2. Если у тебя есть свой форк на GitHub, добавить его как `origin` (решаешь ты; пушить только по твоей команде).
  3. `git switch -c fork/main`.
  4. Закоммитить `fork-plan/` (D10).
  5. Задачи живут в ветках `task/<ID>`; worktree в `../terax_worktrees/<ID>`.
- Обновление из апстрима: раз в 1-2 недели `git fetch upstream && git merge upstream/main` в `fork/main`, затем полный гейт. Конфликтные места заранее перечислены в разделе 6.
- Готово: `git branch` показывает `fork/main`, план закоммичен.

#### E0-T02 · Память агентов: CLAUDE.md и FORK.md
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T01
- Файлы: `CLAUDE.md`, `AGENTS.md`, новый `FORK.md`, новый `scripts/fork/agent-env.sh` (копия `fork-plan/tools/env.sh` с путём к pnpm-шиму `scripts/fork/bin/pnpm`).
- Шаги:
  1. `CLAUDE.md` заменить на две строки импорта: `@TERAX.md` и `@FORK.md`. Сейчас там просто текст "TERAX.md", и Claude Code не подгружает архитектурный документ.
  2. `AGENTS.md` оставить ссылкой на TERAX.md и добавить строку про FORK.md.
  3. В `FORK.md` (до 60 строк, без em-dash):
     - окружение (`source scripts/fork/agent-env.sh`);
     - гейты и базовые числа (раздел 3);
     - правила cargo-слотов и порта 1420;
     - `pkill -f target/debug/terax` вместо `pkill -x terax`;
     - не форматировать весь репозиторий;
     - новый код форка класть в новые модули;
     - ссылка на `fork-plan/PLAN.md`.
- Готово: новая сессия Claude Code в репозитории видит содержимое TERAX.md; `source scripts/fork/agent-env.sh && pnpm --version` печатает 11.9.0.
- Проверка: `bash -c 'source scripts/fork/agent-env.sh && node -v && pnpm -v'`.

#### E0-T03 · Своя идентичность сборки и выключенный апдейтер
- Модель: Sonnet · Ревью: нет (гейт плюс ручная проверка) · M · Зависит: E0-T01
- Файлы:
  - новые `src-tauri/tauri.dev.conf.json` и `src-tauri/tauri.fork.conf.json`;
  - `package.json` (scripts);
  - `src-tauri/src/lib.rs:195` (updater plugin);
  - `src/app/App.tsx:107,1602` (UpdaterDialog);
  - `src/settings/sections/AboutSection.tsx` (проверка обновлений);
  - `src-tauri/src/modules/control.rs:569-582`.
- Шаги:
  1. `tauri.dev.conf.json`: identifier `app.crynta.terax.dev`, productName "Terax Dev". `tauri.fork.conf.json`: identifier `dev.terax.fork`, productName "Terax Fork", `bundle.createUpdaterArtifacts: false`. Оба файла это merge-patch к `tauri.conf.json`.
  2. Скрипты: `"fork:dev": "tauri dev --config src-tauri/tauri.dev.conf.json"`, `"fork:build": "tauri build --ignore-version-mismatches --bundles app --config src-tauri/tauri.fork.conf.json"`.
  3. Апдейтер не должен никогда ставить релиз апстрима. В обоих override-конфигах задать `"plugins": {"updater": {"endpoints": []}}` (merge-patch заменяет массив целиком) и `createUpdaterArtifacts: false`. На фронте константа `FORK_UPDATER_DISABLED` скрывает `UpdaterDialog` (`App.tsx:107,1602`) и проверку обновлений в About (`AboutSection.tsx`, `useUpdater` на строке 27). Плагин в `lib.rs:195` и `updater:default` в `capabilities/desktop.json:15` не трогать: в `Cargo.toml` нет `[features]`, и feature-гейт потребовал бы правок capability и `useUpdater`.
  4. `control.json`: путь `~/Library/Caches/terax/control.json` общий с установленным Terax, и dev-экземпляр перетирает дескриптор CLI. Сделать подпапку зависимой от identifier: `cache/terax` для `app.crynta.terax`, иначе `cache/terax-<identifier>`. Логику вынести в чистую функцию с тестом (это control surface, тест на инвариант обязателен по TERAX.md). CLI внутри терминалов Terax получает адрес через `TERAX_CONTROL_ADDR`/`TOKEN` (`shell_init.rs:174-175`), а запасной путь CLI (`crates/terax-cli/src/main.rs:282-283`) продолжит указывать на основной Terax; это нормально, отметить в коммите.
  5. Keychain-сервис не трогать (D8).
- Готово:
  - `pnpm fork:dev` открывает окно "Terax Dev" с отдельной папкой `~/Library/Application Support/app.crynta.terax.dev`;
  - установленный Terax продолжает работать одновременно, его `control.json` не меняется;
  - в форк-сборке нет запросов к `github.com/crynta/.../latest.json`.
- Проверка: FE, RS.
- Руками: запустить `pnpm fork:dev` при открытом основном Terax, у обоих работает `terax open` из их терминалов.

#### E0-T04 · Выровнять версии @tauri-apps (опционально)
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T01
- Файлы: `package.json:72-82,116`, `pnpm-lock.yaml`.
- Шаги:
  1. Поднять 11 пакетов `@tauri-apps/*` до минорных версий, совпадающих с crate в `src-tauri/Cargo.lock`. Список и целевые версии в отчёте build-env.md, блокер B1.
  2. Учитывать `minimumReleaseAge: 4320` в `pnpm-workspace.yaml`: версии моложе 3 дней не ставятся, их пропустить.
  3. Убрать `--ignore-version-mismatches` из `fork:build`.
- Готово: `pnpm tauri build` без флага не падает на проверке версий.
- Проверка: FE, SIZE.

#### E0-T05 · Скрипт релизной сборки форка
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T03
- Файл: новый `scripts/fork/release.sh`.
- Шаги:
  1. `pnpm fork:build`.
  2. Ad-hoc подпись по рецепту из build-env.md, раздел 4.5: сначала `Contents/MacOS/terax-cli`, затем весь .app с `--entitlements src-tauri/entitlements.plist` (иначе не работает микрофон для голосового ввода).
  3. `codesign --verify --deep --strict`.
  4. Флаг `--install` копирует в `/Applications/Terax Fork.app`, предварительно закрыв запущенный "Terax Fork" по пути. `Terax.app` скрипт не трогает никогда.
- Готово: скрипт печатает путь к .app и его размер; повторный запуск идемпотентен.

#### E0-T06 · Каркас настроек форка
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T01
- Файлы:
  - `src/modules/settings/store.ts` (шаблон: тип 160-169, дефолты 256-265, 348-357, 505-525, 1004);
  - новый `src/settings/sections/ForkSection.tsx`;
  - регистрация раздела в трёх местах: union `SettingsTab` в `src/modules/settings/openSettingsWindow.ts:3-9`, `TABS` (~27-60) и `VALID_TABS` (71) в `src/settings/SettingsApp.tsx`.
- Шаги:
  1. Добавить ключи с дефолтами:
     - `terminalDefaultMode: "classic" | "blocks"` = `"classic"`;
     - `terminalClickToMoveCursor: boolean` = `true`;
     - `terminalOptionAsMeta: "off" | "left" | "both"` = `"off"`;
     - `terminalCopyOnSelect: boolean` = `false`;
     - `sshRemoteShellIntegration: boolean` = `true`;
     - `aiRemoteToolsReadOnly: boolean` = `true`;
     - `notifyLongCommandsSeconds: number` = `0` (0 = выключено).
  2. Валидация значений по образцу соседних ключей.
  3. Пустой раздел настроек "Fork" с заголовком. Строки-переключатели добавляют задачи, реализующие фичу.
- Готово: настройки сохраняются и переживают перезапуск; раздел виден.
- Проверка: FE.

### E1. Ввод в терминале (просьба 1)

Отчёт: `click-cursor.md`, плюс `gaps.md` (разделы про Option и раскладку).

#### E1-T01 · Режим новых вкладок: Classic или Blocks
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T06
- Файлы: `src/app/App.tsx:589-599` (`openNewTab`, `openNewBlockTab`), `src/settings/sections/ForkSection.tsx`, новый `src/modules/tabs/lib/newTabMode.ts` с тестом.
- Шаги:
  1. Чистая функция `resolveNewTabKind(pref, variant: "default" | "alternate")`: при `blocks` Cmd+T открывает Blocks, а Cmd+Shift+T открывает Classic; при `classic` всё как сейчас.
  2. Подключить её в `openNewTab` и `openNewBlockTab`.
  3. Строка в ForkSection с подсказкой: Blocks-вкладки не делятся на панели (`useTabs.ts:1295`), для панелей открыть Classic через альтернативный хоткей.
- Готово: при `blocks` Cmd+T открывает Blocks; тест покрывает 4 комбинации.
- Проверка: FE.

#### E1-T02 · Недостающие readline-хоткеи
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T01
- Файлы: `src/modules/terminal/lib/keymap.ts:47-57`, `src/modules/terminal/lib/keymap.test.ts`, `src-tauri/src/modules/pty/scripts/zshrc.zsh` (образец условного bindkey 87-90).
- Шаги: в `terminalReadlineSequence`, только на основном экране, добавить:

  | Клавиши | Отправляем | Действие |
  |---|---|---|
  | Cmd+Z | `\x1f` | undo в readline/zle/fish |
  | Option+Fn+Delete (`key === "Delete"`, altKey) | `\x1bd` | удалить слово вперёд |
  | Cmd+Fn+Delete | `\x0b` | удалить до конца строки |
  | Home / End без модификаторов | `\x01` / `\x05` | начало / конец строки |

  В `zshrc.zsh` добавить `bindkey '^U' backward-kill-line` с тем же условием "только если стоит стоковый биндинг". Сейчас Cmd+Backspace в zsh стирает всю строку.
- Готово: тесты на каждую комбинацию и на то, что на alt-screen комбинации не перехватываются.
- Проверка: FE. Rust-тесты скриптов (`shell_init.rs:430-480`) запускает главный агент при слиянии: `cd src-tauri && cargo test --locked --lib shell_init` (у задачи нет cargo-слота).

#### E1-T03 · Option-клавиши: баг keyCode 229 и Option как Meta
- Модель: Sonnet · Ревью: Sonnet · M · Зависит: E0-T06
- Файлы:
  - `src/modules/terminal/ghostty/input/GhosttyInputController.ts`: guard 229, `macOptionIsMeta` 144 и 278-281, encode 295-310, `unshiftedCodepoint` 589-605;
  - `.../GhosttyInputController.test.ts`;
  - `src/modules/terminal/ghostty/useGhosttyTerminalSession.ts:844` (`createGhosttyInput`);
  - `ForkSection.tsx`.
- Шаги:
  1. Апстрим-issue #1009: в WKWebView Option+стрелки и Option+Backspace могут приходить с `keyCode === 229`, и guard на строке 229 их глотает до readline-маппинга. Написать тест с таким событием; если он падает, обрабатывать readline-маппинг до IME-guard при `!event.isComposing && event.altKey`.
  2. Option как Meta: прокинуть `terminalOptionAsMeta`. В режиме `left` Meta только при левом Option: отслеживать `AltLeft`/`AltRight` по `event.code` на keydown/keyup, потому что правый Option нужен для кавычек-ёлочек и длинного тире на русской раскладке.
  3. Базовый символ брать из `event.code`, а не из переписанного Option `event.key`. Например, Option+B должен дать `ESC b`, а не `∫`.
  4. Строка в ForkSection.
- Готово, тесты на:
  - Option+B / Option+F / Option+D в режимах off, left, both;
  - правый Option в режиме left печатает символ;
  - событие с keyCode 229 и altKey при стрелке даёт `ESC b`;
  - IME-композиция (isComposing) не ломается.
- Проверка: FE.
- Руками: Option+стрелки, Option+Backspace, Option+B в zsh; набор «» правым Option на русской раскладке.

#### E1-T04 · Трекер фаз строки ввода и доступ к строке экрана
- Модель: Sonnet · Ревью: нет · M · Зависит: E0-T01
- Файлы:
  - новый `src/modules/terminal/ghostty/core/PromptInputTracker.ts` с тестом;
  - `ghostty/AdaptedGhosttyTerminalModel.ts` (`snapshotRow` 979-998);
  - `ghostty/GhosttyTerminalModel.ts:50-109` (интерфейс);
  - `ghostty/useGhosttyTerminalSession.ts`: тип сессии 61-86, `onEvent` 672-678, `respawnGhosttySession` 414-444.
- Шаги:
  1. Чистый класс с фазами `idle → prompt → input → running → idle`:
     - `prompt-start` даёт `prompt`;
     - `prompt-end` даёт `input`;
     - `end-of-input` даёт `running`;
     - `end-of-command` даёт `idle`;
     - `screen-cleared` и `overflow` сбрасывают фазу.

     Метод `phase()`, O(1) на событие.
  2. Публичный `viewportLine(row)` возвращает `{cells, wrapped}` через `snapshotRow`. В интерфейс добавить его как опциональный метод. Не использовать `readCellLine`: он скроллит viewport и теряет флаг `wrapped`.
  3. Хранить трекер в сессии, кормить в `onEvent`, сбрасывать в respawn и при замене модели.
- Готово:
  - тесты переходов, включая повторный A/B без C (перерисовка промпта fish/p10k);
  - тест на реальном WASM: длинная строка переносится, у второй строки `wrapped === true`, у широких символов spacer-ячейка `""`.
- Проверка: FE, `pnpm exec vitest run src/modules/terminal`.

#### E1-T05 · Чистая функция расчёта стрелок по клику
- Модель: Sonnet · Ревью: нет · M · Зависит: E0-T01 (параллельно с E1-T04)
- Файлы: новый `src/modules/terminal/ghostty/input/promptClickMove.ts` с тестом.
- Шаги:
  1. `promptClickDelta({cols, cursor:{row,col}, click:{row,col}, line:(row) => {cells, wrapped}}): number | null`.
  2. Группа строк: от строки курсора вверх, пока `line(r).wrapped`, и вниз, пока `line(r+1).wrapped`. Клик вне группы даёт `null`.
  3. Позиции линеаризуются как `(r - top) * cols + c`.
  4. Цель ограничивается справа последней непустой ячейкой + 1. Клик в spacer-ячейку смещается к началу широкого символа.
  5. Возвращается знаковое число символов (не ячеек) между курсором и целью.
  6. `encodeArrowRun(model, delta, cap)` повторяет `model.encodeKey({action: PRESS, key: LEFT|RIGHT, mods: NONE})`; кодировщик сам учитывает DECCKM и Kitty-режим. Ограничение 4096 нажатий.
- Готово, тесты:
  - ASCII;
  - перенос на 2-3 строки;
  - клик правее конца ввода;
  - клик в промпт левее начала (уезжает влево, это безопасно);
  - CJK и emoji;
  - курсор в состоянии pending-wrap в последней колонке;
  - на реальном WASM при включённом DECCKM стрелка кодируется как `ESC O D`, без него как `ESC [ D`.
- Проверка: FE.

#### E1-T06 · Клик ставит курсор в обычных вкладках
- Модель: Sonnet · Ревью: Sonnet · M · Зависит: E1-T04, E1-T05, E0-T06
- Файлы:
  - `ghostty/selection/TerminalSelectionController.ts`: опции 19-25, `handlePointerUp` 177-192;
  - `ghostty/gpu/WebGpuTerminalSurface.ts`: опции 62-74, создание selection 213-223, ссылки на mouseup 615-630;
  - `ghostty/webgl/WebGlTerminalSurface.ts`: 42-55, 157-167, 502-511;
  - `useGhosttyTerminalSession.ts`: `surfaceBaseOptions` 697-712, запись через `session.writer.enqueue`;
  - `ForkSection.tsx`;
  - тесты `TerminalSelectionController.test.ts` (`dragFixture` 247-290).
- Шаги:
  1. В контроллер выделения добавить `onClick(point, event)`: вызывается на pointerup, если выделение не стало "meaningful", кнопка 0 и `detail <= 1`. Использовать pointer-события, а не mouse: `preventDefault` на pointerdown может гасить mouse-события в Chromium.
  2. Пробросить `onCellClick` через обе поверхности: WebGL на этом Mac основная.
  3. Чистая функция `shouldMoveCursor(state)` разрешает перемещение, только если:
     - настройка включена;
     - не Blocks-prompt (`ghosttyBlocks(leaf)?.getMode() === "prompt"`);
     - основной экран;
     - mouse tracking выключен;
     - `scrollPosition().offset === 0`;
     - нет IME-композиции;
     - фаза трекера `input` для простого клика, либо зажат Option (тогда фаза любая, но только группа строк курсора).
  4. Приоритет ссылок: если клик внутри группы строк ввода и под курсором ссылка (`hyperlinkAtViewportCell`), простой клик двигает курсор, Cmd+клик открывает ссылку. Вне группы ссылки открываются как сейчас.
  5. Строка в ForkSection.
- Готово, тесты:
  - `onClick` не вызывается после drag, двойного и тройного клика и Shift+клика;
  - таблица случаев для `shouldMoveCursor`;
  - обработчик пишет в PTY ровно последовательность из `encodeArrowRun`.
- Проверка: FE.
- Руками:
  - zsh, bash (Homebrew 5.x; `/bin/bash` 3.2 без интеграции работает только через Option+клик), fish;
  - длинная перенесённая команда, кириллица, emoji;
  - в `vim`, `htop`, `less` не срабатывает;
  - URL в редактируемой строке открывается только по Cmd+клику.
- Известное ограничение v1: клик по серой автоподсказке zsh-autosuggestions её примет; многострочный ввод (Shift+Enter, PS2) не поддерживается.

#### E1-T07 · Колесо мыши в less, man, git log
- Модель: Sonnet · Ревью: нет · S · Зависит: E0-T01
- Файлы: `GhosttyInputController.ts`, `handleWheel` 456-487, и его тест.
- Шаги:
  1. На alt-screen без mouse tracking (DECSET 1007, по умолчанию считать включённым, проверять `model.mode(1007)`, если режим доступен) колесо отправляет стрелки вверх/вниз через `encodeKey`, по одной на "строку" прокрутки.
  2. Сейчас там выход на `history === 0`.
- Готово: тест на alt-screen с выключенным и включённым tracking.
- Проверка: FE.
- Руками: `less`, `man ls`, `git log`.

#### E1-T08 · (опционально) Метка OSC 133 B в конец промпта zsh/bash
- Модель: Sonnet · Ревью: Sonnet · S-M · Зависит: E1-T06
- Файлы: `pty/scripts/zshrc.zsh:62-65`, `pty/scripts/bashrc.bash:63-66`, `ghostty/shellBlockCompatibility.test.ts:29`, проверка `core/PromptPresentationGate.ts`.
- Зачем: в zsh и bash метка B сейчас стоит в начале PS1, поэтому точное начало ввода неизвестно. Так сделано по спецификации, как у fish и pwsh.
- Риск: окно подавления рендера между A и B станет длиннее (сторожевой таймер 1 с). Проверить с p10k и starship, если ими пользуешься.

#### E1-T10 · Запуск скриптов в терминале (.command из Finder, Run в Terax)
- Модель: Sonnet · Ревью: Opus (запуск файлов, пришедших от ОС) · M · Зависит: E0-T06
- Зачем: сейчас любой файл, открытый через Finder ("Открыть в программе"), попадает в редактор (`App.tsx:666-686`, `handleOpenFile` / `openLaunchFiles`). `.command` в ассоциациях нет вовсе, а `sh`, `bash`, `zsh` зарегистрированы как Editor (`tauri.conf.json`, `bundle.fileAssociations`).
- Файлы:
  - `src-tauri/tauri.conf.json` (`bundle.fileAssociations`);
  - новый `src-tauri/src/modules/scripts.rs` и `src-tauri/src/modules/mod.rs`, регистрация в `lib.rs` (`generate_handler!`);
  - `src/modules/settings/store.ts` и `src/settings/sections/ForkSection.tsx`;
  - новый `src/modules/terminal/lib/runScript.ts` с тестом;
  - `src/app/App.tsx` (`openLaunchFiles` около 680-686, шаблон запуска `whenSessionReady` + `writeToSession` в 600-636);
  - `src/modules/explorer/FileExplorer.tsx` (контекстное меню 795-925);
  - `src/modules/editor/EditorStack.tsx` (кнопка Run поверх редактора, по образцу `MarkdownViewToggle` на 105-113).
- Шаги:
  1. **Ассоциации.** Новая запись `fileAssociations` для `command` и `tool` с `role: "Shell"` и `contentTypes: ["com.apple.terminal.shell-script"]`. Поддержка полей проверена в `node_modules/@tauri-apps/cli/config.schema.json`. Существующую запись не менять. Сделать Terax обработчиком по умолчанию пользователь может сам через Finder "Свойства".
  2. **Rust**, команда `script_launch_info(path)`:
     - канонизирует путь и требует обычный файл;
     - возвращает `{executable, quarantined}`;
     - `executable` по mode-битам на unix;
     - `quarantined` есть ли xattr `com.apple.quarantine` (на macOS через `libc::getxattr`, на других ОС `false`).

     Команда `script_make_executable(path)`:
     - добавляет `x` туда, где есть `r` (`u`, `g`, `o`);
     - только если родительский каталог авторизован (`WorkspaceRegistry::is_authorized`; каталог launch-файла уже авторизуется в `lib.rs` около 378).

     Чистые хелперы и тесты на временных файлах (mode, xattr на macOS через `xattr -w` в тесте, неавторизованный каталог).
  3. **Настройка** `launchCommandFiles: "run" | "edit"` = `"run"`: валидация как у соседних ключей, строка в ForkSection.
  4. **Чистый модуль `runScript.ts`:**
     - `isRunnableScriptPath(path)`: расширения `command`, `tool`, `sh`, `bash`, `zsh`, `fish`;
     - `isLaunchRunnable(path)`: только `command` и `tool`;
     - `hasShebang(text)`;
     - `buildRunInput(path)`: `quoteShellPath(path)` из `terminal/lib/quoteShellPath.ts` плюс `\r`.

     Тесты: пробелы, кавычки, кириллица, `$`, переводы строк в имени.
  5. **Запуск** `runScriptInNewTab(path)`:
     - запросить `script_launch_info`;
     - при карантине модальное подтверждение "Файл скачан из интернета. Запустить?" (существующий AlertDialog);
     - если файл не исполняемый: toast с действиями "Сделать исполняемым и запустить" (`script_make_executable`) и "Открыть в редакторе"; молча права не менять;
     - затем новая классическая терминальная вкладка с cwd = каталог файла и заголовком = имя файла; после `whenSessionReady` ввести `buildRunInput(path)`. Оболочка остаётся открытой после завершения скрипта.
  6. **Finder.** В `openLaunchFiles`: если `isLaunchRunnable(path)` и настройка `run`, то `runScriptInNewTab`, иначе как сейчас.
  7. **Проводник.** Пункт "Run in Terminal" для `isRunnableScriptPath`. Одиночный клик по-прежнему открывает редактор.
  8. **Редактор.** Кнопка Run для `isRunnableScriptPath` или файла с шебангом:
     - неактивна с подсказкой "Save to run", пока есть несохранённые изменения;
     - только для локального окружения (в SSH-пространстве скрыта до отдельной задачи).
- Готово:
  - тесты чистых функций и Rust-команд;
  - eager-budget не растёт: модальное окно и логика запуска грузятся лениво или уже в eager-графе;
  - `.command` из Finder в Terax Fork запускается в новой вкладке.
- Проверка: FE, RS, SIZE.
- Руками:
  - "Открыть в программе > Terax Fork" для `.command`: вкладка в папке скрипта, вывод виден;
  - скачанный файл с карантином спрашивает подтверждение;
  - неисполняемый файл предлагает `chmod`;
  - "Run in Terminal" в проводнике;
  - кнопка Run в редакторе.

#### P-01 · Доводка по замечаниям ревью W1
- Модель: Sonnet · Ревью: нет · M · Зависит: E1-T01, E3-T06, E1-T10 (все влиты)
- Мелкие замечания ревьюеров W1, собранные в одну задачу. Каждый пункт с тестом, если он меняет логику.
- Шаги:
  1. `src-tauri/src/modules/remote/ssh_config.rs`:
     - (a) сохранять и восстанавливать текущий Host-блок вокруг `Include`, как OpenSSH (`readconf.c` возвращает `activep` после include). Тест: `Host a`, внутри `Include` файла с `Host b`, затем `User u`; `User` должен принадлежать `a`;
     - (b) при раскрытии glob в `Include` пропускать имена, начинающиеся с `.`, если сам компонент шаблона не начинается с `.` (как glob(3)). Тест с `.foo.conf.swp`;
     - (c) верхний `~/.ssh/config` читать, только если это обычный файл не больше 1 MiB; множество посещённых канонических путей, чтобы файл из нескольких Include разбирался один раз.
  2. SSH-вкладка из палитры получает заголовок `ssh <alias>` через `customTitle` (сейчас она называется "private"). Добавить параметр заголовка в `newPrivateTab` или выставить `customTitle` после создания, как в `newAgentGroupTab`.
  3. `src-tauri/src/modules/scripts.rs`:
     - (a) `is_quarantined`: если `getxattr` вернул ошибку, `false` только при `ENOATTR`; любая другая ошибка считается карантином (fail closed). Тест на ветку ошибки через чистую функцию от кода ошибки;
     - (b) `script_launch_info` требует, чтобы родительский каталог был авторизован (`WorkspaceRegistry::is_authorized`), как и `script_make_executable`;
     - (c) `script_launch_info` дополнительно возвращает `hasShebang`, читая только первые 2 байта файла.
  4. `src/app/App.tsx`, запуск скриптов:
     - (a) вкладка скрипта получает `customTitle` с именем файла (`title` для терминала с cwd игнорируется, `tabs/lib/tabLabel.ts`);
     - (b) несколько файлов из Finder обрабатываются строго по очереди (`for ... await`), чтобы подтверждения карантина не отменяли друг друга;
     - (c) вынести решение в чистую `launchAction(path, setting, envKind): "run" | "edit"` в `src/modules/terminal/lib/runScript.ts` с тестом (run только для `.command`/`.tool`, при настройке `run`, в локальном окружении и после гидрации настроек) и использовать её в `openLaunchFiles`.
  5. `src/modules/editor/RunScriptButton.tsx`: кнопка видна для `isRunnableScriptPath` или `hasShebang` из `script_launch_info` (любое расширение, не только без расширения); перепроверка, когда `dirty` переходит из true в false; без `fs_read_file` целиком.
  6. E1-T01: пункты палитры и меню "New Blocks terminal" всегда открывают Blocks, а "New terminal" всегда Classic. Настройке `terminalDefaultMode` следуют только хоткеи Cmd+T / Cmd+Shift+T. Тест на `resolveNewTabKind` дополнить вариантом `explicit`.
- Проверка: FE, RS.

#### P-02 · Доводка по замечаниям ревью E1-T03, E1-T06, E2-T03
- Модель: Sonnet · Ревью: нет · M · Зависит: E1-T03, E1-T06, E2-T03 (все влиты)
- Только фронтенд. Каждый пункт с тестом.
- Шаги:
  1. `src/modules/terminal/ghostty/input/GhosttyInputController.ts`:
     - (a) Option как Meta для «мёртвых» клавиш macOS (Option+E, U, I, N, `): WKWebView присылает `key === "Dead"`, и IME-guard возвращает раньше, чем срабатывает Meta. До guard: если `altKey`, нет композиции, режим Meta активен для этой стороны Option и `optionMetaBase(event)` даёт символ, то отправить `ESC <base>` и поглотить событие. Тесты: `key "Dead"`, `code "KeyU"` в режимах left и both; в режиме off поведение прежнее;
     - (b) в `handleBlur` сбрасывать `altLeftDown`; тест: keyup левого Option потерян, blur, затем правый Option+буква печатает символ;
     - (c) тесты: Option+Backspace с keyCode 229 даёт `\x17`; `key "Process"` со стрелкой вправо даёт `\x1bf`; Shift+Option+KeyB даёт `\x1bB`.
  2. Клик по курсору (`input/cellClickCursor.ts` и обвязка в `useGhosttyTerminalSession.ts`):
     - (a) тест колбэка `shouldOpenLink` на три случая: простой клик внутри группы строк ввода не открывает ссылку, Cmd+клик открывает, клик вне группы открывает;
     - (b) тесты: ветка `pendingWrap` (cols=10, курсор на x=9 над непустой ячейкой, фаза input, delta учитывает +1) и группа из двух перенесённых строк.
  3. `src/modules/html-preview/HtmlPreviewFrame.tsx` и `lib/useHtmlPreviewReload.ts`:
     - (a) в состоянии ошибки кнопка Reload заново вызывает `preview_serve`, а не только меняет nonce;
     - (b) после 30-секундной выгрузки скрытого фрейма показ монтирует его один раз: отложенная перезагрузка, накопленная за время скрытия, отбрасывается, потому что новый фрейм и так грузит свежий файл;
     - (c) два события одного сохранения (`fs:file-written` и `fs:changed` через 150 мс плюс задержка наблюдателя) схлопываются надёжно: например, повторная перезагрузка подавляется, если с прошлой прошло меньше 1 секунды и mtime файла не изменился, или debounce увеличивается до 400 мс;
     - (d) обратные слэши в путях на macOS и Linux не превращаются в `/` (это допустимый символ имени файла).
- Проверка: FE.

#### P-03 · Доводка по замечаниям ревью E3-T08, E3-T09
- Модель: Sonnet · Ревью: нет · S · Зависит: E3-T08, E3-T09 (влиты)
- Шаги:
  1. `src-tauri/src/modules/remote/shell_init.rs`, установка скриптов на сервере:
     - сейчас две вкладки могут одновременно пройти проверку `[ -d "$_tx_dir" ]`;
     - тогда второй `mv -f "$_tx_tmp" "$_tx_dir"` кладёт временную папку внутрь уже установленной, и она остаётся мусором.

     Сделать установку устойчивой к гонке переносимыми средствами (GNU и BSD, без `mv -T`): например, если после `mv` внутри целевой папки оказался каталог с именем временной, удалить его; или делать `mv` только если целевой нет, а иначе удалять временную. Тест через запуск bootstrap дважды подряд с заранее созданной целевой папкой: внутри неё нет `.tmp.*`.
  2. fish: `init.fish` грузится через `-C` уже после `config.fish` и определяет пустой `fish_greeting`, перекрывая приветствие пользователя. Определять пустую функцию, только если пользователь не задал свою (`functions -q fish_greeting`) или если приветствие ещё не показано; комментарий в скрипте привести в соответствие. Проверить, что это не ломает локальный fish (тот же скрипт используется локально через conf.d).
  3. `src-tauri/src/modules/workspace.rs`: тест, что `workspace_authorize` с `WorkspaceEnv::Ssh` регистрирует корень только в `RemoteRegistry` и ничего не добавляет в локальный `WorkspaceRegistry`. Через выделенную чистую функцию, если команде нужен Tauri State.
- Проверка: RS; `fish --no-config` недоступен на этой машине, поэтому синтаксис fish проверять разбором, если fish не установлен.

#### E1-T09 · (отложено) Точное начало ввода из Zig-ядра
- Модель: Opus · Ревью: Opus · L · Зависит: E1-T06
- Нужен Zig 0.16 и пересборка обоих WASM-артефактов (`packages/ghostty-core/adapted/UPSTREAM.md`). Payload `prompt_end` в `terax.zig:812-839` дополняется позицией курсора или pin.
- Делать, только если v1 мешает: нужен многострочный ввод или промпты часто перерисовываются.

### E2. HTML-превью (просьба 2)

Отчёт: `html-preview.md` (разделы 4-10).

#### E2-T01 · Rust-модуль deny-list секретных путей
- Модель: Sonnet · Ревью: Opus · S-M · Зависит: E0-T01
- Файлы:
  - новые `src-tauri/src/modules/security/mod.rs` и `security/secret_paths.rs`;
  - `src-tauri/src/modules/mod.rs`;
  - общая фикстура `src-tauri/tests/fixtures/secret_paths.json`;
  - TS-тест паритета рядом с `src/modules/ai/lib/security.test.ts` (или существующим тестом security).
- Шаги:
  1. Перенести шаблоны и защищённые каталоги из `src/modules/ai/lib/security.ts:31-98`: `.env*`, `.ssh/`, `id_*`, credentials, keychain и т.д.
  2. Две чистые функции: `is_secret_path(&Path)` для локальных путей (на macOS без учёта регистра) и `is_secret_posix_path(&str)` для удалённых.
  3. Фикстура со списком путей и ожидаемым ответом используется и Rust-, и TS-тестом, чтобы списки не разъехались.
  4. Модуль пока никем не используется: потребители E2-T02, E3-T15.
  5. Учесть: в TS защищённые каталоги ищутся как подстрока в любом месте пути (`isUnderProtected`, `security.ts:174-184`), поэтому сегменты `etc/`, `sys/`, `proc/`, `.git/` внутри проекта тоже запрещаются. Для AI это нормально; для превью значит 404 на таких путях. Поведение сохранить как есть и явно описать в doc-комментарии модуля.
- Готово: оба теста читают одну фикстуру и проходят.
- Проверка: FE, RS.

#### E2-T02 · Протокол terax-preview:// в Rust
- Модель: Opus · Ревью: Opus · L · Зависит: E2-T01 (вливается после E3-T01: если к моменту ветвления `resolve_path` уже заменён на `local_path`, использовать его)
- Файлы:
  - новые `src-tauri/src/modules/preview/mod.rs` (команды, состояние), `preview/handler.rs` (протокол), `preview/pure.rs` (чистые функции);
  - `src-tauri/src/modules/mod.rs`;
  - `src-tauri/src/lib.rs`: builder 187-213, `.manage` 234-252, `generate_handler!` 253-344;
  - `src-tauri/tauri.conf.json:30` (CSP);
  - `src-tauri/src/modules/fs/watch.rs` (`SKIP_DIRS` 19-84, авторизация 211-227);
  - `docs/architecture/two-process-model.md`, `docs/architecture/security-model.md`, `TERAX.md`.
- Шаги:
  1. `PreviewState { roots: Mutex<HashMap<token, PreviewRoot { root, entry, served_dirs }>> }`, не больше 32 записей, вытеснение по LRU. Токен: 16 случайных байт в hex (32 символа, укладывается в 63-символьную метку хоста). Генерация как в `control.rs:558-567`, но 16 байт.
  2. Команда `preview_serve(path, root_hint: Option<String>, workspace: Option<WorkspaceEnv>)`:
     - любое окружение, кроме Local, отклоняется (тест обязателен: иначе в SSH-пространстве удалённый путь отдавался бы с локального диска);
     - канонизировать файл;
     - корень = ближайший предок файла (не выше `root_hint`), содержащий `.git` или `package.json`, иначе каталог самого файла. Корень никогда не бывает `$HOME`, `/` или предком `$HOME`. Причина: `$HOME` и любой cwd терминала авторизованы префиксно (`workspace.rs:33-35,135-139`, `App.tsx:1075-1086`), и корень `~` дал бы любой открытой странице читать весь домашний каталог;
     - корень должен быть авторизован (`WorkspaceRegistry::is_authorized`, образец `fs/mutate.rs:39-70`);
     - файл или корень из deny-list отклоняются;
     - вернуть `{token, url}`.

     Команда `preview_release(token)`.

     Команда `preview_open_external(token)` открывает исходный файл в браузере; допустим только файл, привязанный к токену. Terax зарегистрирован как обработчик `.html` (`tauri.conf.json` fileAssociations, ~100-115), поэтому `open_path` может открыть сам Terax. Открывать приложением, которое macOS использует для `https:` (браузер по умолчанию), например `open -b <bundle id обработчика https>` через Rust; на других ОС `tauri_plugin_opener`.
  3. Обработчик `register_asynchronous_uri_scheme_protocol("terax-preview", ...)`:
     - URL `terax-preview://<token>/<path>` (macOS/Linux) и `http://terax-preview.localhost/<token>/<path>` (Windows);
     - разбор, нормализация и проверка в чистых функциях `split_preview_uri`, `sanitize_rel_path` (отказ на `..` в любом кодировании, NUL, абсолютные пути, обратные слэши), `resolve_within_root` (canonicalize + `starts_with(root)`, симлинк наружу запрещён);
     - deny-list на каждый запрос, и к лексическому, и к канонизированному пути; открывается именно канонизированный путь;
     - пустой путь или путь на `/` даёт `index.html`;
     - IO в `spawn_blocking`.
  4. Ответ:
     - собственная таблица MIME с фолбэком `application/octet-stream` (не использовать `tauri_utils::MimeType`: он по умолчанию отдаёт text/html);
     - `X-Content-Type-Options: nosniff`, `Cache-Control: no-store`, `Access-Control-Allow-Origin: *`;
     - CSP страницы с явным origin, а не только `'self'`: не проверено, что WebKit сопоставляет `'self'` в sandbox-документе с opaque origin на custom-схеме. Пример: `default-src terax-preview://<token> http://terax-preview.localhost 'self' 'unsafe-inline' 'unsafe-eval' data: blob: https:; connect-src terax-preview://<token> 'self' https:`. CDN-библиотеки работают; `http://localhost` закрыт, но `https://localhost` технически остаётся доступен (принятый компромисс);
     - одиночный `Range` (206/416) для видео;
     - 404 для неизвестного или освобождённого токена.
  5. `fs/watch.rs`: публичный хелпер `watch_preview_dirs(paths)` / `unwatch_preview_dirs` без фильтра `SKIP_DIRS` (превью из `dist/` тоже обновляется), но с проверкой авторизации. Каталоги, из которых реально отдавались файлы, запоминаются в `served_dirs` и освобождаются в `preview_release`.
  6. В `tauri.conf.json:30` добавить `terax-preview:` в `frame-src`. Не встраивать Tauri-скрипты в эти фреймы и никогда не навигировать главный фрейм на эту схему: Tauri считает custom-схемы локальными (детали в отчёте, раздел 4).
  7. Обновить три документа.
- Готово, тесты на чистые функции и обработчик со временным каталогом:
  - кейсы обхода (`..`, `%2e%2e`, `..%2f`, абсолютный путь, NUL, симлинк наружу);
  - deny-list (`.env`, `id_rsa`, `.ssh/x`, регистр);
  - обе формы URL;
  - неизвестный токен;
  - MIME-таблица;
  - Range и 416;
  - отказ `preview_serve` для неавторизованного корня, для файла вне корня и для не-Local окружения;
  - выбор корня: проект с `.git`, файл без проекта, файл прямо в `~` (корень не `~`);
  - watch-хелпер требует авторизацию.
- Проверка: RS, FE.

#### E2-T03 · Фрейм превью и живая перезагрузка (фронт, без встраивания во вкладки)
- Модель: Sonnet · Ревью: Opus (sandbox-атрибуты) · M · Зависит: E2-T02
- Файлы:
  - новые `src/modules/html-preview/` (`index.ts`, `HtmlPreviewFrame.tsx`, `lib/htmlPreviewUrl.ts`, `lib/useHtmlPreviewReload.ts`, тесты);
  - перенос `pathAtOrUnder` из `src/app/hooks/tabCloseGuards.ts:7-14` в `src/lib/paths.ts` (старое место реэкспортирует);
  - `src/app/eager-budget.test.ts` (шаблон 27-34).
- Шаги:
  1. `HtmlPreviewFrame({path, rootHint, visible})`:
     - при монтировании `preview_serve`, при размонтировании `preview_release`;
     - `<iframe sandbox="allow-scripts allow-forms allow-modals allow-popups allow-popups-to-escape-sandbox" referrerPolicy="no-referrer" key={nonce}>`, без `allow-same-origin` и без `allow-top-navigation`, без `srcDoc`;
     - через 30 с в скрытом состоянии фрейм выгружается, как в `PreviewPane.tsx:35-58`;
     - панель: Reload и "Open in browser" (`preview_open_external`).
  2. `htmlPreviewUrl(token, relPath, platform)` кодирует каждый сегмент отдельно, сохраняя `/`.
  3. `useHtmlPreviewReload`:
     - за наблюдение отвечает Rust (`served_dirs` из E2-T02); фронт не вызывает `watchAdd`/`watchRemove` для этих каталогов. Причина: `fs_watch_remove` не фильтрует `SKIP_DIRS` (`watch.rs:248-271`), а add фильтрует (211-227), и remove из фронта уменьшил бы счётчик, принадлежащий превью;
     - слушает события `fs:file-written` и `fs:changed`;
     - сравнение с канонизированным корнем через `pathAtOrUnder`;
     - debounce ~200 мс; два события от одного сохранения схлопываются;
     - пока фрейм скрыт, перезагрузка откладывается до показа.
  4. Модуль доступен только через `lazy()`.
- Готово:
  - тест sandbox по образцу `src/modules/preview/PreviewPane.test.ts`: нет `allow-same-origin`, нет `allow-top-navigation`, нет `srcDoc`, есть `no-referrer`;
  - тесты URL-билдера (пробелы, `#`, `?`, кириллица, вложенность);
  - тест решения о перезагрузке и debounce;
  - в eager-budget добавлена проверка, что файлы `html-preview` не попадают в стартовый граф.
- Проверка: FE, SIZE.

#### E2-T04 · Режимы Code / Split / Preview во вкладке редактора
- Модель: Sonnet · Ревью: нет · M · Зависит: E2-T03
- Файлы:
  - `src/lib/utils.ts:8-10` (+ `isHtmlPath`, `isSvgPath`) и `src/lib/utils.test.ts`;
  - `src/modules/tabs/lib/useTabs.ts`: `EditorTab` 53-66 получает поле `docView?: "code" | "split" | "preview"`, setter;
  - `src/modules/spaces/lib/serialize.ts` (опциональное поле) и `serialize.test.ts`;
  - `src/modules/markdown/MarkdownViewToggle.tsx` обобщить в `DocumentViewToggle` с подписями через props; Markdown-использование сохранить;
  - `src/modules/editor/EditorStack.tsx:105-120`;
  - `src/app/App.tsx` (прокинуть корень проводника как `rootHint`).
- Шаги:
  1. Для HTML-файлов переключатель Code / Split / Preview поверх редактора. Показывается, только если `envCaps(env).htmlPreview` (после E3-T02; до неё считать true для Local).
  2. Для HTML-вкладки всегда рендерится одна и та же группа панелей (`react-resizable-panels` v4 через `src/components/ui/resizable.tsx`): слева `EditorPane`, справа lazy `HtmlPreviewFrame`. В Code правая панель свёрнута и пуста, в Preview редактор скрыт CSS, но смонтирован. Нельзя оборачивать `EditorPane` в панель только в Split: смена дерева перемонтирует редактор и теряет undo.
  3. Превью показывает сохранённый файл. В режиме Split с несохранёнными правками в панели метка "сохраните, чтобы обновить".
  4. По умолчанию `.html` открывается в Code (D3).
  5. Режим вкладки сохраняется в space.
- Готово:
  - тесты переходов `docView` и сериализации туда и обратно;
  - тест со счётчиком монтирований: code → split → preview → code не перемонтирует `EditorPane`;
  - старые сохранённые space без поля загружаются.
- Проверка: FE, SIZE.
- Руками:
  - относительный CSS, module-скрипт и `fetch('./x.json')` работают (проверка CSP из E2-T02);
  - страница с `./style.css`, `js/app.mjs` (module), `../shared.css`, `/root.css`, картинкой и mp4 с перемоткой;
  - сохранение CSS в Terax обновляет превью;
  - правка во внешнем редакторе обновляет превью;
  - страница внутри `dist/` тоже обновляется.

#### E2-T05 · Живое обновление Markdown и баг дублей вкладок
- Модель: Sonnet · Ревью: нет · S-M · Зависит: E2-T03
- Файлы: `src/modules/markdown/MarkdownPreviewPane.tsx:30-56`, `src/modules/tabs/lib/useTabs.ts` (`planMarkdownTabOpen` 174-203, `planFileTabOpen` 205-282), `src/modules/tabs/lib/planFileTabOpen.test.ts` (блок `planMarkdownTabOpen` со строки 107).
- Шаги:
  1. Rendered-Markdown перечитывает файл через `useHtmlPreviewReload` (или его общую часть). Сейчас файл читается один раз.
  2. Новая функция `planRenderedTabOpen` ищет вкладку с тем же путём среди видов `editor` и `markdown`. Сейчас .md, переключённый в Raw, при повторном клике в проводнике открывается второй вкладкой.
- Готово: тест на дубль; руками правка .md видна в Rendered без переключения.
- Проверка: FE.

#### E2-T06 · Превью SVG и пункт "Open Preview" в проводнике
- Модель: Sonnet (новый проп проводника, проводка через App и открытие вкладки сразу в нужном режиме) · Ревью: нет · S · Зависит: E2-T04
- Файлы: `EditorStack.tsx` (тот же переключатель для `isSvgPath`), `src/modules/explorer/FileExplorer.tsx:817-824` (контекстное меню), проводка нового пропа в `App.tsx:1447-1465`.
- Шаги:
  1. SVG получает те же режимы Code / Split / Preview через `HtmlPreviewFrame`.
  2. В контекстном меню для `.html`, `.htm`, `.svg`, `.md` пункт "Open Preview": открывает вкладку сразу в режиме Preview (для md в Rendered).
- Готово: SVG рисуется; пункт меню есть только у этих типов.
- Проверка: FE.

#### E2-T07 · (опционально) Сузить asset-протокол
- Модель: Sonnet · Ревью: Opus · M · Зависит: E2-T04
- Сейчас `assetProtocol.scope = ["**"]` (`tauri.conf.json:31-34`): при XSS в главном окне доступен весь диск.
- Предлагаемый вариант: пустой scope и разрешение конкретного файла через `asset_protocol_scope().allow_file()` в Rust-команде в момент открытия медиа (`EditorPane.tsx:568-628`), с проверкой авторизации.
- Готово: картинки, видео и PDF открываются, а `asset://` к произвольному файлу отклоняется.

### E3. SSH Remote (просьба 3)

Отчёты: `ssh-seams.md` (весь), `upstream-and-ssh-ecosystem.md` (разделы 2, 4, 6). Весь новый код в `src-tauri/src/modules/remote/*` и `src/modules/remote/*`: апстрим будет делать свой SSH, так проще мержиться. Всё, что требует ControlMaster, под `#[cfg(unix)]`; на Windows компилируется заглушка с понятной ошибкой.

**Фаза 0: подготовка без изменения поведения**

#### E3-T01 · Rust: вариант Ssh и fail-closed
- Модель: Opus · Ревью: Opus · L · Зависит: E0-T01
- Файлы:
  - `src-tauri/src/modules/workspace.rs`: enum 311-329, `resolve_path` 338-349;
  - все места из отчёта ssh-seams.md, раздел 1.2: fs ×23, `git/utils.rs:34`, `shell/mod.rs:94,284-327`, `shell/background.rs:108`, `shell/session.rs:129-141`, `pty/mod.rs:63-70`, `pty/shell_init.rs:53-71`, `lsp/mod.rs:73-75`, `fs/search.rs:237`, `fs/grep.rs:331`, `git/utils.rs:41`.
- Шаги:
  1. Добавить `WorkspaceEnv::Ssh { host: String }` (serde `kind: "ssh"`).
  2. Заменить `resolve_path` на `local_path(path, ws) -> Result<PathBuf, String>`: для `Ssh` ошибка "not supported in SSH workspace yet".
  3. Все `is_wsl()` и `if let Local` в местах исполнения заменить на исчерпывающие `match`.
  4. `build_command` и `build_oneshot_command` для `Ssh` возвращают ошибку.
  5. LSP отклоняет всё, кроме `Local`.
  6. `pty_open` не добавляет control env для любого не-`Local`.

  Поведение для Local и WSL не меняется.
- Готово:
  - тест-инвариант: каждая fs/shell/git/lsp/pty команда с `workspace: Ssh` возвращает ошибку и не трогает локальный диск (временный каталог с файлом по тому же пути не читается и не пишется);
  - существующие 373 теста зелёные.
- Проверка: RS (на macOS; дополнительно `cargo check --target x86_64-pc-windows-msvc` не требуется, но cfg-ветки не должны ломать сборку под Linux: `cargo check` с `--target x86_64-unknown-linux-gnu`, если тулчейн доступен, иначе пропустить и отметить).

#### E3-T02 · TS: модель окружения ssh
- Модель: Sonnet · Ревью: нет · M · Зависит: E3-T01
- Файлы:
  - `src/modules/workspace/env.ts:5-7,52-60`;
  - `src/app/hooks/useWorkspaceSwitcher.ts:11-15,76-82`;
  - `src/modules/spaces/lib/activeSpace.ts:24-31`;
  - `src/modules/theme/themeFiles.ts:29-49` и `useThemeFileEditing.ts:39` (всегда LOCAL);
  - кэши `explorer/lib/useFileTree.ts:57-76,243-277`, `ai/hooks/useWorkspaceFiles.ts:30`, проектная память в `ai/lib/transport.ts:19`, `authorizedCwds` в `App.tsx:1075`.
- Шаги:
  1. `{kind: "ssh"; host: string}` в union.
  2. Исчерпывающие `switch` с `assertNever` вместо тернарников; ключ области видимости `ssh:<host>`.
  3. `envCaps(env)` возвращает `{watch, gitDecorations, reveal, mediaPreview, htmlPreview, lsp, git, formatOnSave, aiShell, aiWrite}` (для ssh `htmlPreview: false` до E3-T20).
  4. Темы всегда читаются локально.
  5. Кэши ключуются окружением, а не только путём; эффект корня в `useFileTree` зависит и от ключа окружения.
- Готово: тесты `env.ts` (парсинг и сериализация ключей, `envCaps`), тест, что тема читается с LOCAL при активном ssh-окружении.
- Проверка: FE.

#### E3-T03 · Окружение из вкладки, а не из глобального стора
- Модель: Sonnet · Ревью: Opus · M-L · Зависит: E3-T04
- Файлы:
  - новый `src/modules/spaces/lib/envForSpace.ts`;
  - `terminal/lib/pty-bridge.ts:36-113`;
  - `editor/lib/useDocument.ts:61-138` (включая автосохранение 206-222);
  - `editor/useEditorFileSync.ts:23-97`;
  - `markdown/MarkdownPreviewPane.tsx`;
  - `App.tsx:1075-1086`;
  - `spaces/lib/useSpacesBoot.ts:105-107`;
  - Rust `fs/watch.rs:104-107,163-178` и `fs/file.rs:105-110,127-158` (метка окружения в событиях);
  - AI: `ai/lib/native.ts:144-163`, `ai/lib/composer.tsx:190`, `ai/tools/*.ts` (например `tools/shell.ts:27`).
- Шаги:
  1. Сейчас окружение берётся из глобального стора в момент вызова: автосохранение после переключения space может записать файл не туда. Нужно передавать env, вычисленный из `tab.spaceId`, во все пути ввода-вывода.
  2. AI-агент привязывает env в момент старта запуска (run) и использует его до конца, а не читает `currentWorkspaceEnv()` при каждом вызове инструмента.
  3. События `fs:changed` и `fs:file-written` получают поле `env` (ключ области видимости). Обновить всех слушателей из `rg "listenFsChanged|fs:file-written" src`: `explorer/lib/watch.ts`, `useFileTree`, `useEditorFileSync`, темы, `useHtmlPreviewReload`.
  4. Авторизация cwd при загрузке делается под окружением каждого space, а не активного.
- Готово:
  - тест-инвариант: после смены активного space отложенное сохранение и перезагрузка документа используют окружение своей вкладки;
  - тест: смена space посреди запуска AI не меняет окружение его инструментов;
  - тест фильтрации событий по env.
- Проверка: FE, RS.

#### E3-T04 · Единый провайдер файловой системы fsFor(env)
- Модель: Sonnet · Ревью: нет · M · Зависит: E3-T02
- Файлы:
  - новый `src/modules/workspace/fs.ts`;
  - около 31 места вызова из ssh-seams.md, раздел 3: explorer, editor, statusbar, terminal `pathComplete`, palette, ai, `native.ts:139-384`.
- Шаги:
  1. Типизированный интерфейс над существующими командами: `readDir`, `listSubdirs`, `readFile`, `writeFile`, `stat`, `canonicalize`, `createFile`, `createDir`, `rename`, `move`, `delete`, `deleteBatch`, `copy`, `search`, `listFiles`, `grep`, `grepInteractive`, `glob`, `watchAdd`, `watchRemove`.
  2. Окружение передаётся явно параметром. Пока вызывающие передают `currentWorkspaceEnv()`; E3-T03 заменит это на окружение вкладки.
  3. Механическая замена `invoke("fs_*", ...)` без изменения аргументов.
- Готово: `rg 'invoke\("fs_' src` находит вызовы только в `workspace/fs.ts` (и в тестах); FE зелёный.
- Проверка: FE.

#### E3-T05 · Хост в OSC 7
- Модель: Sonnet · Ревью: нет · S · Зависит: E3-T02
- Файлы: `src/modules/terminal/lib/osc-handlers.ts:5-21` и тест (`osc-handlers.test.ts:10`), `terminal/ghostty/core/GhosttySemanticEventRouter.ts:33-38`, hostname через `@tauri-apps/plugin-os`, `src-tauri/capabilities/default.json:25`.
- Шаги:
  1. `parseOsc7` возвращает `{host, path}`.
  2. `hostname()` из plugin-os не входит в `os:default`: добавить `"os:allow-hostname"` в `capabilities/default.json`. Если имя хоста получить не удалось, локальная вкладка принимает любой хост (как сейчас), иначе локальные вкладки потеряют отслеживание cwd: скрипты шлют `$HOST` / `uname -n` (`zshrc.zsh:46`, `bashrc.bash:54`, `init.fish:28`).
  3. Локальная вкладка принимает `""`, `localhost` и собственное имя хоста (полное и короткое, без учёта регистра и суффикса `.local`); чужой хост игнорируется. Сейчас после `ssh` в локальной вкладке проводник может уйти на несуществующий локальный путь (апстрим #793).
  4. SSH-вкладка принимает хост, закреплённый при первом OSC 7.
- Готово: тесты на все варианты хоста, включая недоступный hostname; старый тест обновлён.
- Проверка: FE.

**Фаза 1: быстрая польза**

#### E3-T06 · Выбор SSH-хоста и вкладка с ssh
- Модель: Sonnet · Ревью: Opus · M · Зависит: E0-T01
- Файлы:
  - новые `src-tauri/src/modules/remote/mod.rs`, `remote/target.rs`, `remote/ssh_config.rs`;
  - `lib.rs` (`generate_handler!`);
  - `src/modules/command-palette/commands.ts`;
  - `src/modules/tabs/NewTabMenu.tsx:29-39`;
  - новый `src/modules/remote/` (`hosts.ts`);
  - `App.tsx:600-636` (шаблон `whenSessionReady` + `writeToSession`).
- Шаги:
  1. `target.rs`: `validate_alias(&str)`. Разрешены только `[A-Za-z0-9._@:%\[\]_-]`, первый символ не `-`, длина до 255; это защита от `-oProxyCommand=...`. Табличные тесты плюс proptest (`proptest` уже есть в dev-зависимостях, `Cargo.toml:69`).
  2. `ssh_config.rs`: чистый перечислитель `Host`:
     - идёт по `~/.ssh/config` и `Include` (тильда и glob относительно `~/.ssh`, глубина до 16, защита от циклов);
     - понимает `Host=x`, имена в кавычках, ключевые слова в любом регистре; `Match` завершает текущий блок;
     - пропускает шаблоны с `*`, `?`, `!`;
     - для отображения отдаёт `HostName`, `User`, `Port`;
     - ключи и другие файлы не читает.

     У тебя есть `Include ~/.orbstack/ssh/config`, поэтому хосты OrbStack тоже должны появиться.
  3. Команда `ssh_list_hosts`.
  4. Палитра: пункты `SSH: <alias>`. Меню новой вкладки: подменю SSH.
  5. Действие открывает новую приватную терминальную вкладку и после готовности сессии вводит `ssh -- '<alias>'` и Enter. Алиас повторно проверяется на фронте.
- Готово:
  - тесты парсера на фикстурах: Include, вложенный Include, цикл, шаблоны, комментарии, регистр ключевых слов, `Host a b`;
  - тесты `validate_alias` (отказ на `-o...`, пробелы, `;`, `$()`, переводы строк).
- Проверка: FE, RS.
- Руками: в палитре видны `oracle`, `imac-remote`, `imac-local`, `server-local` и хосты OrbStack; подключение открывается во вкладке.

**Фаза 2: удалённый терминал**

#### E3-T07 · Ядро remote: ControlMaster, кавычки, ssh -G
- Модель: Opus · Ревью: Opus · L · Зависит: E3-T01, E3-T06
- Файлы:
  - новые `remote/quote.rs`, `remote/control.rs`, `remote/exec.rs`, `remote/state.rs`;
  - `lib.rs` (`.manage`, команды, очистка в `RunEvent::Exit`);
  - `Cargo.toml` (tokio features `process`, `io-util`, `time`, если нужны).
- Шаги:
  1. `posix_quote(&str)`: строгие одинарные кавычки. Тест на круговую проверку через `sh -c 'printf %s ...'` на unix.
  2. Каталог сокетов с правами 0700, путь короткий (лимит 104 байта на macOS), `ControlPath=<dir>/%C`. Каталог зависит от identifier: `~/.terax/cm` для "Terax Fork", `~/.terax/cm-dev` для "Terax Dev". Иначе выход одного экземпляра (`-O exit`) убьёт мастер другого.
  3. Чистый построитель аргументов, два режима:
     - PTY (интерактивный): `-o ControlMaster=auto -o ControlPath=... -o ControlPersist=600 -o ServerAliveInterval=30 -o RemoteCommand=none`;
     - всё остальное (exec, `-O check`, sftp): `-o ControlMaster=no -o ControlPath=... -o BatchMode=yes -o ConnectTimeout=10 -o RemoteCommand=none -o RequestTTY=no -o ClearAllForwardings=yes -o PermitLocalCommand=no -T`. Неинтерактивный вызов никогда не становится мастером: с `ControlMaster=auto` exec или sftp мог бы превратиться в фоновый ControlPersist-мастер (известная причина зависания stdio и сирот), а заодно подхватить твой `Host *` с `ControlMaster auto` / `ControlPersist 30m`. Host-уровневые `RemoteCommand`, `RequestTTY` и `LocalForward` тоже ломали бы `ssh host cmd` и `-s sftp`;
     - всегда `--` перед хостом;
     - никогда `StrictHostKeyChecking=no`;
     - без проброса агента.

     Наш `-o ControlPath` перекрывает твой `Host *` с `~/.ssh/master-...`; Terax держит свой мастер. Unit-тест на оба набора аргументов.
  4. Разбор вывода `ssh -G <alias>` (чистая функция).
  5. `ssh` ищется через `which`.
  6. `exec(host, script, timeout, max_output)` через мультиплексор. Удалённый login shell пользователя заново разбирает командную строку, а `posix_quote` безопасен только для sh-семейства: в fish `\\` и `\'` работают как экранирование даже внутри одинарных кавычек, tcsh тоже отличается. Поэтому exec всегда отправляет фиксированную обёртку только из безопасных символов и base64: `sh -c 'eval "$(printf %s <B64> | base64 -d 2>/dev/null || printf %s <B64> | base64 -D)"'`. `posix_quote` применяется только внутри декодированного POSIX-скрипта. Эту же обёртку используют bootstrap E3-T08, запасной `mv -f` E3-T12 и поиск E3-T13. Тест: если на машине есть fish, обёртка выполняется через `fish -c` и `bash -c` с аргументами `a\`, `'`, `\\`, `$(x)` и переводом строки, результат совпадает.
  7. Команды:
     - `ssh_check(host)` (`-O check`);
     - `ssh_disconnect(host)` (`-O exit`);
     - `ssh_home(host)` (`printf %s "$HOME"`);
     - `ssh_resolve(host)` (`-G`).
  8. На `RunEvent::Exit` закрыть все мастера, которые запустил Terax.
  9. Не-unix: заглушка.
- Готово:
  - юнит-тесты построителей и кавычек;
  - интеграционный тест, который запускается только при наличии переменной `TERAX_SSH_TEST_HOST` (OrbStack-машина) и проверяет `check`, `exec`, `home`, `disconnect`.
- Проверка: RS; с хостом `TERAX_SSH_TEST_HOST=<orb-host> cargo test --locked remote -- --ignored`.

#### E3-T08 · Удалённый PTY и shell-integration на сервере
- Модель: Opus · Ревью: Opus · L · Зависит: E3-T07
- Файлы:
  - новый `remote/shell_init.rs`;
  - `pty/shell_init.rs:53-71` (ветка `Ssh`);
  - `pty/mod.rs:63-70`;
  - образец аргументов оболочек и загрузки скриптов для WSL: `shell_init.rs:605-795`;
  - `ForkSection.tsx` (переключатель `sshRemoteShellIntegration`).
- Шаги:
  1. Для `Ssh` команда PTY: `ssh -tt <аргументы PTY из E3-T07> -- <host> <обёртка base64 из E3-T07 с bootstrap>`. Первая вкладка становится мастером; пароль, 2FA и подтверждение ключа хоста вводятся прямо в терминале.
  2. Скрипты интеграции маленькие (zshrc 3.6 KB, bashrc 3.1 KB, init.fish 3.5 KB; 1.4-2.2 KB в gzip), поэтому они встраиваются прямо в bootstrap. Bootstrap:
     - `cd -- '<cwd>' 2>/dev/null || cd`;
     - записывает скрипты в `~/.cache/terax/shell-integration/<sha>/`, только если их там ещё нет;
     - выбирает оболочку во время выполнения: `case "${SHELL##*/}"`;
     - запускает её с аргументами как у WSL-сборщика (`shell_init.rs:685-725`):
       - zsh: `ZDOTDIR=<dir> TERAX_USER_ZDOTDIR=<исходный> exec zsh -l`;
       - bash: `exec bash --rcfile <file> -i`, где rcfile подгружает `/etc/profile` и `~/.bash_profile` как локальный (`shell_init.rs:346-357`). `--rcfile` вместе с `-l` bash игнорирует;
       - fish: как у WSL;
       - прочие оболочки: `exec "$SHELL" -l` без интеграции;
     - переменные `TERAX_TERMINAL=1 COLORTERM=truecolor TERAX_BLOCKS=<0|1>`.

     Так интеграция есть уже в первой вкладке, без предварительного определения оболочки и без готового мастера.
  3. При выключенной настройке `sshRemoteShellIntegration` просто `cd` и `exec "$SHELL" -l`.
  4. `pty_open` для `Ssh`: не канонизировать cwd локально (проверить, что это абсолютный POSIX-путь без NUL), не добавлять control env.
  5. Закрытие SSH-вкладки всегда спрашивает подтверждение: проверка запущенного процесса для ssh-клиента не работает (`pty/mod.rs:236-288`).
- Готово:
  - снапшот-тесты argv и bootstrap для zsh, bash и fish, с интеграцией и без;
  - тест, что `--rcfile` никогда не стоит вместе с `-l`;
  - кавычки в cwd с пробелами и `'`.
- Проверка: RS.
- Руками на OrbStack-машине и на iMac:
  - cwd отслеживается;
  - Blocks работает;
  - клик по строке ввода (E1-T06) работает.

#### E3-T08b · Минимальный вход в SSH-пространство
- Модель: Sonnet · Ревью: нет · S · Зависит: E3-T02, E3-T03, E3-T08
- Зачем: без этой задачи до W6 нельзя переключиться на ssh-окружение (`pty-bridge.ts:74-78` берёт глобальное окружение), и `SSH: <alias>` из E3-T06 по-прежнему вводит `ssh` в локальный PTY.
- Шаги: в палитре пункт "SSH: <alias> (remote shell)": создаёт space с `env: {kind: "ssh", host}` и одной терминальной вкладкой, переключается на него. Без диалогов и индикатора (это E3-T14).
- Готово: тест создания space с ssh-окружением; руками вкладка в OrbStack-машине открывается как удалённый PTY с интеграцией.
- Проверка: FE.

#### E3-T09 · Реестр удалённых корней
- Модель: Sonnet · Ревью: Opus · M · Зависит: E3-T07
- Файлы: новый `remote/registry.rs`; `workspace.rs:142-152` (`workspace_authorize`, ветка `Ssh`).
- Шаги:
  1. `RemoteRegistry`: хост → множество нормализованных POSIX-корней.
  2. Чистая нормализация: схлопнуть `.`, `..`, `//`; отказ на относительные пути и NUL.
  3. Корни добавляются только явными действиями пользователя (подключение, открытие папки, cwd из OSC 7 SSH-вкладки), никогда AI.
  4. Проверка `is_authorized(host, path)` для всех мутаций (используется в E3-T12). Только лексической нормализации мало: симлинк `root/link -> /etc` позволил бы писать вне корня. Поэтому реестр принимает уже разрешённый путь, а E3-T12 перед проверкой делает SFTP `realpath` родителя и самого объекта, если он существует.
- Готово: тесты нормализации и авторизации, включая попытку `../` выйти за корень; API реестра документирует, что на вход нужен realpath.
- Проверка: RS.

**Фаза 3: файлы на сервере**

#### E3-T10 · SFTP-клиент поверх ssh
- Модель: Opus · Ревью: Opus · L · Зависит: E3-T07
- Файлы: новые `remote/fs/mod.rs` (трейт `RemoteFs`), `remote/fs/sftp.rs`; `Cargo.toml` (`russh-sftp` с точной версией текущего релиза в виде `=X.Y.Z`, на момент плана 3.0.1).
- Шаги:
  1. Трейт `RemoteFs`: `read_dir`, `stat`, `read(limit)`, `write_atomic`, `mkdir`, `remove`, `rename`, `realpath`, `setstat`, плюс флаг поддержки `posix-rename@openssh.com`.
  2. Реализация на `russh-sftp` поверх stdio дочернего процесса `ssh <неинтерактивные аргументы из построителя E3-T07> -s -- <host> sftp`. Свои аргументы не собирать; unit-тест, что используется общий построитель.
  3. Одна сессия на хост в `RemoteState`:
     - ленивое подключение;
     - переподключение при EOF;
     - таймауты на запросы;
     - дочерний процесс убивается при выходе.
  4. Замерить прирост размера релизного бинаря (ожидание около +170 KB) и записать в коммит.
- Готово: интеграционные тесты без сети: вместо ssh напрямую через stdio запускается локальный `sftp-server` (macOS: `/usr/libexec/sftp-server`, есть на этом Mac; Linux: `/usr/lib/openssh/sftp-server` или `/usr/libexec/openssh/sftp-server`; если не найден, тест пропускается). Проверяются `read_dir`, `stat`, `read`, `write_atomic`, `rename`, `realpath`, ошибки и большие файлы.
- Проверка: RS.

#### E3-T11 · Чтение файлов сервера через существующие fs-команды
- Модель: Sonnet · Ревью: Opus · M · Зависит: E3-T10, E3-T09
- Файлы: `fs/tree.rs:134-248` (`fs_read_dir`, `list_subdirs`), `fs/file.rs:57-103,160-192` (`fs_read_file`, `fs_canonicalize`, `fs_stat`), новый `remote/fs/dispatch.rs`.
- Шаги:
  1. В начале каждой команды: `if let WorkspaceEnv::Ssh { host } = &ws { return remote::fs::... }`.
  2. Синхронные команды пометить `#[tauri::command(async)]`, локальные тела оставить синхронными (поведение Local не меняется). Иначе сетевые задержки подвешивают UI: синхронные команды Tauri выполняются в главном потоке. Ответы теперь могут приходить не по порядку: `useFileTree` должен отбрасывать устаревшие результаты (тест).
  3. Семантика как у локальных команд:
     - лимиты 10/50 MB и `force`;
     - определение бинарника по NUL и невалидному UTF-8;
     - `gitignored = false`.
  4. mtime из SFTP в секундах переводится в миллисекунды.
- Готово: тесты на харнессе `sftp-server` из E3-T10: формы ответа совпадают с локальными для тех же файлов.
- Проверка: RS, FE.

#### E3-T12 · Изменения файлов на сервере и атомарное сохранение
- Модель: Opus · Ревью: Opus · M-L · Зависит: E3-T11
- Файлы: `fs/file.rs:127-158` (`fs_write_file`), `fs/mutate.rs:221-437` (`create`, `rename`, `move`, `delete`, `delete_batch`, `copy`), `remote/fs/dispatch.rs`.
- Шаги:
  1. Все команды мутаций (`fs/mutate.rs:222-411`) пометить `#[tauri::command(async)]` с неизменными локальными телами.
  2. Сохранение:
     - сначала `realpath(dst)`: если это симлинк, писать нужно в настоящий файл, а сам симлинк сохранить;
     - временный файл в каталоге настоящего файла со случайным суффиксом;
     - запись, затем `setstat(tmp, исходный mode)` до переименования, чтобы не было окна с правами по umask;
     - `posix-rename@openssh.com`, если сервер его объявил, иначе `mv -f` через base64-обёртку из E3-T07;
     - вернуть новый mtime.

     При ошибке временный файл удаляется. Если в каталог писать нельзя (EACCES), запасной вариант: запись на месте с предупреждением в UI.
  3. `fs:file-written` с меткой env.
  4. Все мутации проверяются через `RemoteRegistry` по realpath родителя и объекта (см. E3-T09).
  5. `move` с токеном конфликта как у локального; `delete_batch` рекурсивно с лимитом; `copy` с локальными источниками означает загрузку на сервер.
- Готово:
  - тесты на харнессе: атомарность (читатель никогда не видит частично записанный файл);
  - mode применён до переименования;
  - симлинк остаётся симлинком, меняется его цель;
  - отказ вне зарегистрированного корня, в том числе через симлинк наружу;
  - конфликт при move;
  - очистка временного файла при ошибке.
- Проверка: RS.

#### E3-T13 · Поиск и список файлов на сервере
- Модель: Sonnet · Ревью: Opus (команды строятся из шаблонов пользователя и AI) · M · Зависит: E3-T10
- Файлы: `fs/search.rs:47-228`, `fs/grep.rs:172-320`, новый `remote/search.rs`.
- Шаги:
  1. Команды `fs_search`, `fs_list_files` (`search.rs:48,157`), `fs_grep`, `fs_grep_interactive`, `fs_glob` (`grep.rs:173,215,267`) пометить `#[tauri::command(async)]`, локальные тела без изменений.
  2. Для `Ssh` через `exec` (base64-обёртка из E3-T07): если есть `rg` (`command -v rg`), то `rg --files` и `rg --json`; иначе `find` и `grep -rnI`. Флаги совместимы и с GNU, и с BSD (iMac): без `-printf`, без `--null-data`.
  3. Разбор вывода в чистых функциях; те же лимиты (`HARD_MAX_RESULTS` и т.д.), таймауты, кавычки через `posix_quote` внутри декодированного скрипта.
- Готово: тесты разбора на фикстурах вывода GNU, BSD и rg json; тест экранирования шаблона поиска с `'`, `$`, пробелами.
- Проверка: RS.

**Фаза 4: интерфейс Remote-SSH**

#### E3-T14 · Подключение, индикатор состояния, обновление дерева
- Модель: Sonnet · Ревью: нет · M-L · Зависит: E3-T03, E3-T05, E3-T08, E3-T11
- Файлы:
  - `src/modules/remote/` (`ConnectDialog.tsx`, `useRemoteConnection.ts`);
  - `src/modules/spaces/SpaceSwitcher.tsx`;
  - `src/modules/tabs/NewTabMenu.tsx`;
  - `command-palette/commands.ts`;
  - `src/modules/statusbar/` (новый индикатор; `WorkspaceEnvSelector.tsx:22-24` сейчас только для Windows);
  - `explorer/FileExplorer.tsx` (кнопка обновления).
- Шаги:
  1. "Connect to SSH host...": выбор хоста из списка E3-T06, затем новый space `{env: ssh, root: ~ или введённый путь}` с первой терминальной вкладкой.
  2. Пока `ssh_check` не прошёл, проводник показывает "Авторизуйтесь в терминале" и ждёт с backoff.
  3. В статус-баре `SSH: <host>` с состоянием (connecting / connected / lost) и действиями Reconnect и Disconnect.
  4. Дерево обновляется при раскрытии папки, при фокусе окна и по кнопке; опционально раз в N секунд для раскрытых папок.
  5. Шлюз на хост: при восстановлении SSH-space все вкладки стартовали бы одновременно, и без мастера каждая просила бы пароль и 2FA отдельно, плюс гонка мастеров. Сначала открывается первая ssh-вкладка; остальные ждут успешного `ssh_check` и только потом вызывают `pty_open`.
- Готово: тесты машины состояний подключения и шлюза (вторая вкладка не стартует до `ssh_check`).
- Проверка: FE.
- Руками: полный сценарий из `docs/fork/ssh-qa.md`.

#### E3-T15 · Ограничения в SSH-пространстве и AI только на чтение
- Модель: Sonnet · Ревью: Opus · M · Зависит: E3-T02, E3-T11
- Файлы:
  - `explorer/lib/contextActions.ts:17` (Reveal скрыть);
  - `useFileTree.ts` (`gitDecorations` выключить);
  - source-control (заглушка "Git недоступен для SSH, используйте терминал");
  - `editor/lib/externalFormat.ts:129` (без форматирования при сохранении);
  - `EditorPane.tsx:583-584` (медиа: заглушка вместо `convertFileSrc`);
  - `ai/tools/*.ts`;
  - `ai/lib/security.ts:265-322` (`canonicalize` для ssh через `fs_canonicalize` с env).
- Шаги:
  1. Всё через `envCaps`.
  2. AI в SSH-пространстве: проверка идёт по окружению, привязанному к запуску агента (E3-T03), а не по глобальному. При `aiRemoteToolsReadOnly` инструменты записи недоступны и возвращают понятную ошибку; команды недоступны всегда (D6); инструменты чтения проходят deny-list с удалённым realpath.
  3. Переключатель в ForkSection.
- Готово: тесты: AI-инструмент записи отклонён в ssh-space при включённой настройке; deny-list срабатывает на удалённом `~/.ssh/config` и `.env`.
- Проверка: FE.

#### E3-T16 · Конфликты сохранения и QA-сценарий
- Модель: Sonnet · Ревью: нет · S-M · Зависит: E3-T12, E3-T14
- Файлы: `editor/lib/useDocument.ts:78-97`, новый `docs/fork/ssh-qa.md`.
- Шаги:
  1. Для удалённых файлов сравнивать mtime с точностью до секунды.
  2. Чек-лист ручной проверки на OrbStack (Linux) и iMac (macOS):
     - подключение с паролем и ключом;
     - смена ключа хоста;
     - обрыв сети и Reconnect;
     - большой каталог;
     - сохранение при внешнем изменении;
     - поиск;
     - AI только на чтение;
     - закрытие приложения (мастера закрыты: `ls ~/.terax/cm` пусто).
- Готово: тест сравнения mtime; документ.
- Проверка: FE.

**Фаза 5: по желанию**

#### E3-T17 · Проброс портов для превью удалённого dev-сервера
- Модель: Sonnet · Ревью: Opus · M · Зависит: E3-T14
- Шаги:
  1. `ssh -S <ctl> -O forward -L 127.0.0.1:<local>:localhost:<remote> -- host`; список пробросов в индикаторе SSH.
  2. Когда в SSH-терминале появляется `localhost:<port>`, плашка предлагает "Пробросить и открыть превью".
  3. Отмена через `-O cancel`.

#### E3-T18 · Git на сервере
- Модель: Opus · Ревью: Opus · L · Зависит: E3-T12, E3-T13
- Ветка `Ssh` в `git/process.rs:307-332` через exec; проверка путей строками вместо локального canonicalize (`git/utils.rs:29-127`); файлы рабочей копии читаются через `RemoteFs`. Большая задача: делать, только если терминального git мало.

#### E3-T19 · Перетаскивание: загрузка на сервер и скачивание
- Модель: Sonnet · Ревью: нет · M · Зависит: E3-T12
- Перетаскивание из Finder в удалённое дерево (`useExplorerFileDrop.ts:72`) загружает файлы на сервер; пункт "Download" в контекстном меню скачивает в `~/Downloads`.

#### E3-T20 · HTML-превью удалённых файлов
- Модель: Sonnet · Ревью: Opus · M · Зависит: E2-T02, E3-T11
- Обработчик `terax-preview` для корней из SSH-пространства читает файлы через `RemoteFs` с теми же проверками.

### E4. Дополнительно (просьба 4)

Отчёт: `gaps.md`. Каждая задача независима и необязательна, кроме E4-T01 (она в W1).

#### E4-T01 · Cmd+D, Cmd+[, Cmd+], Cmd+G в редакторе не перехватываются
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T01
- Файлы: `src/app/App.tsx:980-1031` (`shortcutsDisabled`), `src/modules/shortcuts/shortcuts.ts` (id хоткеев), новый `src/modules/shortcuts/lib/editorOwnedShortcuts.ts` с тестом.
- Шаги:
  1. Чистая функция `isEditorOwnedShortcut(id, target)`: true, когда фокус внутри `.cm-editor`, для id (`shortcuts.ts:139-190`):
     - `pane.splitRight` (Cmd+D);
     - `pane.splitDown` (Cmd+Shift+D);
     - `pane.focusNext` (Cmd+]);
     - `pane.focusPrev` (Cmd+[);
     - `pane.source` (Cmd+G, "Toggle source panel", D9).

     Проверка по фокусу, а не по виду вкладки: из сайдбара Cmd+G по-прежнему открывает Source Control, а Cmd+G в поле коммита (`SourceControlPanel.tsx:494`) перестаёт глотаться.
  2. Вызвать её в `shortcutsDisabled`.

  Сейчас глобальный обработчик в capture-фазе (`useGlobalShortcuts.ts:27-43`) съедает эти клавиши, и в редакторе не работают выделение следующего вхождения (мультикурсор), отступы и "найти следующее".
- Готово: тест функции; в терминальных вкладках поведение прежнее.
- Проверка: FE.
- Руками: Cmd+D в HTML-файле выделяет следующее вхождение.

#### E4-T02 · Хоткеи на русской раскладке
- Модель: Sonnet · Ревью: нет · S-M · Зависит: шаг U3 (раздел 8)
- Файлы: `src/modules/shortcuts/shortcuts.ts:432-454` (`matchBinding`), `shortcuts.test.ts:118-125` (этот тест не ломать), `src/settings/sections/ShortcutsSection.tsx:293-299`, `src/modules/source-control/SourceControlPanel.tsx:494,627`.
- Шаги:
  1. Если `e.key` это буква не латиницы (`/\p{L}/u` и не `\p{Script=Latin}`), сравнивать по `e.code`.
  2. Запись пользовательских хоткеев сохраняет клавишу из `e.code` для букв и цифр.
  3. Проверки `e.key` в SourceControlPanel заменить на общий хелпер.
- Готово: тесты с кириллицей (Cmd+е для Cmd+T и т.п.); тест AZERTY/Option остаётся зелёным.

#### E4-T03 · Кликабельные path:line:col в выводе терминала
- Модель: Sonnet · Ревью: нет · M · Зависит: E1-T06
- Файлы: `ghostty/core/terminalLinks.ts:3-38`, `useGhosttyTerminalSession.ts:709-711` (`onOpenLink`), `terminal/lib/terminalSessionApi.ts:59` (`leafCwd`), `App.tsx:1313` (`openControlFile`).
- Шаги:
  1. Распознавать пути: абсолютные, `./`, `../`, `~/`, относительные с расширением, с необязательными `:line(:col)`.
  2. Открывать только по Cmd+клику, чтобы не мешать E1-T06.
  3. Путь разрешается относительно cwd панели; существование проверяется через `fs_stat`; файл открывается в редакторе на нужной строке.
  4. Только локальные вкладки (для SSH позже, через env).
- Готово: тесты детектора (вывод tsc, rustc, eslint, pytest, `git grep -n`).

#### E4-T04 · Сохранённые команды и ввод во все панели
- Модель: Sonnet · Ревью: нет · M · Зависит: E0-T06
- Шаги:
  1. Хранилище команд в `tauri-plugin-store`; режим палитры с новым префиксом (`command-palette/lib/mode.ts`); вставка через `writeToSession` (`terminalSessionApi.ts:43`).
  2. Переключатель "broadcast" для вкладки (`PaneTreeView.tsx`) дублирует ввод во все панели вкладки (до 4).
  3. Приватные вкладки не участвуют в broadcast.

#### E4-T05 · Уведомление о завершении долгой команды
- Модель: Sonnet · Ревью: нет · S-M · Зависит: E0-T06
- Шаги:
  1. По длительности между OSC 133 C и D больше порога `notifyLongCommandsSeconds`, если окно не в фокусе или вкладка скрыта, показать уведомление через `agents/lib/route.ts`.
  2. С кодом выхода и названием команды; без текста вывода.
  3. На сервере работает после E3-T08.

#### E4-T06 · Cmd+P: быстрое открытие файла
- Модель: Sonnet · Ревью: нет · S-M · Зависит: E3-T04 (через `fsFor`)
- В режиме палитры по умолчанию к командам подмешиваются результаты `fs_search` по корню проводника; Enter открывает файл.

#### E4-T07 · Копирование при выделении
- Модель: Haiku · Ревью: нет · S · Зависит: E0-T06
- По окончании выделения в `TerminalSelectionController`, если включён `terminalCopyOnSelect`, текст копируется через нативный clipboard-плагин (как в `terminalClipboard.ts`). Приватные вкладки тоже копируют: это локальный буфер.

#### E4-T08 · Emmet для HTML
- Модель: Sonnet · Ревью: нет · S-M · Зависит: E2-T04
- Подключить `@emmetio/codemirror6-plugin` лениво вместе с HTML-языком (`editor/lib/languageDefinitions.ts:170-178`).
- Проверить бюджет: запас всего ~30 KB gzip. Если не влезает, поднять лимит в `.size-limit.json` форка осознанно и отметить в коммите.

#### E4-T09 · Замена по проекту
- Модель: Sonnet · Ревью: Opus · L · Зависит: E3-T04
- Новая Rust-команда с авторизацией корня, режим предпросмотра изменений, применение пакетом с проверкой mtime каждого файла.

#### E4-T11 · Установленные шрифты (Nerd Font) в терминале на macOS
- Модель: Sonnet · Ревью: Opus (новая IPC-поверхность, читающая файлы, и разбор бинарных данных) · M · Зависит: нет
- Зачем, проверено пробой в WKWebView на этом Mac (2026-10-09):
  - DOM-текст видит шрифты из `~/Library/Fonts`, а canvas 2D их не видит: у `"JetBrainsMono Nerd Font", serif` ширина такая же, как у serif. Это защита WebKit от отпечатков;
  - терминал Ghostty растеризует глифы через canvas (`WebGlGlyphAtlas.rasterize`, `gpu/GlyphAtlas.ts`), поэтому любой пользовательский шрифт, включая Nerd Font, не применяется, а значки prompt становятся квадратиками;
  - автоопределение `detectMonoFontFamily` (`src/lib/fonts.ts`) тоже меряет через canvas и поэтому никогда не находит Nerd Font;
  - шрифт, загруженный из байтов через `new FontFace(name, ArrayBuffer)` и `document.fonts.add`, canvas видит: ширина и глиф U+E0A0 корректны.
- Файлы:
  - новый `src-tauri/src/modules/fonts.rs` (регистрация в `modules/mod.rs` и `lib.rs`);
  - новый `src/lib/userFonts.ts` с тестом;
  - `src/lib/fonts.ts` и `fonts.test.ts`;
  - место, где терминал берёт шрифт (`useTerminalFont`, `useGhosttyTerminalSession.ts` около 160, `ensureMonoFontsLoaded`);
  - при необходимости сброс атласа глифов при смене версии шрифтов.
- Шаги:
  1. **Rust, индекс шрифтов.** Лениво, один раз, с кэшем в State. Сканирует каталоги:
     - macOS: `~/Library/Fonts`, `/Library/Fonts`;
     - Linux: `~/.local/share/fonts`, `~/.fonts`, `/usr/share/fonts`, `/usr/local/share/fonts`.

     Ограничения: глубина 3, лимит файлов, файлы до 50 MB, расширения `ttf`, `otf`, `ttc`. Системные шрифты не нужны: их canvas видит. Без новых зависимостей: ручной разбор sfnt, таблица `name` (nameID 16, иначе 1, для семейства; 17, иначе 2, для начертания) и `OS/2` (`usWeightClass`, бит italic в `fsSelection`). Для `ttc` разбирать все лица. Разбор чистыми функциями с тестами на синтетических байтах, включая обрезанные и битые файлы (никаких panic и выходов за границы).
  2. **Команды:**
     - `fonts_resolve(family)`: совпадение без учёта регистра и, отдельно, по компактному имени без пробелов (`JetBrainsMonoNerdFont` даёт семейство `JetBrainsMono Nerd Font`, `JetBrainsMono` даёт `JetBrains Mono`). Возвращает `{family, faces: [{id, weight, italic}]}`;
     - `fonts_nerd_fallback()`: имя первого установленного семейства с "Nerd Font Mono" в названии, иначе с "Nerd Font", в порядке `NERD_FONT_CANDIDATES`;
     - `font_read(id)`: байты через `tauri::ipc::Response`, без base64. Принимает только id из индекса, никаких произвольных путей из webview.
  3. **Фронтенд `userFonts.ts`:**
     - `ensureUserFonts(prefFamily)`: при пустой настройке берёт семейство из `fonts_nerd_fallback`;
     - грузит до 4 лиц (400 и 700, normal и italic, плюс настроенная толщина, если есть) через `FontFace` + `document.fonts.add`;
     - если основное семейство не Nerd, отдельно грузит Nerd-семейство для запасного стека;
     - кэш по семейству; ошибки не роняют терминал (откат на текущий стек).
  4. **`resolveFontFamily`:** стек = основное семейство (каноническое имя из индекса, если найдено), затем загруженный Nerd fallback, затем `FALLBACK_CHAIN`. Чистая функция с тестами.
  5. **Применение к терминалу.** Терминал ждёт `ensureUserFonts` при старте сессии (рядом с `ensureMonoFontsLoaded`) и при смене настройки шрифта. После загрузки атлас глифов перестраивается, а уже нарисованные квадратики не остаются в кэше: через номер версии шрифтов в спецификации шрифта или явный сброс атласа.
  6. Подсказка в настройках шрифта: при пустом поле используется установленный Nerd Font.
- Готово:
  - Rust-тесты разбора и сопоставления имён, отказ `font_read` для неизвестного id;
  - TS-тесты стека и кэша;
  - пользовательский `JetBrainsMono Nerd Font`, `JetBrainsMonoNerdFont`, `JetBrainsMono` и пустое поле дают в терминале значки Nerd, проверено руками.
- Проверка: FE, RS, SIZE.

#### E4-T10 · Русский интерфейс (только набросок, не в расписании)
- Если понадобится: лёгкий словарь без библиотеки, `t(key)` с ленивой загрузкой русской локали, начать с настроек и контекстных меню. Оценка: L, много конфликтов с апстримом.

---

## 6. Горячие файлы и порядок слияния

| Файл | Задачи | Как избегаем конфликтов |
|---|---|---|
| `src/app/App.tsx` | E0-T03, E1-T01, E2-T04, E2-T06, E3-T03, E3-T06, E3-T14, E4-T01, E4-T03 | В каждой задаче правка в 1-5 строк, логика в новых модулях. Внутри волны сливать в порядке из таблицы волн |
| `src/modules/settings/store.ts` | E0-T06, потом только чтение | Все ключи форка добавляются один раз в E0-T06 |
| `src/settings/sections/ForkSection.tsx` | E1-T01, E1-T03, E1-T06, E3-T08, E3-T15, E4-T05, E4-T07 | Каждая задача добавляет одну строку в конец; конфликты тривиальные |
| `useGhosttyTerminalSession.ts` | E1-T03, E1-T04, E1-T06, E3-T03, E4-T03 | E1-T04, затем E1-T06 в W2; E1-T03 и E3-T03 в W3 вливаются после |
| `GhosttyInputController.ts` | E1-T03, E1-T07 | Обе в W3, разные методы; сливать T03, затем T07 |
| `src/modules/tabs/lib/useTabs.ts` | E2-T04, E2-T05 | T04, затем T05 |
| `src-tauri/src/lib.rs` | E0-T03, E2-T02, E3-T06, E3-T07, E3-T10 | Каждая добавляет строки в `generate_handler!` и builder; сливать по волнам |
| `src-tauri/src/modules/workspace.rs` | E3-T01, E3-T08, E3-T09 | Строго последовательно |
| `fs/*.rs` | E2-T02 (`watch.rs`), E3-T01, E3-T03, E3-T11, E3-T12, E3-T13 | E3-T01 вливается первой в W2, E2-T02 после неё; T11, T12, T13 трогают разные функции |
| `src/modules/markdown/MarkdownPreviewPane.tsx` | E2-T05, E3-T03 | Обе в W3: сначала E2-T05, затем E3-T03 |
| `src/modules/ai/lib/native.ts`, `ai/tools/*` | E3-T03, E3-T15 | Последовательно, T15 в W6 |
| `terminal/lib/osc-handlers.ts`, `GhosttySemanticEventRouter.ts` | E3-T05 | Только одна задача; E1-T04 подписывается на события в другом файле |

Связи между эпиками:
- E2-T01 (deny-list на Rust) нужен E2-T02 и E3-T15.
- E3-T08 (интеграция на сервере) включает Blocks, клик E1-T06 и уведомления E4-T05 на удалённых машинах.
- E3-T04 (`fsFor`) нужен E4-T06 и E4-T09.
- E2-T02 и E3-T11 вместе дают E3-T20.

---

## 7. Риски

- **Апстрим сделает свой SSH.** Наш код изолирован в `remote/*`, но при мерже возможна переделка.
- **Бюджет JS.** Запас ~30 KB gzip. Все новые экраны только через `lazy()`, SIZE после каждой волны.
- **Клик по курсору в v1 приблизителен.** Серая автоподсказка принимается кликом; многострочный ввод не поддерживается; при очень быстром вводе возможен сдвиг на символ. Лечится E1-T08 или E1-T09.
- **WebGPU на macOS 27 падает, рендерит WebGL.** Ручные проверки делать на этой машине и в этом режиме.
- **ControlMaster.**
  - Серверы с `MaxSessions < 2` или `AllowTcpForwarding no` ломают мультиплексирование и проброс.
  - Старые сокеты после краша: E3-T07 проверяет и чистит.
  - Длина пути сокета ограничена.
- **SFTP v3.** mtime в секундах; без `posix-rename` атомарность через `mv -f`; нет уведомлений об изменениях (обновление по фокусу и кнопке).
- **BSD на iMac.** Поиск и команды должны быть совместимы; фикстуры BSD обязательны в E3-T13.
- **Shell-integration пишет файлы на сервер.** Это `~/.cache/terax`; отключается настройкой (D5).
- **Общие данные с установленным Terax.** Keychain общий (так задумано); `control.json` разводится в E0-T03; dev и форк получают свои каталоги настроек.
- **Запросы доступа к keychain.** Dev- и форк-сборки подписаны ad-hoc, поэтому macOS будет спрашивать доступ к ключам, созданным Terax.app, после каждой пересборки. Нажимать "Разрешить всегда"; это неизбежно без Apple Developer подписи.
- **Удалённая оболочка разбирает команды заново.** Поэтому все exec идут через base64-обёртку (E3-T07). Обойти её в новой задаче значит открыть дыру; ревьюеры проверяют это отдельно.

---

## 8. Что сделать тебе (ручные шаги и проверки)

- **U1, до W0 (по желанию):**

  ```bash
  brew install cargo-nextest
  ```

  Постоянный pnpm: `source ~/.nvm/nvm.sh && nvm use 22 && corepack enable pnpm`.
- **U2, до W4:** тестовая Linux-машина в OrbStack, затем проверить `ssh <машина>@orb` (или алиас из `~/.orbstack/ssh/config`):

  ```bash
  orb create ubuntu terax-ssh
  ```

  Для macOS-сервера включить "Удалённый вход" на iMac.
- **U3, до E4-T02 (5 минут):**
  1. `pnpm fork:dev` (после W0), в окне правый клик и "Inspect Element". `pnpm dev:inspect` для этого не подходит: это инспектор графа модулей vite.
  2. Выполнить `addEventListener('keydown', e => console.log(e.key, e.code, e.metaKey), true)`.
  3. На русской раскладке нажать Cmd+T, Cmd+], Cmd+`,`, Cmd+Shift+T, Option+B и прислать вывод. Если `e.key` латинский, E4-T02 не нужна.
- **После W1:**
  - Cmd+D в HTML-файле;
  - режим Blocks по умолчанию;
  - Cmd+Z и Fn+Delete-комбинации в zsh;
  - палитра `SSH: ...`.
- **После W2:**
  - клик по строке в zsh, bash, fish;
  - HTML Split с автообновлением.
- **После W3:**
  - Option-клавиши (в том числе «» правым Option);
  - колесо в `less`;
  - Markdown обновляется сам;
  - SVG.
- **После W4-W6:** сценарий `docs/fork/ssh-qa.md`, установка `scripts/fork/release.sh --install`.

---

## 9. Сводка задач

| ID | Задача | Модель | Ревью | Размер | Волна |
|---|---|---|---|---|---|
| E0-T01 | Git для форка | главный агент | нет | S | W0 |
| E0-T02 | CLAUDE.md, FORK.md, agent-env | Haiku | нет | S | W0 |
| E0-T03 | Идентичность сборки, апдейтер выкл. | Sonnet | нет | M | W0 |
| E0-T04 | Версии @tauri-apps (опц.) | Haiku | нет | S | W0 |
| E0-T05 | Скрипт релиза | Haiku | нет | S | W0 |
| E0-T06 | Каркас настроек форка | Haiku | нет | S | W0 |
| E1-T01 | Classic/Blocks по умолчанию | Haiku | нет | S | W1 |
| E1-T02 | Readline-хоткеи | Haiku | нет | S | W1 |
| E1-T03 | Option: баг 229 и Meta | Sonnet | Sonnet | M | W3 |
| E1-T04 | Трекер фаз и viewportLine | Sonnet | нет | M | W2 |
| E1-T05 | promptClickDelta | Sonnet | нет | M | W2 |
| E1-T06 | Клик ставит курсор | Sonnet | Sonnet | M | W2 |
| E1-T07 | Колесо в less/man | Sonnet | нет | S | W3 |
| E1-T08 | OSC 133 B в конец PS1 (опц.) | Sonnet | Sonnet | S-M | W7 |
| E1-T09 | Zig: точное начало ввода (отложено) | Opus | Opus | L | W7 |
| E1-T10 | Запуск скриптов (.command) в терминале | Sonnet | Opus | M | W1 |
| E2-T01 | Rust deny-list | Sonnet | Opus | S-M | W1 |
| E2-T02 | Протокол terax-preview | Opus | Opus | L | W2 |
| E2-T03 | Фрейм превью и перезагрузка | Sonnet | Opus | M | W2 |
| E2-T04 | Code/Split/Preview | Sonnet | нет | M | W2 |
| E2-T05 | Markdown live reload, дубли | Sonnet | нет | S-M | W3 |
| E2-T06 | SVG и Open Preview | Sonnet | нет | S | W3 |
| E2-T07 | Сузить asset-протокол (опц.) | Sonnet | Opus | M | W7 |
| E3-T01 | Rust Ssh + fail-closed | Opus | Opus | L | W2 |
| E3-T02 | TS-модель ssh | Sonnet | нет | M | W2 |
| E3-T03 | Env из вкладки, события с env | Sonnet | Opus | M-L | W3 |
| E3-T04 | Провайдер fsFor | Sonnet | нет | M | W3 |
| E3-T05 | Хост в OSC 7 | Sonnet | нет | S | W3 |
| E3-T06 | Выбор SSH-хоста | Sonnet | Opus | M | W1 |
| E3-T07 | ControlMaster, кавычки, exec | Opus | Opus | L | W4 |
| E3-T08 | Удалённый PTY + интеграция | Opus | Opus | L | W4 |
| E3-T08b | Вход в SSH-пространство | Sonnet | нет | S | W4 |
| E3-T09 | Реестр удалённых корней | Sonnet | Opus | M | W4 |
| E3-T10 | SFTP-клиент | Opus | Opus | L | W5 |
| E3-T11 | Чтение файлов сервера | Sonnet | Opus | M | W5 |
| E3-T12 | Запись и атомарное сохранение | Opus | Opus | M-L | W5 |
| E3-T13 | Поиск на сервере | Sonnet | Opus | M | W5 |
| E3-T14 | Подключение и индикатор | Sonnet | нет | M-L | W6 |
| E3-T15 | Ограничения и AI read-only | Sonnet | Opus | M | W6 |
| E3-T16 | Конфликты сохранения, QA | Sonnet | нет | S-M | W6 |
| E3-T17 | Проброс портов (опц.) | Sonnet | Opus | M | W7 |
| E3-T18 | Git на сервере (опц.) | Opus | Opus | L | W7 |
| E3-T19 | Загрузка/скачивание (опц.) | Sonnet | нет | M | W7 |
| E3-T20 | HTML-превью удалённых файлов (опц.) | Sonnet | Opus | M | W7 |
| E4-T01 | Хоткеи редактора | Haiku | нет | S | W1 |
| E4-T02 | Русская раскладка | Sonnet | нет | S-M | W3 |
| E4-T03 | path:line ссылки (опц.) | Sonnet | нет | M | W7 |
| E4-T04 | Сохранённые команды, broadcast (опц.) | Sonnet | нет | M | W7 |
| E4-T05 | Уведомления о долгих командах | Sonnet | нет | S-M | W6 |
| E4-T06 | Cmd+P файлы (опц.) | Sonnet | нет | S-M | W7 |
| E4-T07 | Копирование при выделении (опц.) | Haiku | нет | S | W7 |
| E4-T08 | Emmet (опц.) | Sonnet | нет | S-M | W7 |
| E4-T09 | Замена по проекту (опц.) | Sonnet | Opus | L | W7 |
