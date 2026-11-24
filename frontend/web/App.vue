<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'

const api = '/api/queues'
const queues = ref([])
const myEntryIds = ref(new Set())
const showMyQueues = ref(false)
const user = ref(null)
const active = ref(null)
const loading = ref(true)
const loadError = ref('')
const createOpen = ref(false)
const queueOpen = ref(false)
const authOpen = ref(false)
const authMode = ref('register')
const darkTheme = ref(false)
const authForm = ref({ username: '', password: '' })
const editingQueue = ref(null)
const createForm = ref({ title: '', course: '', teacher: '' })
const subqueueTitles = ref([])
const subqueueDraft = ref('')
const joinNames = ref({})
const newSubqueueName = ref('')
const pendingSubqueueTitles = ref([])
const toastMessage = ref('')
let toastTimer
let refreshTimer

const visibleQueues = computed(() => {
  if (!showMyQueues.value) return queues.value
  return queues.value.filter((queue) => queue.subqueues.some((subqueue) =>
    subqueue.entries.some((entry) => myEntryIds.value.has(entry.id)),
  ))
})

const myQueueCount = computed(() => queues.value.filter((queue) => queue.subqueues.some((subqueue) =>
  subqueue.entries.some((entry) => myEntryIds.value.has(entry.id)),
)).length)

const queueCount = computed(() => {
  const count = visibleQueues.value.length
  if (count === 1) return '1 ОЧЕРЕДЬ'
  if (count > 1 && count < 5) return `${count} ОЧЕРЕДИ`
  return `${count} ОЧЕРЕДЕЙ`
})

const queueMeta = computed(() => [active.value?.course, active.value?.teacher].filter(Boolean).join(' · ') || 'Открытая очередь')
const isAdmin = computed(() => user.value?.role === 'admin')
const entryCount = (queue) => queue.subqueues.reduce((total, subqueue) => total + subqueue.entries.length, 0)
const isFullyClosed = (queue) => !queue.is_open || !queue.subqueues.some((subqueue) => subqueue.is_open)

function sortQueues(items) {
  return [...items].sort((left, right) =>
    Number(isFullyClosed(left)) - Number(isFullyClosed(right)) ||
      (right.activity_at ?? right.created_at) - (left.activity_at ?? left.created_at),
  )
}

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
    authForm.value = { username: '', password: '' }
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

function setDarkTheme(enabled) {
  darkTheme.value = enabled
  document.documentElement.dataset.theme = enabled ? 'dark' : 'light'
  try {
    localStorage.setItem('queue-theme', enabled ? 'dark' : 'light')
  } catch {
    // The theme still applies for this session when storage is unavailable.
  }
}

function toggleTheme() {
  setDarkTheme(!darkTheme.value)
}

function entryIsMe(entry) {
  return leaveCredentials(active.value?.id).some((credential) => credential.entryId === entry.id)
}

function leaveCredentials(queueId) {
  try {
    const value = JSON.parse(localStorage.getItem(`queue-leave-${queueId}`) || 'null')
    if (Array.isArray(value)) return value
    return value?.entryId ? [value] : []
  } catch {
    return []
  }
}

function syncMyEntries() {
  const entryIds = new Set()
  try {
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index)
      if (!key?.startsWith('queue-leave-')) continue
      const queueId = key.slice('queue-leave-'.length)
      for (const credential of leaveCredentials(queueId)) {
        if (credential.entryId) entryIds.add(credential.entryId)
      }
    }
  } catch {
    // The personal queue filter is unavailable when browser storage is blocked.
  }
  myEntryIds.value = entryIds
}

function saveLeaveCredentials(queueId, credentials) {
  try {
    localStorage.setItem(`queue-leave-${queueId}`, JSON.stringify(credentials))
  } catch {
    toast('Не удалось сохранить данные для выхода из очереди в этом браузере')
  }
}

function openQueue(id) {
  active.value = queues.value.find((queue) => queue.id === id) || null
  if (!active.value) return
  newSubqueueName.value = ''
  pendingSubqueueTitles.value = []
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
    queues.value = sortQueues(await request(api))
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
    queues.value = sortQueues(await request(api))
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
    const queue = await request(api, {
      method: 'POST',
      body: JSON.stringify({
        title: createForm.value.title,
        course: createForm.value.course,
        teacher: createForm.value.teacher,
        subqueues: subqueueTitles.value,
      }),
    })
    queues.value = sortQueues([queue, ...queues.value])
    createOpen.value = false
    createForm.value = { title: '', course: '', teacher: '' }
    subqueueTitles.value = []
    subqueueDraft.value = ''
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
  subqueueTitles.value = []
  subqueueDraft.value = ''
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
    queues.value = sortQueues(queues.value.map((queue) => queue.id === updated.id ? updated : queue))
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
    queues.value = sortQueues(queues.value.map((queue) => queue.id === updated.id ? updated : queue))
    toast(updated.is_open ? 'Запись открыта' : 'Запись закрыта')
  } catch (error) {
    toast(error.message)
  }
}

