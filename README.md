# distribution_service

RSS-генератор для платформы подкастов. Принимает `playlist_id`, читает данные из
БД `podcast-core` (read-only) и отдаёт корректный podcast-RSS с iTunes namespace.

Mapping: **playlist → RSS channel (шоу)**, **podcast → RSS item (эпизод)**.

## Стек

- Rust 1.83 + axum 0.7
- sqlx (Postgres, runtime-queries — без compile-time DB)
- quick-xml для сборки RSS с iTunes/`content:encoded` namespaces
- reqwest для HTTP-вызовов в Auth-service (получение email владельца)
- utoipa + Swagger UI (`/swagger-ui`)

## Внешние зависимости

| Сервис | Зачем | Что будет, если упал |
|---|---|---|
| Postgres `podcast_db` (read-only) | основной источник данных | `503` на любом запросе |
| **Auth-service** `/internal/users/{user_id}` | email владельца плейлиста для `<itunes:owner><itunes:email>` | фид строится, но без `<itunes:email>` (логируется warning) |

## Endpoints

| Метод | Путь | Описание |
|---|---|---|
| GET | `/feed/{playlist_id}` или `/feed/{playlist_id}.xml` | RSS-фид плейлиста |
| GET | `/health` | Liveness + DB-пинг |
| GET | `/swagger-ui` | OpenAPI документация |

Фид отдаётся с `Content-Type: application/rss+xml; charset=utf-8` и
`Cache-Control: public, max-age=300`. Возвращает:

- `200 OK` — фид сгенерирован
- `400 Bad Request` — невалидный UUID.
- `403 Forbidden` — плейлист найден, но `is_public = false`.
- `404 Not Found` — плейлист не найден.

В фид попадают только эпизоды со статусом `PUBLISHED`, **авторство которых
принадлежит владельцу плейлиста** (`author_profiles.user_profile_id =
playlists.owner_profile_id`) — чужие подкасты, добавленные в плейлист,
отбрасываются. Эпизоды отсортированы по `playlist_podcasts.position`.

## Конфигурация (env)

| Переменная | Default | Описание |
|---|---|---|
| `DATABASE_URL` | — (обязательна) | Postgres DSN, например `postgres://podcast_user:podcast_pass@localhost:5433/podcast_db` |
| `BIND_ADDR` | `0.0.0.0:8788` | Адрес HTTP-сервера (8787 занят `podcast-backend`) |
| `PUBLIC_BASE_URL` | `http://localhost:8788` | База для `<link>` канала |
| `DB_MAX_CONNECTIONS` | `10` | Размер пула sqlx |
| `DB_CONNECT_TIMEOUT_SECONDS` | `5` | Таймаут acquire |
| `AUTH_SERVICE_URL` | `http://localhost:8080` | Базовый URL Auth-service. Дёргает `GET {url}/internal/users/{user_id}` |
| `AUTH_INTERNAL_API_TOKEN` | (пусто) | Shared-secret для `/internal/*`. Должен совпадать с `INTERNAL_API_TOKEN` в Auth-service. Если пусто — заголовок Bearer не отправляется, Auth-service вернёт 403, фид соберётся без email |
| `AUTH_CACHE_TTL_SECONDS` | `3600` | TTL in-memory кэша на ответы Auth-service |
| `RUST_LOG` | `distribution_service=info,tower_http=info,sqlx=warn` | Уровни логов |

См. [.env.example](.env.example).

## Локальный запуск

### Вариант A — всё через docker compose (включая Postgres)

```bash
cp .env.example .env
docker compose up --build
```

Postgres поднимется пустым. Для нормальной работы нужны таблицы и данные —
проще всего использовать вариант B и подключиться к уже работающей БД `podcast-core`.

### Вариант B — рядом с уже запущенным podcast-core

Поднимаем podcast-core (он же применит миграции, включая `V3__add_podcast_audio_size_bytes.sql`):

```bash
cd ../Podcast_core
docker compose up -d postgres app
```

Затем запускаем distribution_service напрямую (быстрее, чем через docker):

