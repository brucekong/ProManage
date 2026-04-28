<script setup lang="ts">
import { ref, watch, nextTick, computed } from "vue";
import { useProjectStore } from "../stores/project";
import type { ProjectConfig } from "../types/project";

const props = defineProps<{
  project: ProjectConfig;
}>();

const emit = defineEmits<{
  edit: [project: ProjectConfig];
  delete: [id: string];
}>();

const store = useProjectStore();

// ─── Status helpers ──────────────────────────
const statusColor = (id: string) => {
  const s = store.processStatuses[id];
  if (s === "Running") return "var(--color-green)";
  if (s === "Error") return "var(--color-red)";
  return "var(--color-muted)";
};

const statusLabel = (id: string) => {
  return store.processStatuses[id] || "Stopped";
};

// ─── Dependency helpers ─────────────────────
const depName = (id: string) => {
  return store.projects.find((p) => p.id === id)?.name;
};

const depStatus = (id: string) => {
  const s = store.processStatuses[id];
  if (s === "Running") return "dep-running";
  if (s === "Stopped") return "dep-stopped";
  return "";
};

// ─── Output panel ────────────────────────────
const showOutput = ref(false);
const outputEl = ref<HTMLDivElement | null>(null);
const autoScroll = ref(true);

const outputs = computed(() => store.processOutputs[props.project.id] || []);
const isWorkspaceProject = computed(() => props.project.project_kind === "workspace");
const hasRunTargets = computed(() => store.runTargetsForProject(props.project).length > 0);

function toggleOutput() {
  showOutput.value = !showOutput.value;
  if (showOutput.value) {
    nextTick(() => scrollToBottom());
  }
}

function scrollToBottom() {
  if (outputEl.value && autoScroll.value) {
    outputEl.value.scrollTop = outputEl.value.scrollHeight;
  }
}

function onOutputScroll() {
  if (!outputEl.value) return;
  const el = outputEl.value;
  // Consider "near bottom" if within 40px of the bottom
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
}

// Auto-scroll when new output arrives
watch(outputs, () => {
  if (showOutput.value) {
    nextTick(() => scrollToBottom());
  }
}, { deep: true });
</script>

<template>
  <div class="project-card" :class="{ running: store.processStatuses[project.id] === 'Running' }">
    <!-- Header -->
    <div class="card-header">
      <span class="status-dot" :style="{ background: statusColor(project.id) }"></span>
      <span class="project-name">{{ project.name }}</span>
      <span class="status-badge" :class="statusLabel(project.id).toLowerCase()">
        {{ statusLabel(project.id) }}
      </span>
      <div class="header-actions">
        <button class="icon-btn" title="Edit" @click="emit('edit', project)">
          ✎
        </button>
        <button class="icon-btn icon-btn-danger" title="Remove" @click="emit('delete', project.id)">
          ✕
        </button>
      </div>
    </div>

    <!-- Details -->
    <div class="card-details">
      <div class="detail-row">
        <span class="label">Path</span>
        <span class="value path" :title="project.path">{{ project.path }}</span>
      </div>
      <div class="detail-row">
        <span class="label">Command</span>
        <span class="value">
          {{ isWorkspaceProject && !hasRunTargets ? "Antigravity workspace" : project.command }}
        </span>
      </div>
      <div class="detail-row">
        <span class="label">Port</span>
        <span class="value">{{ project.port || "auto" }}</span>
      </div>
      <div v-if="project.note" class="detail-row">
        <span class="label">Note</span>
        <span class="value">{{ project.note }}</span>
      </div>
      <div v-if="project.depends_on && project.depends_on.length > 0" class="detail-row">
        <span class="label">Depends on</span>
        <span class="value">
          <span
            v-for="depId in project.depends_on"
            :key="depId"
            class="dep-chip"
            :class="depStatus(depId)"
          >
            {{ depName(depId) || depId }}
          </span>
        </span>
      </div>
    </div>

    <!-- Actions -->
    <div class="card-actions">
      <button
        v-if="hasRunTargets"
        class="btn btn-start"
        :disabled="store.processStatuses[project.id] === 'Running'"
        @click="store.startProject(project.id)"
      >
        ▶ Start
      </button>
      <button
        v-if="hasRunTargets"
        class="btn btn-stop"
        :disabled="store.processStatuses[project.id] !== 'Running'"
        @click="store.stopProject(project.id)"
      >
        ■ Stop
      </button>
      <button
        v-if="hasRunTargets"
        class="btn btn-restart"
        @click="store.restartProject(project.id)"
      >
        ↻ Restart
      </button>
      <button class="btn btn-output" @click="toggleOutput">
        {{ showOutput ? "▼" : "▶" }} Output
        <span v-if="outputs.length > 0" class="output-count">{{ outputs.length }}</span>
      </button>
    </div>

    <!-- Output Panel -->
    <div v-if="showOutput" class="output-section">
      <div class="output-header">
        <span class="output-label">
          {{ store.processStatuses[project.id] === "Running" ? "● Live" : "● Finished" }}
        </span>
        <span class="output-line-count">{{ outputs.length }} lines</span>
        <button class="btn-clear-output" @click="store.clearOutput(project.id)">Clear</button>
      </div>
      <div
        ref="outputEl"
        class="output-lines"
        @scroll="onOutputScroll"
      >
        <div v-if="outputs.length === 0" class="output-empty">No output yet</div>
        <div
          v-for="(line, i) in outputs"
          :key="i"
          class="output-line"
          :class="{ stderr: line.stream === 'stderr' }"
        >
          <span class="line-text">{{ line.text }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.project-card {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 10px;
  padding: 16px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.project-card:hover {
  border-color: var(--color-primary);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.15);
}

.project-card.running {
  border-left: 3px solid var(--color-green);
}

/* ── Header ──────────────────────────────── */
.card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.project-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text);
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-badge {
  font-size: 11px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 10px;
  flex-shrink: 0;
}

