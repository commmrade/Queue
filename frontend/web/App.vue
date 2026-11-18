<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'

const api = '/api/queues'
const queues = ref([])
const user = ref(null)
const active = ref(null)
const loading = ref(true)
const loadError = ref('')
const createOpen = ref(false)
const queueOpen = ref(false)
const authOpen = ref(false)
const authMode = ref('register')
const authForm = ref({ email: '', password: '' })
const editingQueue = ref(null)
const createForm = ref({ title: '', course: '', teacher: '' })
const joinName = ref('')
const toastMessage = ref('')
let toastTimer
let refreshTimer

const queueCount = computed(() => {
  const count = queues.value.length
  if (count === 1) return '1 ОЧЕРЕДЬ'
  if (count > 1 && count < 5) return `${count} ОЧЕРЕДИ`
  return `${count} ОЧЕРЕДЕЙ`
})

const queueMeta = computed(() => [active.value?.course, active.value?.teacher].filter(Boolean).join(' · ') || 'Открытая очередь')
const isAdmin = computed(() => user.value?.role === 'admin')

function errorMessage(body, status) {
  try {
    const parsed = JSON.parse(body)
    if (typeof parsed === 'string') return parsed
    if (parsed.error) return parsed.error
  } catch {
    if (body) return body
  }
  return ({ 400: 'Проверьте заполнение полей', 404: 'Очередь не найдена', 409: 'Вы уже записаны' })[status] || 'Не удалось выполнить действие'
}

async function request(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...(options.headers || {}) },
  })
  if (!response.ok) throw new Error(errorMessage(await response.text(), response.status))
  return response.status === 204 ? null : response.json()
}

async function checkSession() {
  try {
    user.value = await request('/api/auth/me')
  } catch {
    user.value = null
  }
}

async function submitAuth() {
  try {
    user.value = await request(`/api/auth/${authMode.value === 'register' ? 'register' : 'login'}`, {
      method: 'POST',
      body: JSON.stringify(authForm.value),
    })
    authOpen.value = false
    authForm.value = { email: '', password: '' }
    toast(authMode.value === 'register' ? 'Аккаунт создан' : 'Вы вошли в аккаунт')
  } catch (error) {
    toast(error.message)
  }
}

async function logout() {
  try {
    await request('/api/auth/logout', { method: 'POST' })
    user.value = null
    toast('Вы вышли из аккаунта')
  } catch (error) {
    toast(error.message)
  }
}

function toast(message) {
  toastMessage.value = message
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toastMessage.value = ''), 2400)
}

function initials(name) {
  return name.trim().split(/\s+/).slice(0, 2).map((part) => part[0] || '').join('').toUpperCase()
}

function entryIsMe(entry) {
  try {
    return JSON.parse(localStorage.getItem(`queue-leave-${active.value?.id}`) || 'null')?.entryId === entry.id
  } catch {
    return false
  }
}

function openQueue(id) {
  active.value = queues.value.find((queue) => queue.id === id) || null
  if (!active.value) return
  queueOpen.value = true
  history.replaceState(null, '', `#${id}`)
}

function closeQueue() {
  queueOpen.value = false
  active.value = null
  if (location.hash) history.replaceState(null, '', location.pathname + location.search)
}

async function loadQueues() {
  try {
    queues.value = await request(api)
    loadError.value = ''
    const id = location.hash.slice(1)
    if (id) openQueue(id)
  } catch (error) {
    loadError.value = error.message
  } finally {
    loading.value = false
  }
}

async function refresh() {
  try {
    queues.value = await request(api)
    if (active.value) {
      active.value = queues.value.find((queue) => queue.id === active.value.id) || null
      if (!active.value) closeQueue()
    }
  } catch {
    // Keep the last successfully loaded state while the API is temporarily unavailable.
  }
}

async function createQueue() {
  try {
    const queue = await request(api, { method: 'POST', body: JSON.stringify(createForm.value) })
    queues.value.unshift(queue)
    createOpen.value = false
    createForm.value = { title: '', course: '', teacher: '' }
    toast('Очередь создана — поделитесь ссылкой')
    openQueue(queue.id)
  } catch (error) {
    toast(error.message)
  }
}