```bash
cd ../distribution_service
cp .env.example .env
cargo run
```

Сервис слушает на `http://localhost:8788`. Swagger: `http://localhost:8788/swagger-ui`.

Проверка:

```bash
# создай публичный плейлист с эпизодами через API podcast-core,
# затем подставь его UUID:
curl -i http://localhost:8788/feed/<PLAYLIST_UUID>.xml
```

## Продакшен

В прод-режиме сервис подключается к управляемой БД, локальный Postgres из
`docker-compose.yml` не нужен.

```bash
DATABASE_URL='postgres://reader:***@db.internal:5432/podcast_db' \
PUBLIC_BASE_URL='https://feeds.example.com' \
AUTH_SERVICE_URL='https://auth.internal' \
AUTH_INTERNAL_API_TOKEN="$(cat /run/secrets/auth_internal_token)" \
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d distribution_service
```

`AUTH_INTERNAL_API_TOKEN` — длинный случайный shared-secret
(`openssl rand -hex 32`). Должен совпадать с `INTERNAL_API_TOKEN` в
переменных Auth-service. См. [Auth-service README](../Auth-service/README.md)
про endpoint `/internal/users/{user_id}`.

Рекомендации для прода:

- Используй **read-only роль** Postgres с `GRANT SELECT` на таблицы
  `playlists`, `playlist_podcasts`, `podcasts`, `author_profiles`, `user_profiles`.
- Поставь сервис за reverse proxy (nginx/traefik) с TLS.
- Сервис stateless — горизонтальное масштабирование безопасно.
- При высокой нагрузке добавь Redis-кэш на готовый XML (`Cache-Control` уже
  выставляется, ETag добавится позже).

## Миграции

distribution_service не владеет схемой — миграции живут в `podcast-core`.
Добавленные для этого сервиса миграции:

- `V3__add_podcast_audio_size_bytes.sql` — колонка `podcasts.audio_size_bytes`
  для атрибута `<enclosure length="...">` в RSS.
- `V4__add_podcast_audio_url_file.sql` — колонка `podcasts.audio_url_file`,
  прямая ссылка на исходный файл (mp3/ogg). Используется как `<enclosure url>`,
  потому что в `audio_url` лежит HLS-плейлист (`.m3u8`), а большинство
  подкаст-клиентов HLS не понимают.

Оба поля заполняются media-worker'ом одновременно: source-файл грузится рядом
с HLS-артефактами под `media/<file_id>/source.<ext>`, размер — через
`fs::metadata.len()`. См. [media-worker kafka-contract](../Media_worker/docs/kafka-contract.md).

Podcast-core должен слушать топик `media.worker` и на событие `converted`
обновлять строку подкаста (`audio_url`, `audio_url_file`, `audio_size_bytes`,
`duration_seconds`, `status='PUBLISHED'`). Этот consumer — единственное
звено, которого пока нет в проде; distribution_service ждёт, что данные
в БД уже есть.

## Структура

```
src/
├── main.rs           bootstrap: config, pool, auth client, router, graceful shutdown
├── config.rs         AppConfig из env
├── error.rs          AppError + IntoResponse → JSON
├── state.rs          AppState { pool, public_base_url, auth }
├── models.rs         Playlist, Episode
├── categories.rs     compile-time таблица Apple Podcasts таксономии
│                     + резолвер internal name → (parent, child)
├── auth_client.rs    HTTP-клиент к Auth-service с TTL-кэшем; на сбой
│                     отдаёт None — фид всё равно строится
├── db/
│   ├── mod.rs
│   └── queries.rs    fetch_playlist, fetch_episodes (фильтр
│                     status='PUBLISHED' AND audio_url_file IS NOT NULL)
├── rss/
│   ├── mod.rs
│   └── builder.rs    quick-xml → RSS 2.0 + itunes ns + content:encoded
└── routes/
    ├── mod.rs        router + OpenApi doc
    ├── feed.rs       GET /feed/{id}
    └── health.rs     GET /health
```
