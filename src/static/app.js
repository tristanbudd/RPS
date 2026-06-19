// Mapping of file extensions to language names for highlighting
const EXT = {
  abnf:'abnf', adoc:'asciidoc', asciidoc:'asciidoc',
  ahk:'autohotkey', ahkl:'autohotkey', au3:'autoit', awk:'awk', gawk:'awk',
  as:'actionscript', adb:'ada', ads:'ada', ada:'ada',
  apacheconf:'apache', vhost:'apache', applescript:'applescript', scpt:'applescript',
  sh:'bash', bash:'bash', zsh:'bash', ksh:'bash',
  bas:'basic', bb:'basic', bf:'brainfuck', bnf:'bnf', bsl:'1c', os:'1c',
  c:'c', h:'c', cs:'csharp', csx:'csharp',
  cpp:'cpp', cc:'cpp', cxx:'cpp', hpp:'cpp', hh:'cpp', hxx:'cpp',
  capnp:'capnproto', chpl:'chapel',
  clj:'clojure', cljs:'clojure', cljc:'clojure', edn:'clojure',
  cmake:'cmake', coffee:'coffeescript', cson:'coffeescript', cr:'crystal', css:'css',
  d:'d', dart:'dart', dpr:'delphi', dfm:'delphi', pas:'delphi', int:'delphi',
  diff:'diff', patch:'diff', jinja:'django', jinja2:'django', j2:'django',
  dockerfile:'dockerfile', bat:'dos', cmd:'dos', dts:'dts', dtsi:'dts',
  ebnf:'ebnf', ex:'elixir', exs:'elixir', elm:'elm',
  erl:'erlang', hrl:'erlang', escript:'erlang',
  f:'fortran', f90:'fortran', f95:'fortran', f03:'fortran', for:'fortran',
  fs:'fsharp', fsx:'fsharp', fsi:'fsharp', flix:'flix',
  glsl:'glsl', vert:'glsl', frag:'glsl', gml:'gml', go:'go',
  gradle:'gradle', gql:'graphql', graphql:'graphql',
  groovy:'groovy', gvy:'groovy', gy:'groovy', gsh:'groovy',
  hbs:'handlebars', handlebars:'handlebars',
  hs:'haskell', lhs:'haskell', hx:'haxe', hxml:'haxe',
  html:'html', htm:'html', xhtml:'html', http:'http', hy:'hy',
  ini:'ini', cfg:'ini', prefs:'ini', conf:'ini', properties:'ini',
  java:'java', js:'javascript', mjs:'javascript', cjs:'javascript', jsx:'javascript',
  json:'json', jsonc:'json', jl:'julia',
  kt:'kotlin', kts:'kotlin',
  tex:'latex', ltx:'latex', lean:'lean', less:'less', ldif:'ldif',
  lisp:'lisp', lsp:'lisp', ls:'livescript', ll:'llvm', lua:'lua',
  mk:'makefile', mak:'makefile', make:'makefile',
  md:'markdown', markdown:'markdown', mkd:'markdown', mkdown:'markdown', mdx:'markdown',
  mel:'mel', moo:'mercury', mips:'mipsasm', mm:'objectivec',
  ml:'ocaml', mli:'ocaml', mll:'ocaml', mly:'ocaml',
  nginx:'nginx', nginxconf:'nginx', nim:'nim', nims:'nim',
  nix:'nix', nsi:'nsis', nsh:'nsis',
  pl:'perl', pm:'perl', perl:'perl', pde:'processing',
  php:'php', phtml:'php', php3:'php', php4:'php', php5:'php', php7:'php',
  txt:'plaintext', text:'plaintext', log:'plaintext',
  pony:'pony', ps1:'powershell', psm1:'powershell', psd1:'powershell',
  pro:'prolog', prolog:'prolog', proto:'protobuf', pp:'puppet',
  py:'python', pyw:'python', gyp:'python', gypi:'python',
  q:'q', k:'q', qml:'qml', r:'r', re:'reasonml', rei:'reasonml',
  rb:'ruby', gemspec:'ruby', podspec:'ruby', rake:'ruby', erb:'ruby', ru:'ruby',
  rs:'rust', sas:'sas', scala:'scala', sc:'scala',
  scss:'scss', sass:'scss', scm:'scheme', sls:'scheme', ss:'scheme', sch:'scheme',
  sci:'scilab', sce:'scilab', scad:'openscad', smali:'smali', st:'smalltalk',
  sml:'sml', sig:'sml', sol:'solidity', sql:'sql', stan:'stan',
  do:'stata', ado:'stata', styl:'stylus', swift:'swift',
  tcl:'tcl', tk:'tcl', thrift:'thrift', toml:'toml', twig:'twig',
  ts:'typescript', tsx:'typescript', mts:'typescript', cts:'typescript',
  vala:'vala', vapi:'vala', vb:'vbnet', vbs:'vbscript',
  v:'verilog', sv:'verilog', svh:'verilog', vhd:'vhdl', vhdl:'vhdl', vim:'vim',
  wasm:'wasm', wat:'wasm', wren:'wren',
  asm:'x86asm', nasm:'x86asm', s:'x86asm',
  xml:'xml', rss:'xml', atom:'xml', xsd:'xml', xsl:'xml', xslt:'xml',
  plist:'xml', wsf:'xml', svg:'xml',
  xq:'xquery', xqm:'xquery', xqs:'xquery', xquery:'xquery',
  yaml:'yaml', yml:'yaml', zep:'zephir',
};