async function toggleSubqueue(subqueue) {
  if (!isAdmin.value) return
  try {
    const updated = await request(`${api}/${active.value.id}/subqueues/${subqueue.id}/open`, {
      method: 'PUT',
      body: JSON.stringify({ is_open: !subqueue.is_open }),
    })
    active.value = updated
    queues.value = sortQueues(queues.value.map((queue) => queue.id === updated.id ? updated : queue))
  } catch (error) {
    toast(error.message)
  }
}

async function addSubqueue() {
  stageSubqueueDraft(newSubqueueName, pendingSubqueueTitles)
  const titles = [...pendingSubqueueTitles.value]
  if (!active.value || !isAdmin.value || !titles.length) return
  try {
    let updated = active.value
    for (const title of titles) updated = await request(`${api}/${active.value.id}/subqueues`, {
      method: 'POST',
      body: JSON.stringify({ title }),
    })
    active.value = updated
    queues.value = sortQueues(queues.value.map((queue) => queue.id === updated.id ? updated : queue))
    pendingSubqueueTitles.value = []
    toast(titles.length === 1 ? 'Подочередь добавлена' : `Добавлено подочередей: ${titles.length}`)
  } catch (error) {
    toast(error.message)
  }
}

function stageSubqueueDraft(draft, tags) {
  const title = draft.value.trim()
  if (title && !tags.value.some((tag) => tag.toLocaleLowerCase() === title.toLocaleLowerCase())) tags.value.push(title)
  draft.value = ''
}

function handleSubqueueKeydown(event, draft, tags) {
  if (event.key === ' ') {
    event.preventDefault()
    stageSubqueueDraft(draft, tags)
  } else if (event.key === 'Enter' || event.key === ',') {
    event.preventDefault()
    stageSubqueueDraft(draft, tags)
  } else if (event.key === 'Backspace' && !draft.value && tags.value.length) {
    tags.value.pop()
  }
}

function handleSubqueuePaste(event, draft, tags) {
  const pasted = event.clipboardData?.getData('text') || ''
  if (!/[\n,;]/.test(pasted)) return
  event.preventDefault()
  const values = [...draft.value, pasted].join('').split(/[\n,;]+/).map((value) => value.trim()).filter(Boolean)
  for (const value of values) {
    if (!tags.value.some((tag) => tag.toLocaleLowerCase() === value.toLocaleLowerCase())) tags.value.push(value)
  }
  draft.value = ''
}

function createSubqueueKeydown(event) {
  handleSubqueueKeydown(event, subqueueDraft, subqueueTitles)
}

function createSubqueueBlur() {
  stageSubqueueDraft(subqueueDraft, subqueueTitles)
}

function createSubqueuePaste(event) {
  handleSubqueuePaste(event, subqueueDraft, subqueueTitles)
}

function pendingSubqueueKeydown(event) {
  handleSubqueueKeydown(event, newSubqueueName, pendingSubqueueTitles)
}

function pendingSubqueueBlur() {
  stageSubqueueDraft(newSubqueueName, pendingSubqueueTitles)
}