.status-badge.running {
  background: rgba(52, 199, 89, 0.15);
  color: var(--color-green);
}

.status-badge.stopped {
  background: rgba(142, 142, 147, 0.15);
  color: var(--color-muted);
}

.status-badge.error {
  background: rgba(255, 69, 58, 0.15);
  color: var(--color-red);
}

.header-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.icon-btn {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--color-muted);
  cursor: pointer;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}

.icon-btn:hover {
  background: var(--color-hover);
  color: var(--color-text);
}

.icon-btn-danger:hover {
  color: var(--color-red);
  background: rgba(255, 69, 58, 0.1);
}

/* ── Details ─────────────────────────────── */
.card-details {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
}

.detail-row {
  display: flex;
  gap: 8px;
  font-size: 12px;
}

.label {
  color: var(--color-muted);
  min-width: 65px;
  flex-shrink: 0;
}

.value {
  color: var(--color-text-secondary);
  word-break: break-all;
}

.value.path {
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 11px;
}

.dep-chip {
  display: inline-block;
  font-size: 11px;
  padding: 1px 7px;
  border-radius: 8px;
  margin: 1px 4px 1px 0;
  background: rgba(142, 142, 147, 0.12);
  color: var(--color-text-secondary);
}

.dep-chip.dep-running {
  background: rgba(52, 199, 89, 0.12);
  color: var(--color-green);
}

/* ── Actions ─────────────────────────────── */
.card-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.btn {
  padding: 6px 14px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-bg);
  color: var(--color-text);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-start {
  color: var(--color-green);
  border-color: var(--color-green);
}

.btn-start:hover:not(:disabled) {
  background: rgba(52, 199, 89, 0.1);
}

.btn-stop {
  color: var(--color-red);
  border-color: var(--color-red);
}

.btn-stop:hover:not(:disabled) {
  background: rgba(255, 69, 58, 0.1);
}

.btn-restart:hover:not(:disabled) {
  background: var(--color-hover);
}

.btn-output {
  margin-left: auto;
  color: var(--color-primary);
  border-color: var(--color-primary);
  display: flex;
  align-items: center;
  gap: 4px;
}

.btn-output:hover {
  background: rgba(0, 122, 255, 0.08);
}

.output-count {
  background: var(--color-primary);
  color: white;
  font-size: 10px;
  font-weight: 600;
  padding: 0 5px;
  border-radius: 6px;
  min-width: 16px;
  text-align: center;
  line-height: 16px;
}

/* ── Output Panel ────────────────────────── */
.output-section {
  margin-top: 12px;
  border-top: 1px solid var(--color-border);
  padding-top: 10px;
}

.output-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  font-size: 11px;
}

.output-label {
  color: var(--color-green);
  font-weight: 600;
}

.output-line-count {
  color: var(--color-muted);
  flex: 1;
}

.btn-clear-output {
  padding: 2px 8px;
  border-radius: 4px;
  border: 1px solid var(--color-border);
  background: transparent;
  color: var(--color-muted);
  font-size: 11px;
  cursor: pointer;
}

.btn-clear-output:hover {
  background: var(--color-hover);
  color: var(--color-text);
}

.output-lines {
  max-height: 200px;
  overflow-y: auto;
  background: var(--color-bg);
  border-radius: 6px;
  padding: 8px;
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 11px;
  line-height: 1.5;
}

.output-empty {
  color: var(--color-muted);
  font-family: -apple-system, sans-serif;
  font-size: 12px;
  text-align: center;
  padding: 12px;
}

.output-line {
  color: var(--color-text-secondary);
  white-space: pre-wrap;
  word-break: break-all;
}

.output-line.stderr {
  color: var(--color-red);
}

.line-text {
  /* ensure wrapping works */
}
</style>