// DOM Elements & States
const editor = document.getElementById('editor');
const viewer = document.getElementById('viewer');
const viewerCode = document.getElementById('viewer-code');
const gutter = document.getElementById('gutter');
const toast = document.getElementById('toast');
const btnSave = document.getElementById('btn-save');
const btnNew = document.getElementById('btn-new');
const btnDuplicate = document.getElementById('btn-duplicate');
const btnRaw = document.getElementById('btn-raw');

let state = { mode: 'edit', pasteId: null, lang: null, content: '' };
let toastTimer = null;
let rafPending = false;
let currentLine = 1;

/**
 * Displays a toast message for a specified duration.
 * @param {string} msg - The message to display in the toast.
 * @param {number} [dur=2200] - The duration in milliseconds to display the toast (default: 2200).
 */
function showToast(msg, dur = 2200) {
  if (toastTimer) clearTimeout(toastTimer);
  toast.textContent = msg;
  toast.classList.add('show');
  if (dur > 0) toastTimer = setTimeout(() => toast.classList.remove('show'), dur);
}

/**
 * Dismisses the toast message.
 */
function dismissToast() {
  if (toastTimer) clearTimeout(toastTimer);
  toast.classList.remove('show');
}

/**
 * Ensures that the highlight.js library is loaded and ready for use.
 * @returns {Promise<void>}
 */
let hljsReady = null;

function ensureHljs() {
  if (typeof hljs !== 'undefined') return Promise.resolve();
  if (hljsReady) return hljsReady;
  hljsReady = new Promise((resolve) => {
    if (!document.getElementById('hljs-theme')) {
      const l = document.createElement('link');
      l.id = 'hljs-theme';
      l.rel = 'stylesheet';
      l.href = 'https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.10.0/styles/github.min.css';
      document.head.appendChild(l);
    }
    const s = document.createElement('script');
    s.src = 'https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.10.0/highlight.min.js';
    s.onload = resolve;
    s.onerror = resolve;
    document.head.appendChild(s);
  });
  return hljsReady;
}

/**
 * Asynchronously loads external typography (Inter and Cascadia Code fonts).
 * Designed to be called after initial load to avoid blocking render.
 */
function loadFonts() {
  const fonts = [
    'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap',
    'https://cdn.jsdelivr.net/npm/@fontsource/cascadia-code/index.min.css'
  ];

  fonts.forEach(href => {
    if (!document.querySelector(`link[href="${href}"]`)) {
      const l = document.createElement('link');
      l.rel = 'stylesheet';
      l.href = href;
      document.head.appendChild(l);
    }
  });
}

/**
 * Updates the line gutter based on the number of lines in the content.
 * @param {number} lines - The number of lines in the content.
 * @param {number} active - The active line number to highlight.
 */
