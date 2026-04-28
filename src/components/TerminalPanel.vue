<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useProjectStore, type ProjectRunTarget } from "../stores/project";
import type { ProcessOutputLine } from "../stores/project";
import { detectPortsFromOutput } from "../utils/ports";
import { useI18n } from "../i18n";

const store = useProjectStore();
const { t, status: statusText } = useI18n();
const outputEl = ref<HTMLDivElement | null>(null);
const xtermEl = ref<HTMLDivElement | null>(null);
const autoScroll = ref(true);
const terminalInput = ref("");
const inputError = ref("");
let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let renderedOutputCount = 0;
let pendingWrites: string[] = [];
let writeRafScheduled = false;
const LINK_RE = /\b(?:https?:\/\/|www\.|localhost(?::\d+)?(?:\/[^\s]*)?|127\.0\.0\.1(?::\d+)?(?:\/[^\s]*)?)(?:[^\s<>"']*)/gi;

type TerminalLink = {
  text: string;
  range: {
    start: { x: number; y: number };
    end: { x: number; y: number };
  };
  activate: () => void;
  decorations: { underline: boolean; pointerCursor: boolean };
};

function normalizeDetectedUrl(raw: string) {
  const trimmed = raw.replace(/[),.;!?]+$/g, "");
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  if (/^www\./i.test(trimmed)) return `https://${trimmed}`;
  if (/^(localhost|127\.0\.0\.1)/i.test(trimmed)) return `http://${trimmed}`;
  return trimmed;
}

function collectWrappedLine(instance: Terminal, rowIndex: number) {
  const buffer = instance.buffer.active;
  let startRow = rowIndex;
  let startLine = buffer.getLine(startRow);

  while (startRow > 0 && startLine?.isWrapped) {
    startRow -= 1;
    startLine = buffer.getLine(startRow);
  }

  const segments: Array<{ row: number; text: string; start: number; end: number }> = [];
  let cursor = 0;
  let currentRow = startRow;

  while (currentRow < buffer.length) {
    const line = buffer.getLine(currentRow);
    if (!line) break;

    const text = line.translateToString(false);
    const end = cursor + text.length;
    segments.push({ row: currentRow, text, start: cursor, end });
    cursor = end;

    const nextLine = buffer.getLine(currentRow + 1);
    if (!nextLine?.isWrapped) break;
    currentRow += 1;
  }

  return {
    text: segments.map((segment) => segment.text).join(""),
    segments,
  };
}

function indexToBufferPosition(
  segments: Array<{ row: number; text: string; start: number; end: number }>,
  index: number
) {
  const segment =
    segments.find((item) => index >= item.start && index <= item.end) ||
    segments[segments.length - 1];
  const col = Math.max(0, Math.min(index - segment.start, segment.text.length));

  return {
    x: col + 1,
    y: segment.row + 1,
  };
}

function installTerminalLinkProvider(instance: Terminal) {
  instance.registerLinkProvider({
    provideLinks(y, callback) {
      const wrapped = collectWrappedLine(instance, y - 1);
      if (!wrapped.text || wrapped.segments.length === 0) {
        callback(undefined);
        return;
      }

      const links: TerminalLink[] = [];

      for (const match of wrapped.text.matchAll(LINK_RE)) {
        const matchText = match[0];
        const startIndex = match.index ?? -1;
        if (startIndex < 0) continue;

        const url = normalizeDetectedUrl(matchText);
        const endIndexExclusive = startIndex + matchText.length;
        const start = indexToBufferPosition(wrapped.segments, startIndex);
        const end = indexToBufferPosition(wrapped.segments, endIndexExclusive);

        links.push({
          text: url,
          range: { start, end },
          activate: () => {
            void openUrl(url);
          },
          decorations: {
            underline: true,
            pointerCursor: true,
          },
        });
      }

      callback(links.length > 0 ? links : undefined);
    },
  });
}

function flushTerminalWrites() {
  writeRafScheduled = false;
  if (!terminal || pendingWrites.length === 0) return;
  const batch = pendingWrites.join("");
  pendingWrites = [];
  terminal.write(batch);
  scrollToBottom();
}

const processOutputListener = ((event: Event) => {
  const line = (event as CustomEvent<ProcessOutputLine>).detail;
  if (!terminal || line.project_id !== store.selectedOutputId) return;
  pendingWrites.push(line.text);
  renderedOutputCount += 1;
  if (!writeRafScheduled) {
    writeRafScheduled = true;
    requestAnimationFrame(flushTerminalWrites);
  }
}) as EventListener;

const selectedProject = computed(() => store.selectedProject);
const isWorkspaceProject = computed(() => selectedProject.value?.project_kind === "workspace");
const hasRunTargets = computed(() => store.selectedProjectTargets.length > 0);
const status = computed(() => {
  const id = store.selectedProcessId || selectedProject.value?.id;
  return id ? store.processStatuses[id] || "Stopped" : "Idle";
});
const selectedTarget = computed(() => {
  return (
    store.selectedProjectTargets.find((target) => target.id === store.selectedProcessId) ||
    store.selectedProjectTargets[0] ||
    null
  );
});
const outputs = computed(() => {
  return store.selectedOutputs;
});
const detectedPorts = computed(() => {
  if (!["Starting", "Running"].includes(status.value)) return [];
  return detectPortsFromOutput(outputs.value);
});

function targetPorts(targetId: string) {
  if (!["Starting", "Running"].includes(store.processStatuses[targetId] || "Stopped")) return [];
  return detectPortsFromOutput(store.processOutputs[targetId] || []);
}

function scrollToBottom() {
  if (!outputEl.value || !autoScroll.value) return;
  outputEl.value.scrollTop = outputEl.value.scrollHeight;
}

function onOutputScroll() {
  if (!outputEl.value) return;
  const el = outputEl.value;
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
}

function clearOutput() {
  const processId = store.selectedOutputId || store.selectedProcessId || selectedProject.value?.id;
  if (processId) {
    pendingWrites = [];
    store.clearOutput(processId);
    renderTerminalFromStore(true);
  }
}

function statusLabel(id: string) {
  return store.processStatuses[id] || "Stopped";
}

function selectTerminalTarget(target: ProjectRunTarget) {
  store.selectTarget(target);
}

async function startTarget(target: ProjectRunTarget) {
  const project = selectedProject.value;
  if (!project) return;
  if (target.name === "default") {
    await store.startProject(project.id);
    return;
  }
  await store.startProjectCommand(project.id, target.id, `${project.name} · ${target.label}`, target.command);
}

async function startCurrentTarget() {
  const target = selectedTarget.value;
  if (target) {
    await startTarget(target);
    return;
  }

  const project = selectedProject.value;
  if (project) {
    await store.startProject(project.id);
  }
}

async function stopCurrentTarget() {
  const project = selectedProject.value;
  const processId = store.selectedProcessId || project?.id;
  if (processId) {
    await store.stopProject(processId);
  }
}

async function restartCurrentTarget() {
  if (["Starting", "Running"].includes(status.value)) {
    await stopCurrentTarget();
  }
  await startCurrentTarget();
}

async function sendInput() {
  inputError.value = "";
  const project = selectedProject.value;
  const processId = store.selectedProcessId || project?.id;
  if (!processId || !terminalInput.value) return;

  try {
    await store.writeProjectInput(processId, `${terminalInput.value}\r`);
    terminalInput.value = "";
  } catch (e) {
    console.error("Write terminal input failed:", e);
    inputError.value = t("terminal.inputError");
  }
}

async function sendTerminalData(data: string) {
  inputError.value = "";
  const project = selectedProject.value;
  const processId = store.selectedProcessId || project?.id;
  if (!processId || !["Starting", "Running"].includes(status.value)) return;

  try {
    await store.writeProjectInput(processId, data);
  } catch (e) {
    console.error("Write terminal input failed:", e);
    inputError.value = t("terminal.inputError");
  }
}

function initTerminal() {
  if (!xtermEl.value || terminal) return;
  terminal = new Terminal({
    cursorBlink: true,
    convertEol: true,
    fontFamily: "SF Mono, JetBrains Mono, Fira Code, Menlo, monospace",
    fontSize: 12,
    lineHeight: 1.45,
    scrollback: 20000,
    theme: {
      background: "#070b12",
      foreground: "#bfd1df",
      cursor: "#69baf5",
      selectionBackground: "#17344a",
      black: "#0b1119",
      red: "#ff6d82",
      green: "#7ce2bc",
      yellow: "#8fc8ff",
      blue: "#69baf5",
      magenta: "#9fb7ff",
      cyan: "#86d9e9",
      white: "#eef7ff",
      brightBlack: "#708796",
      brightRed: "#ff8a9c",
      brightGreen: "#9ff0c5",
      brightYellow: "#d8f0ff",
      brightBlue: "#b7ddff",
      brightMagenta: "#c5d2ff",
      brightCyan: "#c5f6ff",
      brightWhite: "#f6fbff",
    },
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(xtermEl.value);
  installTerminalLinkProvider(terminal);
  fitAddon.fit();
  terminal.onData((data) => {
    void sendTerminalData(data);
  });
}

function renderTerminalFromStore(reset = false) {
  if (!terminal) return;
  const lines = outputs.value;
  reset = reset || renderedOutputCount > lines.length;

  if (reset) {
    terminal.reset();
    renderedOutputCount = 0;
    const project = selectedProject.value;
    if (project) {
      terminal.writeln(
        `\x1b[38;5;215mprostation\x1b[0m attached to ${store.selectedProcessLabel || project.name}`
      );
    }
  }

  for (let index = renderedOutputCount; index < lines.length; index++) {
    terminal.write(lines[index].text);
  }
  renderedOutputCount = lines.length;
}

function focusTerminal() {
  terminal?.focus();
}

function fitTerminal() {
  fitAddon?.fit();
  syncPtySize();
}

function syncPtySize() {
  if (!terminal) return;
  const processId = store.selectedProcessId || selectedProject.value?.id;
  if (!processId) return;
  const { rows, cols } = terminal;
  if (rows > 0 && cols > 0) {
    void store.resizePty(processId, rows, cols);
  }
}

watch(
  () => store.selectedOutputId,
  (outputId, oldOutputId) =>
    nextTick(() => {
      initTerminal();
      fitTerminal();
      renderTerminalFromStore(outputId !== oldOutputId);
      scrollToBottom();
      syncPtySize();
    })
);

onMounted(() => {
  initTerminal();
  renderTerminalFromStore(true);
  window.addEventListener("prostation-process-output", processOutputListener);
  window.addEventListener("resize", fitTerminal);
});

onBeforeUnmount(() => {
  window.removeEventListener("prostation-process-output", processOutputListener);
  window.removeEventListener("resize", fitTerminal);
  terminal?.dispose();
  terminal = null;
  fitAddon = null;
});
</script>

<template>
  <aside class="terminal-panel">
    <div class="terminal-topbar">
      <div class="terminal-title">
        <span class="eyebrow">{{ t("terminal.title") }}</span>
        <strong>{{ store.selectedProcessLabel || selectedProject?.name || t("terminal.noProject") }}</strong>
      </div>
      <span class="terminal-status" :class="status.toLowerCase()">{{ statusText(status) }}</span>
    </div>

    <div v-if="selectedProject" class="terminal-meta">
      <div>
        <span>{{ t("terminal.path") }}</span>
        <strong :title="selectedProject.path">{{ selectedProject.path }}</strong>
      </div>
      <div v-if="!isWorkspaceProject || hasRunTargets">
        <span>{{ t("terminal.command") }}</span>
        <strong>{{ selectedTarget?.command || selectedProject.command }}</strong>
      </div>
      <div v-if="!isWorkspaceProject || hasRunTargets">
        <span>{{ t("terminal.port") }}</span>
        <strong>{{ detectedPorts.length ? detectedPorts.map(port => `:${port}`).join(", ") : t("terminal.detecting") }}</strong>
      </div>
      <div v-if="isWorkspaceProject">
        <span>{{ t("terminal.type") }}</span>
        <strong>{{ t("terminal.workspaceType") }}</strong>
      </div>
    </div>

    <div v-if="selectedProject && store.selectedProjectTargets.length > 1" class="terminal-tabs">
      <button
        v-for="target in store.selectedProjectTargets"
        :key="target.id"
        class="terminal-tab"
        :class="[target.kind, { active: store.selectedProcessId === target.id, running: statusLabel(target.id) === 'Running', starting: statusLabel(target.id) === 'Starting' }]"
        @click="selectTerminalTarget(target)"
      >
        <span>{{ target.label }}</span>
        <strong v-if="targetPorts(target.id).length">
          {{ targetPorts(target.id).map(port => `:${port}`).join(", ") }}
        </strong>
      </button>
    </div>

    <div class="terminal-actions" v-if="selectedProject">
      <button
        class="terminal-btn start"
        :disabled="['Starting', 'Running'].includes(status)"
        @click="startCurrentTarget"
      >
        {{ t("common.start") }}
      </button>
      <button
        class="terminal-btn stop"
        :disabled="!['Starting', 'Running'].includes(status)"
        @click="stopCurrentTarget"
      >
        {{ t("common.stop") }}
      </button>
      <button class="terminal-btn" @click="restartCurrentTarget">
        {{ t("common.restart") }}
      </button>
      <button class="terminal-btn muted" @click="clearOutput">{{ t("common.clear") }}</button>
    </div>

    <div ref="outputEl" class="terminal-output" @click="focusTerminal" @scroll="onOutputScroll">
      <template v-if="selectedProject">
        <div ref="xtermEl" class="xterm-host"></div>
      </template>
      <div v-else class="terminal-placeholder">
        <span class="scan-ring"></span>
        <strong>{{ t("terminal.selectProject") }}</strong>
        <span>{{ t("terminal.outputHint") }}</span>
      </div>
    </div>

    <div v-if="selectedProject" class="terminal-input-wrap">
      <div class="terminal-input-row" :class="{ disabled: !['Starting', 'Running'].includes(status) }">
        <span class="input-prompt">$</span>
        <input
          v-model="terminalInput"
          class="terminal-input"
          type="text"
          :disabled="!['Starting', 'Running'].includes(status)"
          :placeholder="t('terminal.inputPlaceholder')"
          @keyup.enter="sendInput"
        />
        <button
          class="terminal-send"
          :disabled="!['Starting', 'Running'].includes(status) || !terminalInput"
          @click="sendInput"
        >
          {{ t("common.send") }}
        </button>
      </div>
      <p v-if="inputError" class="terminal-input-error">{{ inputError }}</p>
    </div>
  </aside>
</template>

<style scoped>
.terminal-panel {
  min-width: 260px;
  width: clamp(260px, 34vw, 420px);
  height: 100%;
  display: flex;
  flex-direction: column;
  border-left: 1px solid rgba(190, 224, 255, 0.1);
  background:
    radial-gradient(ellipse 88% 44% at 50% 0%, rgba(82, 169, 235, 0.13), transparent 72%),
    linear-gradient(180deg, rgba(19, 16, 19, 0.76), rgba(9, 9, 11, 0.94));
  box-shadow:
    inset 1px 0 0 rgba(255, 255, 255, 0.04),
    -24px 0 70px rgba(0, 0, 0, 0.24);
  backdrop-filter: blur(24px) saturate(120%);
}

.terminal-topbar {
  height: 72px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 18px;
  border-bottom: 1px solid rgba(190, 224, 255, 0.09);
}

.terminal-title {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.eyebrow {
  color: var(--color-muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
}

.terminal-title strong {
  overflow: hidden;
  color: var(--color-text);
  font-size: 15px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-status {
  border: 1px solid rgba(190, 224, 255, 0.16);
  border-radius: 999px;
  padding: 4px 9px;
  color: var(--color-muted);
  font-size: 11px;
  font-weight: 700;
}

.terminal-status.running {
  border-color: rgba(131, 230, 177, 0.34);
  color: var(--color-green);
  box-shadow: 0 0 22px rgba(131, 230, 177, 0.12);
}

.terminal-status.starting {
  border-color: rgba(105, 186, 245, 0.34);
  color: var(--color-primary);
  box-shadow: 0 0 22px rgba(82, 169, 235, 0.12);
}

.terminal-status.error {
  border-color: rgba(255, 109, 130, 0.34);
  color: var(--color-red);
}

.terminal-meta {
  display: grid;
  grid-template-columns: 1fr;
  gap: 9px;
  padding: 16px 18px;
  border-bottom: 1px solid rgba(190, 224, 255, 0.08);
}

.terminal-meta div {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.terminal-meta span {
  color: var(--color-muted);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.terminal-meta strong {
  overflow: hidden;
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-tabs {
  display: flex;
  gap: 0;
  padding: 0 18px;
  border-bottom: 1px solid rgba(190, 224, 255, 0.08);
}

.terminal-tab {
  position: relative;
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 42px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--color-text-secondary);
  padding: 0 16px;
  font-size: 12px;
  font-weight: 800;
  cursor: pointer;
  transition: 0.16s ease;
}

.terminal-tab::before {
  position: absolute;
  inset: 9px 8px;
  z-index: -1;
  content: "";
  border-radius: 8px;
  background: transparent;
  transition: 0.16s ease;
}

.terminal-tab:hover::before {
  background: rgba(190, 224, 255, 0.05);
}

.terminal-tab.web {
  color: #d8f0ff;
}

.terminal-tab.api {
  color: #c3f5f8;
}

.terminal-tab.build {
  color: #cdbbff;
}

.terminal-tab.active {
  border-bottom-color: var(--color-primary);
  color: var(--color-text);
}

.terminal-tab.active::before {
  background: rgba(105, 186, 245, 0.1);
  box-shadow: inset 0 0 0 1px rgba(105, 186, 245, 0.18);
}

.terminal-tab.running strong {
  color: var(--color-green);
}

.terminal-tab.starting strong {
  color: var(--color-primary);
}

.terminal-tab strong {
  color: var(--color-muted);
  font-family: var(--font-mono);
  font-size: 11px;
}

.terminal-actions {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
  padding:10px;
}

.terminal-btn {
  min-width: 0;
  height: 34px;
  border: 1px solid rgba(190, 224, 255, 0.11);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.045);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}

.terminal-btn:hover:not(:disabled) {
  border-color: rgba(105, 186, 245, 0.34);
  color: var(--color-text);
  background: rgba(105, 186, 245, 0.08);
}

.terminal-btn:disabled {
  cursor: not-allowed;
  opacity: 0.36;
}

.terminal-btn.start {
  color: var(--color-green);
}

.terminal-btn.stop {
  color: var(--color-red);
}

.terminal-btn.muted {
  color: var(--color-muted);
}

.terminal-output {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 18px;
  background:
    radial-gradient(ellipse 90% 34% at 50% 0%, rgba(105, 186, 245, 0.08), transparent 70%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.025), transparent);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
}

.xterm-host {
  width: 100%;
  height: 100%;
}

:deep(.xterm) {
  height: 100%;
  padding: 0;
}

:deep(.xterm-viewport) {
  background: transparent !important;
}

:deep(.xterm-screen) {
  width: 100% !important;
}

.terminal-input-wrap {
  padding: 12px 18px 18px;
  border-top: 1px solid rgba(190, 224, 255, 0.08);
}

.terminal-input-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-height: 42px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 14px;
  background: rgba(9, 8, 10, 0.62);
  padding: 0 8px 0 12px;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.terminal-input-row:focus-within {
  border-color: rgba(105, 186, 245, 0.4);
  box-shadow: 0 0 0 3px rgba(105, 186, 245, 0.08);
}

.terminal-input-row.disabled {
  opacity: 0.48;
}

.input-prompt {
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-weight: 800;
}

.terminal-input {
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: 12px;
}

.terminal-input::placeholder {
  color: var(--color-muted);
}

.terminal-send {
  height: 28px;
  min-width: 58px;
  border: 1px solid rgba(105, 186, 245, 0.28);
  border-radius: 10px;
  background: rgba(105, 186, 245, 0.12);
  color: #d8f0ff;
  font-size: 12px;
  font-weight: 800;
  cursor: pointer;
}

.terminal-send:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}

.terminal-input-error {
  margin-top: 8px;
  color: var(--color-red);
  font-size: 12px;
}

.terminal-line {
  display: grid;
  grid-template-columns: 76px minmax(0, 1fr);
  gap: 10px;
  padding: 2px 0;
  color: #d3c5bd;
}

.terminal-line.boot {
  color: #d8f0ff;
}

.terminal-line.stderr {
  color: #ff8a9c;
}

.prompt {
  color: var(--color-primary);
  opacity: 0.75;
}

.line-text {
  min-width: 0;
  white-space: pre-wrap;
  word-break: break-word;
}

.terminal-empty,
.terminal-placeholder {
  color: var(--color-muted);
}

.terminal-empty {
  padding: 10px 0 0 86px;
}

.terminal-placeholder {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  text-align: center;
}

.terminal-placeholder strong {
  color: var(--color-text);
  font-size: 16px;
}

.scan-ring {
  width: 54px;
  height: 54px;
  border: 1px solid rgba(105, 186, 245, 0.26);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  box-shadow: 0 0 38px rgba(82, 169, 235, 0.18);
  animation: spin 1.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@media (max-width: 1120px) {
  .terminal-panel {
    min-width: 320px;
    width: 38vw;
  }
}
</style>
