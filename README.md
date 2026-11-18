# Очередь / ДВФУ

Сервис записи студентов на показы. Приложение разделено на три контейнерных сервиса:

- `frontend` — Vue 3, собранный в статические файлы и раздаваемый Nginx;
- `backend` — REST API на Rust/Axum;
- `mariadb` — постоянное хранилище аккаунтов, очередей и записей.

Nginx проксирует `/api/` в backend. MariaDB не доступна из внешней сети: её порт привязан только к loopback хоста для локальной разработки.

## Запуск на Linux-хосте

Установите Docker Engine и Compose v2 по официальным инструкциям для [Ubuntu](https://docs.docker.com/engine/install/ubuntu/) или [Debian](https://docs.docker.com/engine/install/debian/). Если Compose plugin не установлен, добавьте [Docker Compose plugin](https://docs.docker.com/compose/install/linux/).

### Вариант 1: собрать образы на своём компьютере и перенести на сервер

Этот вариант подходит, если у сервера мало ресурсов. Сборку выполняйте на компьютере с установленным Docker и Compose v2. Архитектура образов должна совпадать с сервером: проверьте её командой `uname -m` на сервере (`x86_64` соответствует `linux/amd64`, `aarch64` — `linux/arm64`). Ниже команды для `x86_64` / `linux/amd64`.

На компьютере с проектом соберите backend и frontend и скачайте образ MariaDB:

```sh
docker buildx build --platform linux/amd64 --load -t queue-backend:latest ./backend
docker buildx build --platform linux/amd64 --load -t queue-frontend:latest ./frontend
docker pull --platform linux/amd64 mariadb:11.4
docker save -o queue-images.tar queue-backend:latest queue-frontend:latest mariadb:11.4
```

Передайте `queue-images.tar`, `docker-compose.yml` и `.env.example` на сервер (например, `scp`). На сервере в каталоге проекта загрузите образы и настройте секреты:

```sh
docker load -i queue-images.tar
cp .env.example .env
openssl rand -hex 32
openssl rand -hex 32
openssl rand -hex 32
nano .env
```

Вставьте три разных значения в `QUEUE_JWT_SECRET`, `DB_PASSWORD` и `MARIADB_ROOT_PASSWORD`. Запустите контейнеры без `--build`:

```sh
docker compose up -d
docker compose ps
docker compose logs -f
```

При обновлении передайте новый `queue-images.tar` и Compose-файл, затем выполните `docker load -i queue-images.tar && docker compose up -d`. Если сервер не `x86_64`, замените `linux/amd64` при сборке/скачивании на архитектуру сервера.

### Вариант 2: собрать непосредственно на сервере

Из корневого каталога проекта создайте файл настроек:

```sh
cp .env.example .env
openssl rand -hex 32
openssl rand -hex 32
openssl rand -hex 32
nano .env
```

Вставьте три сгенерированных значения в `QUEUE_JWT_SECRET`, `DB_PASSWORD` и `MARIADB_ROOT_PASSWORD`. Используйте отдельное значение для каждой переменной. После этого запустите:

```sh
docker compose up -d --build
docker compose ps
docker compose logs -f
```

Откройте `http://АДРЕС-СЕРВЕРА:8080`. Порт можно изменить через `FRONTEND_PORT` в `.env`. Backend и MariaDB наружу не публикуются; порт MariaDB доступен только на `127.0.0.1:3307`.

Для домена настройте HTTPS reverse proxy на `127.0.0.1:8080` и установите `QUEUE_COOKIE_SECURE=true` в `.env`, затем выполните `docker compose up -d`.

При старте backend применяет SQLx-миграции автоматически. После изменения кода обновите сервисы:

```sh
git pull
docker compose up -d --build
```

Остановить приложение, сохранив базу:

```sh
docker compose down
```

**Не используйте `docker compose down -v`, если нужно сохранить данные**: команда удалит volume MariaDB.

## Первый администратор

При запуске **не создаются аккаунты**. Сначала зарегистрируйте свой аккаунт через интерфейс. Регистрация всегда назначает роль `user`. Чтобы выдать себе роль администратора, выполните на хосте:

```sh
docker compose exec mariadb mariadb -u root -p queue
```

Введите значение `MARIADB_ROOT_PASSWORD` из `.env`, затем в консоли MariaDB назначьте роль:

```sql
UPDATE accounts SET role = 'admin' WHERE email = 'ваша-почта@example.com';
SELECT email, role FROM accounts;
```

Backend читает роль из БД на каждом запросе, поэтому после назначения роли обновите страницу в браузере; перезапуск backend не требуется. Обычные пользователи и гости могут записываться и удалять только собственную запись; администраторы могут создавать, редактировать, закрывать и удалять любые очереди и участников. Повторную запись блокируют аккаунт и постоянный HttpOnly ID браузера.

## Локальная разработка

Для backend/frontend без Docker запустите MariaDB из Compose. Сначала заполните `.env`, затем:

```sh
docker compose up -d mariadb
set -a
. ./.env
set +a
export DATABASE_URL="mysql://queue_app:${DB_PASSWORD}@127.0.0.1:${DB_PORT:-3307}/queue"
cargo run --manifest-path backend/Cargo.toml
```

В другом терминале:

```sh
cd frontend
npm ci
npm run dev
```

Откройте адрес Vite из терминала. По умолчанию запросы `/api` проксируются в `http://localhost:3000`. Для другого адреса задайте `VITE_API_PROXY_TARGET`.

## Схема и сохранность данных

Миграции находятся в `backend/migrations/` и выполняются SQLx при старте API. Схема нормализована:

- `accounts` — почта, Argon2-хеш пароля и роль;
- `queues` — очередь и ссылка на создавший её аккаунт;
- `queue_entries` — отдельные записи, связанные с очередью, аккаунтом и ID браузера;
- `revoked_tokens` — отозванные JWT до истечения срока действия.

Ограничения и внешние ключи базы защищают уникальность email, повторную запись одного аккаунта или браузера в очередь, дубли фамилий в очереди и целостность ссылок. Секрет выхода гостя хранится в таблице только в виде SHA-256-хеша. Для гостей ограничение действует на браузер; очистка cookie или другой браузер создаёт новую анонимную идентичность.

Данные MariaDB хранятся в Docker volume `mariadb_data` и переживают пересоздание контейнеров. Секрет JWT должен быть постоянным и не короче 32 байт; backend завершится с ошибкой, если переменная `QUEUE_JWT_SECRET` не задана или слишком короткая. В production задайте `QUEUE_COOKIE_SECURE=true` для HTTPS.

Сделать SQL-дамп базы можно так:

```sh
docker compose exec -T mariadb sh -c 'mariadb-dump -u root -p"$MARIADB_ROOT_PASSWORD" queue' > queue-backup.sql
```

## API

- Авторизация: `POST /api/auth/register`, `POST /api/auth/login`, `GET /api/auth/me`, `POST /api/auth/logout`.
- Очереди: `GET/POST /api/queues`, `POST /api/queues/:id/join`, `PUT/DELETE /api/queues/:id`.
- Участники: `DELETE /api/queues/:id/entries/:entry_id` (выход по секрету записи), `DELETE /api/queues/:id/manage/entries/:entry_id` (удаление администратором).
- Состояние backend: `GET /health`.