function updateGutter(lines, active) {
  const existing = gutter.children.length;
  if (lines > existing) {
    const frag = document.createDocumentFragment();
    for (let i = existing + 1; i <= lines; i++) {
      const s = document.createElement('span');
      s.textContent = i;
      frag.appendChild(s);
    }
    gutter.appendChild(frag);
  } else {
    while (gutter.children.length > lines) gutter.removeChild(gutter.lastChild);
  }
  const prev = gutter.querySelector('.current');
  if (prev) prev.classList.remove('current');
  const cur = gutter.children[active - 1];
  if (cur) cur.classList.add('current');
  gutter.scrollTop = viewer.classList.contains('hidden') ? editor.scrollTop : viewer.scrollTop;
}

/**
 * Schedules an update to the line gutter.
 */
function scheduleGutterUpdate() {
  if (rafPending) return;
  rafPending = true;
  requestAnimationFrame(() => {
    rafPending = false;
    const lines = editor.value.split('\n').length;
    const pos = editor.selectionStart;
    const before = editor.value.slice(0, pos);
    currentLine = before.split('\n').length;
    updateGutter(lines, currentLine);
  });
}

/**
 * Schedules an update to the line gutter when the editor content changes.
 */
editor.addEventListener('input', () => {
  scheduleGutterUpdate();
});

/**
 * Requests that a callback be run at the next idle period.
 * @param {IdleRequestCallback} callback The callback function to execute at the next idle period.
 */
const idleCall = window.requestIdleCallback || ((cb) => setTimeout(cb, 1));
idleCall(() => {
  loadFonts();

  let typingTimer = null;
  editor.addEventListener('input', () => {
    document.body.classList.add('typing');
    clearTimeout(typingTimer);
    typingTimer = setTimeout(() => document.body.classList.remove('typing'), 800);
  });
});

editor.addEventListener('keyup', scheduleGutterUpdate);
editor.addEventListener('click', scheduleGutterUpdate);

editor.addEventListener('scroll', () => {
  gutter.scrollTop = editor.scrollTop;
});

editor.addEventListener('keydown', (e) => {
  if (e.key === 'Tab') {
    e.preventDefault();
    const s = editor.selectionStart;
    const end = editor.selectionEnd;
    editor.value = editor.value.slice(0, s) + '\t' + editor.value.slice(end);
    editor.selectionStart = editor.selectionEnd = s + 1;
    scheduleGutterUpdate();
  }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 's') { e.preventDefault(); save(); }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'n') { e.preventDefault(); newPaste(); }
});

document.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'd' && state.mode === 'view') { e.preventDefault(); duplicate(); }
});

viewer.addEventListener('scroll', () => { gutter.scrollTop = viewer.scrollTop; });

/**
 * Switches the application to edit mode.
 */
function setEditMode() {
  state.mode = 'edit';
  document.title = 'RPS - New Paste';
  editor.classList.remove('hidden');
  viewer.classList.add('hidden');
  btnSave.classList.remove('hidden');
  btnDuplicate.classList.add('hidden');
  btnRaw.classList.add('hidden');
  scheduleGutterUpdate();
  editor.focus();
}

/**
 * Switches the application to view mode.
 * @param {string} content - The content of the paste to display.
 * @param {string} lang - The language of the paste to display.
 */
async function setViewMode(content, lang) {
  state.mode = 'view';
  state.content = content;
  state.lang = lang;
  document.title = 'RPS - Viewing Paste';

  viewerCode.textContent = content;
  viewerCode.className = '';

  const lines = content.split('\n').length;
  updateGutter(lines, 1);

  editor.classList.add('hidden');
  viewer.classList.remove('hidden');
  btnSave.classList.add('hidden');
  btnDuplicate.classList.remove('hidden');
  btnRaw.classList.remove('hidden');
  btnRaw.href = state.pasteId ? '/raw/' + state.pasteId : '#';
  viewer.focus();

  if (lang && lang !== 'plaintext') {
    showToast('Loading syntax highlighting...', 0);
    await ensureHljs();
    dismissToast();
    if (typeof hljs !== 'undefined') {
      viewerCode.className = 'language-' + lang;
      hljs.highlightElement(viewerCode);
    }
  } else if (state.pasteId !== 'demo' && !window.location.pathname.includes('.')) {
    showToast('Tip: Add a file extension to the URL (e.g. .rs) for syntax highlighting.', 6000);
  }
}

