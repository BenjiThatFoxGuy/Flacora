'use strict';

let state = null;
let page = 'downloads';
const ui = {
  urls: '',
  enqueueError: '',
  extension: 'auto',
  openLogs: new Set(),
  filePath: '',
  filePaths: {},
  lookup: {},
  uploading: false,
  lastNotices: '',
  toastTimer: null,
};

const main = document.getElementById('main');
const tabs = document.getElementById('tabs');
const toasts = document.getElementById('toasts');

tabs.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-page]');
  if (!button) return;
  page = button.dataset.page;
  for (const tab of tabs.querySelectorAll('button')) tab.classList.toggle('active', tab === button);
  render();
});

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[character]));
}

async function api(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) {
    const text = await response.text().catch(() => '');
    throw new Error(text || `${response.status}`);
  }
  if (response.status === 204) return null;
  return response.json();
}

function post(path, body) {
  return api(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body ?? {}),
  });
}

async function refresh() {
  try {
    state = await api('/api/state');
    render();
  } catch (error) {
    toasts.innerHTML = `<div class="toast error">Cannot reach the server: ${esc(error.message)}</div>`;
  }
}

function downloadName(job) {
  if (job.title) {
    const artist = job.artist ? `${job.artist} - ` : '';
    return `${artist}${job.title}`;
  }
  return job.url;
}

function statusBadge(status) {
  return `<span class="badge ${esc(status)}">${esc(status)}</span>`;
}

function progressBar(job) {
  if (job.progress == null) return '';
  const percent = Math.round(Math.max(0, Math.min(1, job.progress)) * 100);
  return `<div class="progress"><span style="width:${percent}%"></span></div>`;
}

// ------------------------------------------------------------------ downloads

function candidatesFor(url) {
  let host = '';
  try {
    host = new URL(url).hostname.toLowerCase().replace(/\.$/, '');
  } catch {
    return [];
  }
  const matches = (hosts) => hosts.some((candidate) => host === candidate || host.endsWith(`.${candidate}`));
  const downloaders = state.installed.filter((extension) => extension.kind === 'Downloader' && matches(extension.hosts));
  const resolvers = state.installed.filter((extension) => extension.kind === 'Link Resolver' && matches(extension.hosts));
  return [...downloaders, ...resolvers];
}

