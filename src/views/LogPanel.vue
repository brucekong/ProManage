<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { NSelect } from "naive-ui";
import { useProjectStore } from "../stores/project";

const store = useProjectStore();

interface LogEntry {
  timestamp: string;
  level: string;
  project_id: string | null;
  message: string;
}

const logs = ref<LogEntry[]>([]);
const filter = ref<"all" | "info" | "warn" | "error">("all");
const selectedProject = ref<string>("all");

const filteredLogs = () => {
  return logs.value.filter((log) => {
    if (filter.value !== "all" && log.level !== filter.value) return false;
    if (selectedProject.value !== "all" && log.project_id !== selectedProject.value) return false;
    return true;
  });
};

function formatTime(ts: string) {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString();
  } catch {
    return ts;
  }
}

function logClass(level: string) {
  switch (level) {
    case "error": return "log-error";
    case "warn": return "log-warn";
    default: return "log-info";
  }
}

const levelOptions = [
  { label: "All Levels", value: "all" },
  { label: "Info", value: "info" },
  { label: "Warning", value: "warn" },
  { label: "Error", value: "error" },
];

const projectOptions = computed(() => [
  { label: "All Projects", value: "all" },
  ...store.projects.map((p) => ({ label: p.name, value: p.id })),
]);

onMounted(async () => {
  // Initial logs will come from Rust backend events
});
</script>

<template>
  <div class="log-panel">
    <div class="log-toolbar">
      <div class="filters">
        <NSelect v-model:value="filter" :options="levelOptions" class="log-select" />
        <NSelect v-model:value="selectedProject" :options="projectOptions" class="log-select" />
      </div>

      <button class="btn btn-clear" @click="logs = []">Clear</button>
    </div>

    <div class="log-list" v-if="filteredLogs().length > 0">
      <div
        v-for="(log, i) in filteredLogs()"
        :key="i"
        class="log-entry"
        :class="logClass(log.level)"
      >
        <span class="log-time">{{ formatTime(log.timestamp) }}</span>
        <span class="log-level">[{{ log.level.toUpperCase() }}]</span>
        <span class="log-message">{{ log.message }}</span>
      </div>
    </div>

    <div class="log-empty" v-else>
      <p>No logs yet. Start/stop projects to see logs here.</p>
    </div>
  </div>
</template>

<style scoped>
.log-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.log-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  gap: 12px;
}

.filters {
  display: flex;
  gap: 8px;
}

.log-select {
  width: 160px;
}

.btn {
  padding: 6px 14px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-surface);
  color: var(--color-text);
  font-size: 12px;
  cursor: pointer;
}

.btn:hover {
  background: var(--color-hover);
}

.log-list {
  flex: 1;
  overflow-y: auto;
  font-family: "SF Mono", "Fira Code", monospace;
  font-size: 12px;
  background: var(--color-surface);
  border-radius: 8px;
  border: 1px solid var(--color-border);
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.log-entry {
  padding: 3px 6px;
  border-radius: 3px;
  display: flex;
  gap: 8px;
}

.log-entry.log-info { color: var(--color-text-secondary); }
.log-entry.log-warn { color: var(--color-orange); background: rgba(105, 186, 245, 0.08); }
.log-entry.log-error { color: var(--color-red); background: rgba(255, 69, 58, 0.08); }

.log-time {
  color: var(--color-muted);
  flex-shrink: 0;
}

.log-level {
  font-weight: 600;
  min-width: 48px;
  flex-shrink: 0;
}

.log-message {
  word-break: break-all;
}

.log-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--color-muted);
  font-size: 13px;
}
</style>