/**
 * Duplicates the current paste and switches to edit mode.
 */
function duplicate() {
  editor.value = state.content;
  history.pushState(null, '', '/');
  setEditMode();
  scheduleGutterUpdate();
}

/**
 * Creates a new paste.
 */
function newPaste() {
  editor.value = '';
  history.pushState(null, '', '/');
  setEditMode();
}

/**
 * Saves the current paste to the server.
 */
async function save() {
  const content = editor.value;
  if (!content.trim()) { showToast('Nothing to save.'); return; }
  btnSave.disabled = true;
  btnSave.setAttribute('aria-disabled', 'true');
  try {
    const res = await fetch('/api/paste', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
    if (!res.ok) throw new Error();
    const { id } = await res.json();
    history.pushState({ id }, '', '/' + id);
    state.pasteId = id;
    setViewMode(content, null);
  } catch {
    showToast('Save failed. Try again.');
    btnSave.disabled = false;
    btnSave.removeAttribute('aria-disabled');
  }
}

/**
 * Loads a paste from the server and switches to view mode.
 * @param {string} id - The ID of the paste to load.
 * @param {string} lang - The language of the paste to display.
 */
async function loadPaste(id, lang) {
  try {
    const res = await fetch('/api/paste/' + id);
    if (res.status === 404) {
      editor.value = 'Paste not found or has expired.';
      history.pushState(null, '', '/');
      setEditMode();
      showToast('Paste not found.');
      return;
    }
    if (!res.ok) throw new Error();
    const data = await res.json();
    state.pasteId = id;
    setViewMode(data.content, lang || data.language || null);
  } catch {
    editor.value = '';
    setEditMode();
    showToast('Failed to load paste.');
  }
}



/**
 * Regular expression to validate standard URL paste IDs.
 * @constant {RegExp}
 */
const PASTE_ID_RE = /^[a-zA-Z0-9_-]{1,64}$/;

/**
 * Extracts the requested paste ID and mapped language extension from the URL path.
 * @param {string} path - The raw URL pathname.
 * @returns {{id: string|null, lang: string|null}} Object containing parsed ID and language.
 */
function parsePath(path) {
  const clean = path.replace(/^\//, '');
  if (!clean) return { id: null, lang: null };
  const dot = clean.lastIndexOf('.');
  if (dot > 0) {
    const id = clean.slice(0, dot);
    if (!PASTE_ID_RE.test(id)) return { id: null, lang: null };
    const ext = clean.slice(dot + 1).toLowerCase();
    return { id, lang: EXT[ext] || 'plaintext' };
  }
  if (!PASTE_ID_RE.test(clean)) return { id: null, lang: null };
  return { id: clean, lang: null };
}

/**
 * Determines application behavior based on the current URL route.
 * @param {string} path - The URL pathname to process.
 */
function handlePath(path) {
  const { id, lang } = parsePath(path);
  if (!id) {
    setEditMode();
  } else {
    loadPaste(id, lang);
  }
}

/**
 * Listens for browser history traversal events to update view state.
 */
window.addEventListener('popstate', () => handlePath(window.location.pathname));

// Initial route handling on page load.
handlePath(window.location.pathname);

/**
 * Immediately invoked function expression to restore duplicated editor contents from session storage.
 */
(function () {
  try {
    const dup = sessionStorage.getItem('rps_duplicate');
    if (dup) {
      sessionStorage.removeItem('rps_duplicate');
      editor.value = dup;
      scheduleGutterUpdate();
    }
  } catch {}
}());
// END: To be removed / reworked heavily

// Event Listeners for action buttons
btnSave.addEventListener('click', save);
btnNew.addEventListener('click', newPaste);
btnDuplicate.addEventListener('click', duplicate);