function renderDownloads() {
  const urls = ui.urls.split('\n').map((line) => line.trim()).filter(Boolean);
  const candidates = urls.length === 1 ? candidatesFor(urls[0]) : [];
  if (ui.extension !== 'auto' && !candidates.some((extension) => extension.id === ui.extension)) {
    ui.extension = 'auto';
  }
  const selected = candidates.find((extension) => extension.id === ui.extension);

  const options = (selected?.options ?? []).map((option) => {
    const current = state.settings.options[`${selected.id}/${option.key}`];
    const value = current ?? option.choices[option.default]?.value ?? option.choices[0]?.value ?? '';
    return `<label>${esc(option.label)}
      <select data-option="${esc(option.key)}">
        ${option.choices.map((choice) => `<option value="${esc(choice.value)}" ${choice.value === value ? 'selected' : ''}>${esc(choice.label)}</option>`).join('')}
      </select>
    </label>`;
  }).join('');

  const jobs = state.jobs.map((job) => {
    const open = ui.openLogs.has(job.id);
    const choices = job.status === 'choosing'
      ? `<div class="choices">${job.choices.map((choice, index) => `
          <div class="choice">
            <button class="button small" data-choose="${job.id}" data-index="${index}">${esc(choice.service)}</button>
            <span class="muted">${esc(choice.downloader_name)}</span>
          </div>`).join('')}</div>`
      : '';
    const log = open
      ? `<div class="log">${job.log.length ? job.log.map(esc).join('\n') : '(no output yet)'}</div>`
      : '';
    const error = job.error ? `<div class="sub" style="color:var(--bad)">${esc(job.error)}</div>` : '';
    const saved = job.saved.length ? `<div class="sub">${job.saved.map(esc).join('<br>')}</div>` : '';
    const actions = [];
    if (job.status === 'running' || job.status === 'queued' || job.status === 'choosing') {
      actions.push(`<button class="icon-btn" data-action="cancel" data-id="${job.id}">Cancel</button>`);
    }
    if (job.status === 'failed' || job.status === 'cancelled' || job.status === 'done') {
      actions.push(`<button class="icon-btn" data-action="retry" data-id="${job.id}">Retry</button>`);
    }
    actions.push(`<button class="icon-btn" data-action="toggle-log" data-id="${job.id}">${open ? 'Hide log' : 'Log'}</button>`);
    actions.push(`<button class="icon-btn" data-action="remove" data-id="${job.id}">Remove</button>`);
    return `<div class="job">
      <div class="head">
        <span class="title">${esc(downloadName(job))}</span>
        <span class="sub">${esc(job.extension.name)}${job.service ? ` · ${esc(job.service)}` : ''}${job.quality ? ` · ${esc(job.quality)}` : ''}</span>
        <span class="status">${statusBadge(job.status)}</span>
      </div>
      ${job.stage ? `<div class="sub">${esc(job.stage)}</div>` : ''}
      ${error}${saved}${progressBar(job)}${choices}${log}
      <div class="actions">${actions.join('')}</div>
    </div>`;
  }).join('');

  main.innerHTML = `
    <div class="grid two">
      <div class="card">
        <h2>Add download</h2>
        <label>Paste one or more links</label>
        <textarea id="urls" placeholder="https://tidal.com/track/...">${esc(ui.urls)}</textarea>
        <p id="enqueue-error" role="alert" style="color:var(--bad)" ${ui.enqueueError ? '' : 'hidden'}>${esc(ui.enqueueError)}</p>
        ${candidates.length ? `<label>Downloader</label>
        <select id="extension">
          <option value="auto" ${ui.extension === 'auto' ? 'selected' : ''}>Automatic</option>
          ${candidates.map((extension) => `<option value="${esc(extension.id)}" ${ui.extension === extension.id ? 'selected' : ''}>${esc(extension.name)}</option>`).join('')}
        </select>` : ''}
        <div class="row" style="margin-top:10px">${options}</div>
        <div class="row end" style="margin-top:12px">
          <button class="button" id="enqueue">Add to queue</button>
        </div>
      </div>
      <div class="card">
        <h2>Queue</h2>
        <div class="row" style="margin-bottom:10px">
          <button class="button ghost small" id="pause">${state.queue_paused ? 'Resume' : 'Pause'}</button>
          <button class="button ghost small" id="retry-failed">Retry failed</button>
          <button class="button ghost small" id="cancel-all">Cancel all</button>
          <button class="button ghost small" id="clear-finished">Clear finished</button>
        </div>
        <div class="queue">${jobs || '<div class="empty">Nothing queued.</div>'}</div>
      </div>
    </div>`;

  document.getElementById('urls').addEventListener('input', (event) => { ui.urls = event.target.value; });
  document.getElementById('enqueue').addEventListener('click', enqueue);
  const extensionSelect = document.getElementById('extension');
  if (extensionSelect) extensionSelect.addEventListener('change', (event) => { ui.extension = event.target.value; renderDownloads(); });
  document.getElementById('pause').addEventListener('click', () => post('/api/queue/pause', { paused: !state.queue_paused }));
  document.getElementById('retry-failed').addEventListener('click', () => post('/api/jobs/retry-failed'));
  document.getElementById('cancel-all').addEventListener('click', () => post('/api/jobs/cancel-all'));
  document.getElementById('clear-finished').addEventListener('click', () => post('/api/jobs/clear-finished'));
  main.querySelectorAll('[data-action]').forEach((button) => button.addEventListener('click', () => {
    const { action, id } = button.dataset;
    if (action === 'cancel') post(`/api/jobs/${id}/cancel`);
    if (action === 'retry') post(`/api/jobs/${id}/retry`);
    if (action === 'remove') post(`/api/jobs/${id}/remove`);
    if (action === 'toggle-log') {
      if (ui.openLogs.has(Number(id))) ui.openLogs.delete(Number(id)); else ui.openLogs.add(Number(id));
      renderDownloads();
    }
  }));
  main.querySelectorAll('[data-choose]').forEach((button) => button.addEventListener('click', () => {
    post(`/api/jobs/${button.dataset.choose}/choose`, { choice: Number(button.dataset.index) });
  }));
}