function startCreate() {
  if (!isAdmin.value) return
  editingQueue.value = null
  createForm.value = { title: '', course: '', teacher: '' }
  createOpen.value = true
}

function startEdit(queue = active.value) {
  if (!queue || !isAdmin.value) return
  editingQueue.value = queue
  createForm.value = { title: queue.title, course: queue.course, teacher: queue.teacher }
  createOpen.value = true
}

async function saveQueue() {
  if (!editingQueue.value) return createQueue()
  try {
    const updated = await request(`${api}/${editingQueue.value.id}`, {
      method: 'PUT',
      body: JSON.stringify({ ...createForm.value, is_open: editingQueue.value.is_open }),
    })
    queues.value = queues.value.map((queue) => queue.id === updated.id ? updated : queue)
    active.value = updated
    createOpen.value = false
    toast('Очередь обновлена')
  } catch (error) {
    toast(error.message)
  }
}

async function toggleQueue() {
  if (!isAdmin.value) return
  try {
    const updated = await request(`${api}/${active.value.id}`, {
      method: 'PUT',
      body: JSON.stringify({ title: active.value.title, course: active.value.course, teacher: active.value.teacher, is_open: !active.value.is_open }),
    })
    active.value = updated
    queues.value = queues.value.map((queue) => queue.id === updated.id ? updated : queue)
    toast(updated.is_open ? 'Запись открыта' : 'Запись закрыта')
  } catch (error) {
    toast(error.message)
  }
}

async function deleteQueue() {
  if (!active.value || !isAdmin.value || !window.confirm('Удалить очередь вместе со списком участников?')) return
  try {
    await request(`${api}/${active.value.id}`, { method: 'DELETE' })
    queues.value = queues.value.filter((queue) => queue.id !== active.value.id)
    closeQueue()
    toast('Очередь удалена')
  } catch (error) {
    toast(error.message)
  }
}

async function joinQueue() {
  if (!active.value) return
  try {
    const result = await request(`${api}/${active.value.id}/join`, {
      method: 'POST',
      body: JSON.stringify({ name: joinName.value }),
    })
    const queue = result.queue
    const entry = queue.entries[queue.entries.length - 1]
    localStorage.setItem(`queue-leave-${queue.id}`, JSON.stringify({ entryId: entry.id, token: result.leave_token }))
    active.value = queue
    joinName.value = ''
    toast(`Вы записаны под номером ${queue.entries.length}`)
  } catch (error) {
    toast(error.message)
  }
}

async function removeMe(entryId) {
  try {
    const credentials = JSON.parse(localStorage.getItem(`queue-leave-${active.value.id}`) || 'null')
    active.value = await request(`${api}/${active.value.id}/entries/${entryId}`, {
      method: 'DELETE',
      headers: { 'X-Queue-Leave-Token': credentials?.token || '' },
    })
    localStorage.removeItem(`queue-leave-${active.value.id}`)
    toast('Вы вышли из очереди')
  } catch (error) {
    toast(error.message)
  }
}

async function removeParticipant(entryId) {
  if (!isAdmin.value) return
  try {
    active.value = await request(`${api}/${active.value.id}/manage/entries/${entryId}`, { method: 'DELETE' })
    queues.value = queues.value.map((queue) => queue.id === active.value.id ? active.value : queue)
    toast('Участник удалён из очереди')
  } catch (error) {
    toast(error.message)
  }
}

async function shareQueue() {
  const url = `${location.origin}${location.pathname}#${active.value.id}`
  try {
    await navigator.clipboard.writeText(url)
    toast('Ссылка скопирована')
  } catch {
    window.prompt('Скопируйте ссылку:', url)
  }
}

function handleHashChange() {
  const id = location.hash.slice(1)
  if (id) openQueue(id)
  else closeQueue()
}

function closeOnEscape(event) {
  if (event.key === 'Escape') {
    createOpen.value = false
    closeQueue()
  }
}

onMounted(() => {
  checkSession()
  loadQueues()
  refreshTimer = setInterval(() => {
    if (!document.hidden) refresh()
  }, 5000)
  window.addEventListener('hashchange', handleHashChange)
  window.addEventListener('keydown', closeOnEscape)
})