function pendingSubqueuePaste(event) {
  handleSubqueuePaste(event, newSubqueueName, pendingSubqueueTitles)
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

async function joinQueue(subqueueId) {
  if (!active.value) return
  try {
    const result = await request(`${api}/${active.value.id}/subqueues/${subqueueId}/join`, {
      method: 'POST',
      body: JSON.stringify({ name: joinNames.value[subqueueId] || '' }),
    })
    const queue = result.queue
    const subqueue = queue.subqueues.find((item) => item.id === subqueueId)
    const entry = subqueue.entries[subqueue.entries.length - 1]
    const credentials = leaveCredentials(queue.id).filter((item) => item.entryId !== entry.id)
    credentials.push({ entryId: entry.id, token: result.leave_token })
    saveLeaveCredentials(queue.id, credentials)
    syncMyEntries()
    active.value = queue
    joinNames.value[subqueueId] = ''
    toast(`Вы записаны в «${subqueue.title}» под номером ${subqueue.entries.length}`)
  } catch (error) {
    toast(error.message)
  }
}

async function removeMe(entryId) {
  try {
    const credentials = leaveCredentials(active.value.id)
    const credential = credentials.find((item) => item.entryId === entryId)
    active.value = await request(`${api}/${active.value.id}/entries/${entryId}`, {
      method: 'DELETE',
      headers: { 'X-Queue-Leave-Token': credential?.token || '' },
    })
    saveLeaveCredentials(active.value.id, credentials.filter((item) => item.entryId !== entryId))
    syncMyEntries()
    toast('Вы вышли из очереди')
  } catch (error) {
    toast(error.message)
  }
}

async function removeParticipant(entryId) {
  if (!isAdmin.value) return
  try {
    active.value = await request(`${api}/${active.value.id}/manage/entries/${entryId}`, { method: 'DELETE' })
    queues.value = sortQueues(queues.value.map((queue) => queue.id === active.value.id ? active.value : queue))
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
  syncMyEntries()
  let savedTheme
  try {
    savedTheme = localStorage.getItem('queue-theme')
  } catch {
    savedTheme = null
  }
  setDarkTheme(savedTheme ? savedTheme === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches)
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
    <div class="header-right"><span class="tag">ДВФУ · ПРОСТРАНСТВО ДЛЯ ПОКАЗОВ</span><button class="theme-toggle" type="button" :aria-label="darkTheme ? 'Включить светлую тему' : 'Включить тёмную тему'" :title="darkTheme ? 'Светлая тема' : 'Тёмная тема'" @click="toggleTheme">{{ darkTheme ? '☀' : '☾' }}</button><div v-if="user" class="account"><span v-if="isAdmin" class="admin-badge">АДМИН</span><span>{{ user.username }}</span><button class="account-action" @click="logout">Выйти</button></div><button v-else class="account-action" @click="authMode = 'login'; authOpen = true">Войти</button></div>
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
      <div class="toolbar-title"><span class="section-title">{{ showMyQueues ? 'Мои очереди' : 'Открытые очереди' }}</span><span class="count">{{ loading ? 'ЗАГРУЗКА…' : queueCount }}</span><button class="filter-button" :class="{ selected: showMyQueues }" type="button" title="Личные записи, сохранённые в этом браузере" @click="showMyQueues = !showMyQueues">{{ showMyQueues ? 'Все очереди' : `Мои очереди · ${myQueueCount}` }}</button></div>
      <button v-if="isAdmin" class="primary" @click="startCreate"><span class="plus">+</span> Создать очередь</button>
      <span v-else class="admin-note">Создавать очереди могут администраторы</span>
    </div>

    <section class="grid">
      <div v-if="loading" class="blank">Загружаем очереди…</div>
      <div v-else-if="loadError" class="blank"><div><strong>Не удалось загрузить очереди</strong>{{ loadError }}. Обновите страницу.</div></div>
      <div v-else-if="showMyQueues && visibleQueues.length === 0" class="blank"><div><strong>Вы пока никуда не записаны</strong>Запишитесь в подочередь — она появится здесь.</div></div>
      <div v-else-if="!showMyQueues && queues.length === 0" class="blank"><div><strong>Пока здесь тихо</strong>Создайте первую очередь и поделитесь ссылкой с группой.</div></div>
      <article v-for="queue in visibleQueues" :key="queue.id" class="card">
        <div class="card-top"><span class="course">{{ queue.course || 'ПОКАЗ' }}</span><span class="live" :class="{ 'is-closed': isFullyClosed(queue) }">{{ isFullyClosed(queue) ? 'ЗАПИСЬ ЗАКРЫТА' : 'ОТКРЫТА' }}</span></div>
        <h2>{{ queue.title }}</h2>
        <div class="teacher">{{ queue.teacher ? `Преподаватель · ${queue.teacher}` : 'Можно записаться прямо сейчас' }}</div>
        <div class="card-subqueues"><span v-for="subqueue in queue.subqueues" :key="subqueue.id" class="subqueue-chip">{{ subqueue.title }} <b>{{ subqueue.entries.length }}</b></span></div>
        <div class="card-bottom">
          <span class="more">Всего участников: {{ entryCount(queue) }}</span>
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
      <template v-if="!editingQueue"><label class="field" for="subqueues">Подочереди</label><div class="subqueue-tagbox"><span v-for="(title, index) in subqueueTitles" :key="`${title}-${index}`" class="subqueue-tag">{{ title }}<button type="button" :aria-label="`Удалить ${title}`" @click="subqueueTitles.splice(index, 1)">×</button></span><input id="subqueues" v-model="subqueueDraft" placeholder="Название и пробел…" maxlength="120" @keydown="createSubqueueKeydown" @blur="createSubqueueBlur" @paste="createSubqueuePaste" /></div><p class="subqueue-help">Каждое название станет тегом по пробелу или когда вы перейдёте дальше.</p></template>
      <button class="primary submit" type="submit">{{ editingQueue ? 'Сохранить изменения' : 'Создать очередь' }} <span>→</span></button>
    </form>
  </div>

  <div v-if="queueOpen && active" class="modal-backdrop" @click.self="closeQueue">
    <section class="modal queue-modal">
      <div class="modal-head"><div><p class="eyebrow">{{ active.is_open ? 'СПИСОК УЧАСТНИКОВ' : 'ЗАПИСЬ ЗАКРЫТА' }}</p><h2>{{ active.title }}</h2><p class="queue-meta">{{ queueMeta }}</p></div><div class="modal-actions"><button class="share" @click="shareQueue">↗ ССЫЛКА</button><button class="close" type="button" aria-label="Закрыть" @click="closeQueue">×</button></div></div>
      <div v-if="isAdmin" class="owner-tools"><button class="manage-button" @click="toggleQueue">{{ active.is_open ? 'Закрыть все очереди' : 'Открыть все очереди' }}</button><button class="manage-button" @click="startEdit()">Изменить</button><button class="manage-button danger" @click="deleteQueue">Удалить очередь</button></div>
      <form v-if="isAdmin" class="add-subqueue" @submit.prevent="addSubqueue"><div class="subqueue-tagbox"><span v-for="(title, index) in pendingSubqueueTitles" :key="`${title}-${index}`" class="subqueue-tag">{{ title }}<button type="button" :aria-label="`Убрать ${title}`" @click="pendingSubqueueTitles.splice(index, 1)">×</button></span><input v-model="newSubqueueName" placeholder="Название этапа и пробел…" maxlength="120" @keydown="pendingSubqueueKeydown" @blur="pendingSubqueueBlur" @paste="pendingSubqueuePaste" /></div><button class="manage-button" type="submit">Добавить{{ pendingSubqueueTitles.length > 1 ? ` (${pendingSubqueueTitles.length})` : '' }}</button></form>
      <div class="subqueue-list">
        <section v-for="subqueue in active.subqueues" :key="subqueue.id" class="subqueue-section">
          <div class="subqueue-heading"><div><h3>{{ subqueue.title }}</h3><span class="subqueue-count">{{ subqueue.entries.length }} участников</span></div><button v-if="isAdmin" class="manage-button" @click="toggleSubqueue(subqueue)">{{ subqueue.is_open ? 'Закрыть запись' : 'Открыть запись' }}</button><span v-else class="subqueue-status" :class="{ closed: !subqueue.is_open || !active.is_open }">{{ active.is_open && subqueue.is_open ? 'ОТКРЫТА' : 'ЗАКРЫТА' }}</span></div>
          <form v-if="active.is_open && subqueue.is_open" class="join-form" @submit.prevent="joinQueue(subqueue.id)"><input v-model="joinNames[subqueue.id]" placeholder="Ваша фамилия" required maxlength="50" autocomplete="name" /><button class="primary" type="submit">Записаться <span>→</span></button></form>
          <ol class="people">
            <li v-if="subqueue.entries.length === 0" class="empty-list">В этой подочереди пока никого нет.</li>
            <li v-for="(entry, index) in subqueue.entries" :key="entry.id" class="person">
              <span class="number">{{ String(index + 1).padStart(2, '0') }}</span><span class="person-name">{{ entry.name }}</span>
              <span v-if="entryIsMe(entry)" class="you">ЭТО ВЫ</span><button v-if="entryIsMe(entry)" class="remove" title="Удалить себя" @click="removeMe(entry.id)">×</button><button v-else-if="isAdmin" class="remove" title="Удалить участника" @click="removeParticipant(entry.id)">×</button>
            </li>
          </ol>
        </section>
      </div>
    </section>
  </div>

  <div v-if="authOpen" class="modal-backdrop" @click.self="authOpen = false">
    <form class="modal" @submit.prevent="submitAuth">
      <div class="modal-head"><div><p class="eyebrow">АККАУНТ ДВФУ</p><h2>{{ authMode === 'register' ? 'Регистрация' : 'Вход' }}</h2></div><button class="close" type="button" aria-label="Закрыть" @click="authOpen = false">×</button></div>
      <label class="field" for="auth-username">Имя пользователя</label><input id="auth-username" v-model="authForm.username" type="text" placeholder="например, ivan_23" required minlength="3" maxlength="32" pattern="[A-Za-z0-9_]+" autocomplete="username" />
      <label class="field" for="auth-password">Пароль</label><input id="auth-password" v-model="authForm.password" type="password" :placeholder="authMode === 'register' ? 'Не менее 8 символов' : 'Ваш пароль'" required minlength="8" maxlength="128" :autocomplete="authMode === 'register' ? 'new-password' : 'current-password'" />
      <button class="primary submit" type="submit">{{ authMode === 'register' ? 'Создать аккаунт' : 'Войти' }} <span>→</span></button>
      <button class="mode-switch" type="button" @click="authMode = authMode === 'register' ? 'login' : 'register'">{{ authMode === 'register' ? 'Уже есть аккаунт? Войти' : 'Нет аккаунта? Зарегистрироваться' }}</button>
    </form>
  </div>

  <div v-if="toastMessage" class="toast">{{ toastMessage }}</div>
</template>