async function enqueue() {
  const urls = ui.urls.split('\n').map((line) => line.trim()).filter(Boolean);
  if (!urls.length) return;
  const options = [];
  const selected = state.installed.find((extension) => extension.id === ui.extension);
  if (selected && urls.length === 1) {
    for (const select of main.querySelectorAll('[data-option]')) {
      const option = selected.options.find((candidate) => candidate.key === select.dataset.option);
      const index = option ? option.choices.findIndex((choice) => choice.value === select.value) : -1;
      if (index >= 0) options.push({ extension: selected.id, option: select.dataset.option, index });
    }
  }
  const draft = ui.urls;
  try {
    await post('/api/enqueue', { urls, options });
    if (ui.urls === draft) ui.urls = '';
    ui.enqueueError = '';
    refresh();
  } catch (error) {
    ui.enqueueError = error.message;
    const alert = document.getElementById('enqueue-error');
    if (alert) {
      alert.textContent = ui.enqueueError;
      alert.hidden = false;
    }
  }
}

// -------------------------------------------------------------------- library

function renderLibrary() {
  const lookupExtensions = state.installed.filter((extension) => extension.kind === 'Lyrics' || extension.kind === 'Metadata');
  const entries = state.history.map((entry, index) => {
    const options = lookupExtensions.map((extension) => `<option value="${esc(extension.id)}">${esc(extension.name)}</option>`).join('');
    const lookup = lookupExtensions.length
      ? `<span class="row">
          <select data-lookup="${index}">${options}</select>
          <button class="icon-btn" data-lookup-go="${index}">Run</button>
        </span>`
      : '<span class="muted">No lookup extension installed</span>';
    return `<div class="item">
      <div class="main grow">
        <div class="name">${esc(entry.title || 'Untitled')}</div>
        <div class="meta">${esc(entry.artist || '')}${entry.album ? ` · ${esc(entry.album)}` : ''} · ${esc(entry.service || '')} ${entry.quality ? `· ${esc(entry.quality)}` : ''}</div>
        <div class="meta">${entry.files.map(esc).join('<br>')}</div>
      </div>
      ${lookup}
      <button class="icon-btn" data-remove-entry="${index}">Remove</button>
    </div>`;
  }).join('');

  main.innerHTML = `
    <div class="card">
      <div class="row">
        <h2 class="grow">Library (${state.history.length})</h2>
        <button class="button danger small" id="clear-history">Clear history</button>
      </div>
      <div class="list">${entries || '<div class="empty">No downloads yet.</div>'}</div>
    </div>`;

  document.getElementById('clear-history').addEventListener('click', () => post('/api/library/clear'));
  main.querySelectorAll('[data-remove-entry]').forEach((button) => button.addEventListener('click', () => {
    post(`/api/library/${button.dataset.removeEntry}/remove`);
  }));
  main.querySelectorAll('[data-lookup-go]').forEach((button) => button.addEventListener('click', () => {
    const index = button.dataset.lookupGo;
    const select = main.querySelector(`[data-lookup="${index}"]`);
    post(`/api/library/${index}/lookup`, { extension: select.value });
  }));
}

// ----------------------------------------------------------------- extensions