onUnmounted(() => {
  clearInterval(refreshTimer)
  clearTimeout(toastTimer)
  window.removeEventListener('hashchange', handleHashChange)
  window.removeEventListener('keydown', closeOnEscape)
})
</script>

<template>
  <header class="topbar shell">
    <div class="brand"><span class="brand-mark">↗</span> очередь<span class="brand-tail">/даль</span></div>
    <div class="header-right"><span class="tag">ДВФУ · ПРОСТРАНСТВО ДЛЯ ПОКАЗОВ</span><div v-if="user" class="account"><span v-if="isAdmin" class="admin-badge">АДМИН</span><span>{{ user.email }}</span><button class="account-action" @click="logout">Выйти</button></div><button v-else class="account-action" @click="authMode = 'login'; authOpen = true">Войти</button></div>
  </header>

  <main class="shell">
    <section class="hero">
      <div>
        <p class="eyebrow">Меньше сообщений — больше порядка</p>
        <h1>Показы по очереди.<br /><em>Без хаоса в чате.</em></h1>
      </div>
      <p class="hero-copy">Создайте очередь, отправьте ссылку в чат — и каждый запишется сам. Всё по порядку и в одном месте.</p>
    </section>

    <div class="toolbar">
      <div><span class="section-title">Открытые очереди</span><span class="count">{{ loading ? 'ЗАГРУЗКА…' : queueCount }}</span></div>
      <button v-if="isAdmin" class="primary" @click="startCreate"><span class="plus">+</span> Создать очередь</button>
      <span v-else class="admin-note">Создавать очереди могут администраторы</span>
    </div>

    <section class="grid">
      <div v-if="loading" class="blank">Загружаем очереди…</div>
      <div v-else-if="loadError" class="blank"><div><strong>Не удалось загрузить очереди</strong>{{ loadError }}. Обновите страницу.</div></div>
      <div v-else-if="queues.length === 0" class="blank"><div><strong>Пока здесь тихо</strong>Создайте первую очередь и поделитесь ссылкой с группой.</div></div>
      <article v-for="queue in queues" :key="queue.id" class="card">
        <div class="card-top"><span class="course">{{ queue.course || 'ПОКАЗ' }}</span><span class="live" :class="{ 'is-closed': !queue.is_open }">{{ queue.is_open ? 'ОТКРЫТА' : 'ЗАПИСЬ ЗАКРЫТА' }}</span></div>
        <h2>{{ queue.title }}</h2>
        <div class="teacher">{{ queue.teacher ? `Преподаватель · ${queue.teacher}` : 'Можно записаться прямо сейчас' }}</div>
        <div class="card-bottom">
          <div class="avatars">
            <span v-for="entry in queue.entries.slice(0, 4)" :key="entry.id" class="avatar" :title="entry.name">{{ initials(entry.name) }}</span>
            <span v-if="queue.entries.length > 4" class="more">+{{ queue.entries.length - 4 }}</span>
            <span class="more">{{ queue.entries.length }} чел.</span>
          </div>
          <div class="card-actions"><button v-if="isAdmin" class="manage-link" @click="startEdit(queue)">Изменить</button><button class="join" @click="openQueue(queue.id)">Открыть очередь →</button></div>
        </div>
      </article>
    </section>

    <footer class="footer"><span>СДЕЛАНО ДЛЯ ПОКАЗОВ</span><span>ДВФУ · 2026</span></footer>
  </main>

  <div v-if="createOpen" class="modal-backdrop" @click.self="createOpen = false">
    <form class="modal" @submit.prevent="saveQueue">
      <div class="modal-head"><div><p class="eyebrow">{{ editingQueue ? 'НАСТРОЙКИ ОЧЕРЕДИ' : 'НОВАЯ ОЧЕРЕДЬ' }}</p><h2>{{ editingQueue ? 'Изменить очередь' : 'Что показываем?' }}</h2></div><button class="close" type="button" aria-label="Закрыть" @click="createOpen = false">×</button></div>
      <label class="field" for="title">Название показа</label><input id="title" v-model="createForm.title" placeholder="Например, макеты упаковки" required maxlength="80" />
      <label class="field" for="course">Предмет или курс</label><input id="course" v-model="createForm.course" placeholder="Дизайн-проектирование" maxlength="60" />
      <label class="field" for="teacher">Преподаватель</label><input id="teacher" v-model="createForm.teacher" placeholder="Иванова Анна Сергеевна" maxlength="60" />
      <button class="primary submit" type="submit">{{ editingQueue ? 'Сохранить изменения' : 'Создать очередь' }} <span>→</span></button>
    </form>
  </div>

  <div v-if="queueOpen && active" class="modal-backdrop" @click.self="closeQueue">
    <section class="modal queue-modal">
      <div class="modal-head"><div><p class="eyebrow">{{ active.is_open ? 'СПИСОК УЧАСТНИКОВ' : 'ЗАПИСЬ ЗАКРЫТА' }}</p><h2>{{ active.title }}</h2><p class="queue-meta">{{ queueMeta }}</p></div><div class="modal-actions"><button class="share" @click="shareQueue">↗ ССЫЛКА</button><button class="close" type="button" aria-label="Закрыть" @click="closeQueue">×</button></div></div>
      <div v-if="isAdmin" class="owner-tools"><button class="manage-button" @click="toggleQueue">{{ active.is_open ? 'Закрыть запись' : 'Открыть запись' }}</button><button class="manage-button" @click="startEdit()">Изменить</button><button class="manage-button danger" @click="deleteQueue">Удалить очередь</button></div>
      <form v-if="active.is_open" class="join-form" @submit.prevent="joinQueue"><input v-model="joinName" placeholder="Ваша фамилия" required maxlength="50" autocomplete="name" /><button class="primary" type="submit">Записаться <span>→</span></button></form>
      <div class="list-label">Очередь · {{ active.entries.length }}</div>
      <ol class="people">
        <li v-if="active.entries.length === 0" class="empty-list">Пока никто не записался. Будьте первыми!</li>
        <li v-for="(entry, index) in active.entries" :key="entry.id" class="person">
          <span class="number">{{ String(index + 1).padStart(2, '0') }}</span><span class="person-name">{{ entry.name }}</span>
          <span v-if="entryIsMe(entry)" class="you">ЭТО ВЫ</span><button v-if="entryIsMe(entry)" class="remove" title="Удалить себя" @click="removeMe(entry.id)">×</button><button v-else-if="isAdmin" class="remove" title="Удалить участника" @click="removeParticipant(entry.id)">×</button>
        </li>
      </ol>
    </section>
  </div>

  <div v-if="authOpen" class="modal-backdrop" @click.self="authOpen = false">
    <form class="modal" @submit.prevent="submitAuth">
      <div class="modal-head"><div><p class="eyebrow">АККАУНТ ДВФУ</p><h2>{{ authMode === 'register' ? 'Регистрация' : 'Вход' }}</h2></div><button class="close" type="button" aria-label="Закрыть" @click="authOpen = false">×</button></div>
      <label class="field" for="auth-email">Электронная почта</label><input id="auth-email" v-model="authForm.email" type="email" placeholder="you@example.com" required maxlength="254" autocomplete="email" />
      <label class="field" for="auth-password">Пароль</label><input id="auth-password" v-model="authForm.password" type="password" :placeholder="authMode === 'register' ? 'Не менее 8 символов' : 'Ваш пароль'" required minlength="8" maxlength="128" :autocomplete="authMode === 'register' ? 'new-password' : 'current-password'" />
      <button class="primary submit" type="submit">{{ authMode === 'register' ? 'Создать аккаунт' : 'Войти' }} <span>→</span></button>
      <button class="mode-switch" type="button" @click="authMode = authMode === 'register' ? 'login' : 'register'">{{ authMode === 'register' ? 'Уже есть аккаунт? Войти' : 'Нет аккаунта? Зарегистрироваться' }}</button>
    </form>
  </div>

  <div v-if="toastMessage" class="toast">{{ toastMessage }}</div>
</template>