function renderExtensions() {
  const extensions = state.installed.map((extension) => {
    const options = extension.options.map((option) => {
      const key = `${extension.id}/${option.key}`;
      const current = state.settings.options[key];
      const value = current ?? option.choices[option.default]?.value ?? option.choices[0]?.value ?? '';
      return `<label>${esc(option.label)}
        <select data-pick-extension="${esc(extension.id)}" data-pick-option="${esc(option.key)}">
          ${option.choices.map((choice) => `<option value="${esc(choice.value)}" ${choice.value === value ? 'selected' : ''}>${esc(choice.label)}</option>`).join('')}
        </select>
      </label>`;
    }).join('');
    return `<div class="item ext">
      <div class="main grow">
        <div class="name">${esc(extension.name)} <span class="muted">${esc(extension.version)}</span></div>
        <div class="meta">${esc(extension.kind)} · ${extension.services.length ? extension.services.map(esc).join(', ') : 'no services'} ${extension.browser ? ' · browser' : ''}</div>
        ${extension.status ? `<div class="meta">${statusBadge(extension.status)} ${esc(extension.status_detail || '')}</div>` : ''}
        <div class="opts">${options}</div>
      </div>
      <button class="icon-btn" data-check="${esc(extension.id)}">Check</button>
      <button class="icon-btn" data-remove-ext="${esc(extension.id)}">Remove</button>
    </div>`;
  }).join('');

  main.innerHTML = `
    <div class="card">
      <h2>Extensions (${state.installed.length})</h2>
      <p class="muted">Install a <code>.fext</code> package to add a downloader. Packages are listed at <a href="https://rentry.org/flacora" target="_blank" rel="noreferrer">rentry.org/flacora</a>.</p>
      <form id="upload" class="row">
        <input type="file" id="package" accept=".fext,.flacora,.zip" multiple>
        <button class="button" type="submit">${state.importing ? `Installing ${state.importing}…` : 'Install package'}</button>
      </form>
      <div class="row" style="margin-top:12px">
        <button class="button ghost small" id="check-all">Check status</button>
        <button class="button danger small" id="remove-all">Remove all</button>
      </div>
      <div class="list" style="margin-top:12px">${extensions || '<div class="empty">No extensions installed.</div>'}</div>
    </div>`;

  document.getElementById('upload').addEventListener('submit', upload);
  document.getElementById('check-all').addEventListener('click', () => post('/api/extensions/check-all'));
  document.getElementById('remove-all').addEventListener('click', () => post('/api/extensions/remove-all'));
  main.querySelectorAll('[data-check]').forEach((button) => button.addEventListener('click', () => post(`/api/extensions/${encodeURIComponent(button.dataset.check)}/check`)));
  main.querySelectorAll('[data-remove-ext]').forEach((button) => button.addEventListener('click', () => post(`/api/extensions/${encodeURIComponent(button.dataset.removeExt)}/remove`)));
  main.querySelectorAll('[data-pick-extension]').forEach((select) => select.addEventListener('change', () => {
    const option = state.installed.find((extension) => extension.id === select.dataset.pickExtension)
      ?.options.find((candidate) => candidate.key === select.dataset.pickOption);
    const index = option ? option.choices.findIndex((choice) => choice.value === select.value) : -1;
    if (index >= 0) post('/api/settings/pick', { extension: select.dataset.pickExtension, option: select.dataset.pickOption, index });
  }));
}

async function upload(event) {
  event.preventDefault();
  const input = document.getElementById('package');
  if (!input.files.length) return;
  const form = new FormData();
  for (const file of input.files) form.append('file', file);
  await api('/api/extensions/install', { method: 'POST', body: form });
  input.value = '';
  refresh();
}

// ---------------------------------------------------------------------- files

async function renderFiles() {
  const path = ui.filePath;
  let data;
  try {
    data = await api(`/api/files?path=${encodeURIComponent(path)}`);
  } catch (error) {
    main.innerHTML = `<div class="card"><div class="empty">${esc(error.message)}</div></div>`;
    return;
  }
  ui.filePaths = { ...ui.filePaths, [data.path]: data };
  const parent = data.path === data.root ? null : data.path.slice(0, data.path.lastIndexOf('/')) || data.root;
  const files = data.files.map((file) => `
    <div class="file">
      <span>${file.is_dir ? '📁' : '🎵'}</span>
      <span class="name" data-open="${esc(file.path)}">${esc(file.name)}</span>
      <span class="size">${file.is_dir ? '' : formatSize(file.size)}</span>
      ${file.is_dir ? '' : `<a class="icon-btn" href="/api/files/download?path=${encodeURIComponent(file.path)}">Download</a>`}
    </div>`).join('');
  main.innerHTML = `
    <div class="card">
      <h2>Files</h2>
      <div class="row" style="margin-bottom:10px">
        <span class="muted grow" data-path>${esc(data.path)}</span>
        ${parent ? '<button class="icon-btn" id="up">Up</button>' : ''}
        <button class="icon-btn" id="refresh-files">Refresh</button>
      </div>
      <div class="list">${files || '<div class="empty">Empty folder.</div>'}</div>
    </div>`;
  const up = document.getElementById('up');
  if (up) up.addEventListener('click', () => { ui.filePath = parent; renderFiles(); });
  document.getElementById('refresh-files').addEventListener('click', renderFiles);
  main.querySelectorAll('[data-open]').forEach((row) => row.addEventListener('click', () => {
    const target = data.files.find((file) => file.path === row.dataset.open);
    if (target?.is_dir) { ui.filePath = target.path; renderFiles(); }
  }));
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unit]}`;
}

// ------------------------------------------------------------------- settings

function renderSettings() {
  const settings = state.settings;
  const filenameOptions = [
    ['extension-default', 'Keep Original'], ['artist-title', 'Artist - Title'],
    ['title-artist', 'Title - Artist'], ['title', 'Title Only'],
  ].map(([value, label]) => `<option value="${value}" ${settings.filename === value ? 'selected' : ''}>${label}</option>`).join('');
  const folderOptions = [
    ['single', 'Single Folder'], ['artist', 'By Artist'], ['artist-album', 'By Artist / Album'],
  ].map(([value, label]) => `<option value="${value}" ${settings.folders === value ? 'selected' : ''}>${label}</option>`).join('');

  const preferred = state.services.map((service) => {
    const downloaders = state.installed.filter((extension) => extension.kind === 'Downloader');
    const current = settings.preferred[service.key];
    return `<label>${esc(service.name)}
      <select data-preferred="${esc(service.key)}">
        <option value="">Automatic</option>
        ${downloaders.map((extension) => `<option value="${esc(extension.id)}" ${current === extension.id ? 'selected' : ''}>${esc(extension.name)}</option>`).join('')}
      </select>
    </label>`;
  }).join('');

  main.innerHTML = `
    <div class="grid two">
      <div class="card">
        <h2>Output</h2>
        <label>Download folder</label>
        <div class="row">
          <input type="text" id="output" value="${esc(settings.output)}">
          <button class="button" id="save-output">Save</button>
        </div>
        <label>File naming</label>
        <select id="filename">${filenameOptions}</select>
        <label>Folders</label>
        <select id="folders">${folderOptions}</select>
        <label class="row" style="margin-top:12px">
          <input type="checkbox" id="auto-tag" ${settings.auto_tag ? 'checked' : ''} style="width:auto">
          <span>Automatically tag downloaded FLAC files</span>
        </label>
      </div>
      <div class="card">
        <h2>Preferred downloaders</h2>
        ${preferred || '<div class="empty">Install a downloader first.</div>'}
        <h3>Tools</h3>
        <div class="muted">ffmpeg: ${esc(state.tools.ffmpeg || 'not found')}</div>
        <div class="muted">CloakBrowser: ${esc(state.tools.browser || 'not installed (downloads on first use)')}</div>
      </div>
    </div>`;

  document.getElementById('save-output').addEventListener('click', () => {
    post('/api/settings/output', { output: document.getElementById('output').value });
  });
  document.getElementById('filename').addEventListener('change', (event) => saveLayout(event.target.value, settings.folders));
  document.getElementById('folders').addEventListener('change', (event) => saveLayout(settings.filename, event.target.value));
  document.getElementById('auto-tag').addEventListener('change', (event) => post('/api/settings/auto-tag', { value: event.target.checked }));
  main.querySelectorAll('[data-preferred]').forEach((select) => select.addEventListener('change', () => {
    post('/api/settings/preferred', { service: select.dataset.preferred, extension: select.value || null });
  }));
}

function saveLayout(filename, folders) {
  post('/api/settings/layout', { filename, folders });
}

// ---------------------------------------------------------------------- render

function render() {
  if (!state) return;
  const tools = document.getElementById('tools');
  const missing = [];
  if (!state.tools.ffmpeg) missing.push('ffmpeg');
  if (!state.tools.browser) missing.push('browser');
  tools.innerHTML = `<span class="pill ${state.installed.length ? '' : 'missing'}">${state.installed.length} extension${state.installed.length === 1 ? '' : 's'}</span>`
    + (missing.length ? `<span class="pill missing">missing ${missing.join(', ')}</span>` : '');
  const key = state.notices.map((notice) => `${notice.kind}:${notice.message}`).join('|');
  if (key !== ui.lastNotices) {
    ui.lastNotices = key;
    toasts.innerHTML = state.notices.map((notice) => `<div class="toast ${esc(notice.kind)}">${esc(notice.message)}</div>`).join('');
    clearTimeout(ui.toastTimer);
    if (key) ui.toastTimer = setTimeout(() => { toasts.innerHTML = ''; }, 7000);
  }

  const editing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement?.tagName);
  if (editing) return;

  if (page === 'downloads') renderDownloads();
  else if (page === 'library') renderLibrary();
  else if (page === 'extensions') renderExtensions();
  else if (page === 'files') renderFiles();
  else if (page === 'settings') renderSettings();
}

refresh();
setInterval(refresh, 500);
api('/api/version')
  .then((info) => { document.getElementById('version').textContent = `v${info.flacora}`; })
  .catch(() => {});
