<script setup lang="ts">
import { computed, ref } from "vue";
import { useProjectStore } from "../stores/project";
import ProjectFormModal from "../components/ProjectFormModal.vue";
import type { ProjectConfig } from "../types/project";
import { detectPortsFromOutput } from "../utils/ports";

const store = useProjectStore();
const query = ref("");
const dragOver = ref(false);
const showForm = ref(false);
const editingProject = ref<ProjectConfig | null>(null);

const filteredProjects = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return store.projects;
  return store.projects.filter((project) =>
    [project.name, project.path, project.command, project.group, project.note]
      .filter(Boolean)
      .some((value) => value.toLowerCase().includes(keyword))
  );
});

function statusLabel(id: string) {
  return store.processStatuses[id] || "Stopped";
}

function targetIcon() {
  return "▶";
}

function runTargets(project: ProjectConfig) {
  return store.runTargetsForProject(project);
}

function detectedPorts(id: string) {
  if (!["Starting", "Running"].includes(statusLabel(id))) return [];
  return detectPortsFromOutput(store.processOutputs[id] || []);
}

async function startTarget(project: ProjectConfig, target: ReturnType<typeof runTargets>[number]) {
  if (target.name === "default") {
    await store.startProject(project.id);
    return;
  }
  await store.startProjectCommand(project.id, target.id, `${project.name} · ${target.label}`, target.command);
}

async function stopTarget(targetId: string) {
  await store.stopProject(targetId);
}

async function openProjectFolder(project: ProjectConfig) {
  try {
    const { openPath, revealItemInDir } = await import("@tauri-apps/plugin-opener");
    try {
      await openPath(project.path);
      return;
    } catch (openPathError) {
      console.warn("openPath failed, trying shell open:", openPathError);
    }

    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(project.path);
      return;
    } catch (shellOpenError) {
      console.warn("shell open failed, revealing folder instead:", shellOpenError);
    }

    await revealItemInDir(project.path);
  } catch (e) {
    console.error("Open project folder failed:", e);
    alert("Unable to open the project folder in this runtime.");
  }
}

function selectTarget(project: ProjectConfig, target: ReturnType<typeof runTargets>[number]) {
  store.selectProcess(project.id, target.id, `${project.name} · ${target.label}`);
}

function selectProjectCard(project: ProjectConfig) {
  if (store.selectedProjectId === project.id) {
    return;
  }

  store.selectProject(project.id);

  const runningTarget = runTargets(project).find((target) =>
    ["Starting", "Running"].includes(statusLabel(target.id))
  );
  if (runningTarget) {
    store.selectProcess(project.id, runningTarget.id, `${project.name} · ${runningTarget.label}`);
    return;
  }

  const firstTarget = runTargets(project)[0];
  if (firstTarget) {
    store.selectProcess(project.id, firstTarget.id, `${project.name} · ${firstTarget.label}`);
  }
}

function openAddModal() {
  editingProject.value = null;
  showForm.value = true;
}

function openEditModal(project: ProjectConfig) {
  editingProject.value = { ...project };
  showForm.value = true;
}

function closeForm() {
  showForm.value = false;
  editingProject.value = null;
}

async function onFormSaved(project: ProjectConfig) {
  if (editingProject.value) {
    await store.updateProject(project);
  } else {
    await store.addProject(project);
  }
  closeForm();
}

async function handleDelete(id: string) {
  if (confirm("Remove this project from the list?")) {
    await store.removeProject(id);
  }
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  dragOver.value = true;
}

function onDragLeave() {
  dragOver.value = false;
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  dragOver.value = false;

  const files = e.dataTransfer?.files;
  if (!files) return;

  for (let i = 0; i < files.length; i++) {
    const path = (files[i] as any).path || files[i].name;
    if (path) {
      try {
        await store.scanDirectory(path);
      } catch (err) {
        console.error("Scan dropped path failed:", err);
      }
    }
  }
}
</script>

<template>
  <div class="project-list-view" @dragover="onDragOver" @dragleave="onDragLeave" @drop="onDrop">
    <header class="command-strip">
      <div class="title-block">
        <span class="section-kicker">Workspace</span>
        <h2>Project Matrix</h2>
      </div>

      <section class="ops-bar">
        <input v-model="query" class="search-input" type="text" placeholder="Search projects" />
        <button class="btn" @click="openAddModal">Add</button>
        <button
          class="btn danger"
          :disabled="store.runningCount === 0"
          @click="store.stopAll()"
        >
          Stop All
        </button>
      </section>
    </header>

    <section class="status-grid">
      <div class="metric">
        <span>Running</span>
        <strong>{{ store.runningCount }}</strong>
      </div>
      <div class="metric">
        <span>Stopped</span>
        <strong>{{ store.stoppedCount }}</strong>
      </div>
      <div class="metric">
        <span>Error</span>
        <strong>{{ store.errorCount }}</strong>
      </div>
      <div class="metric">
        <span>Total</span>
        <strong>{{ store.projects.length }}</strong>
      </div>
    </section>

    <div v-if="dragOver" class="drop-overlay">
      <div class="drop-hint">Drop folders to scan projects</div>
    </div>

    <section v-if="store.projects.length > 0" class="project-table">
      <div
        v-for="project in filteredProjects"
        :key="project.id"
        class="project-row"
        role="button"
        tabindex="0"
        :class="{
          selected: store.selectedProjectId === project.id,
          running: runTargets(project).some(target => ['Starting', 'Running'].includes(statusLabel(target.id))),
          error: runTargets(project).some(target => statusLabel(target.id) === 'Error')
        }"
        @click="selectProjectCard(project)"
        @keyup.enter="selectProjectCard(project)"
      >
        <span class="status-rail"></span>
        <span class="project-main">
          <span class="project-name">{{ project.name }}</span>
          <span class="project-path" :title="project.path">{{ project.path }}</span>
        </span>
        <span class="project-meta">
          <span class="service-list">
            <span
              v-for="target in runTargets(project)"
              :key="target.id"
              class="service-item"
              role="button"
              tabindex="0"
              @click.stop="selectTarget(project, target)"
              @keyup.enter.stop="selectTarget(project, target)"
            >
              <div class="service-info">
                <span class="service-kind" :class="target.kind">
                {{ target.label }}
              </span>
              <span
                class="service-state"
                :class="statusLabel(target.id).toLowerCase()"
              >
                {{ statusLabel(target.id) }}
              </span>
              </div>
              <span class="target-port">
                {{ detectedPorts(target.id).map(port => `:${port}`).join(", ") }}
              </span>
            </span>
          </span>
        </span>
        <span class="row-actions">
          <span v-for="target in runTargets(project)" :key="target.id" class="target-control">
            <button
              class="target-action start"
              :class="[target.kind, { running: ['Starting', 'Running'].includes(statusLabel(target.id)) }]"
              :data-tooltip="['Starting', 'Running'].includes(statusLabel(target.id))
                ? `Stop ${target.label}`
                : `Start ${target.label}: ${target.command}`"
              @click.stop="['Starting', 'Running'].includes(statusLabel(target.id))
                ? stopTarget(target.id)
                : startTarget(project, target)"
            >
              {{ ['Starting', 'Running'].includes(statusLabel(target.id)) ? '■' : targetIcon() }}
            </button>
          </span>
          <button class="icon-action" data-tooltip="Open folder" @click.stop="openProjectFolder(project)">
            📁
          </button>
          <button class="icon-action" data-tooltip="Edit project" @click.stop="openEditModal(project)">
            ✎
          </button>
          <button class="icon-action danger" data-tooltip="Remove project" @click.stop="handleDelete(project.id)">
            ×
          </button>
        </span>
      </div>

      <div v-if="filteredProjects.length === 0" class="empty-inline">
        No projects match the current search.
      </div>
    </section>

    <section v-else class="empty-state">
      <div class="empty-orbit"></div>
      <h3>No Projects</h3>
      <p>Scan a directory or add a project manually to start the workspace.</p>
      <button class="btn primary" @click="openAddModal">Add Project</button>
    </section>

    <ProjectFormModal
      :show="showForm"
      :project="editingProject"
      :otherProjects="store.projects"
      @close="closeForm"
      @saved="onFormSaved"
    />
  </div>
</template>

<style scoped>
.project-list-view {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: hidden;
  container-type: inline-size;
}

.command-strip,
.status-grid,
.project-table,
.empty-state {
  border: 1px solid rgba(190, 224, 255, 0.11);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.062), rgba(255, 255, 255, 0.026)),
    rgba(18, 16, 18, 0.58);
  box-shadow:
    0 24px 80px rgba(0, 0, 0, 0.28),
    inset 0 1px 0 rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(28px) saturate(120%);
}

.command-strip {
  position: relative;
  overflow: hidden;
  display: grid;
  grid-template-columns: minmax(220px, 0.7fr) minmax(360px, 1.4fr);
  gap: 18px;
  align-items: center;
  padding: 20px;
  border-radius: 22px;
}

.command-strip::before {
  position: absolute;
  inset: -40% 22% auto 18%;
  height: 160px;
  content: "";
  pointer-events: none;
  /* background: radial-gradient(ellipse at center, rgba(236, 111, 55, 0.24), transparent 68%); */
  filter: blur(22px);
}

.title-block {
  position: relative;
  min-width: 0;
}

.section-kicker {
  color: var(--color-primary);
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.16em;
  text-transform: uppercase;
}

.title-block h2 {
  color: var(--color-text);
  font-size: 24px;
  line-height: 1.1;
  font-weight: 760;
}

.ops-bar {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 8px;
  min-width: 0;
  align-items: center;
  justify-self: stretch;
  padding: 0;
  border-radius: 0;
}

.search-input {
  min-width: 0;
  flex: 1;
  height: 42px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 14px;
  background: rgba(10, 9, 11, 0.52);
  color: var(--color-text);
  padding: 0 14px;
  outline: none;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.search-input:focus {
  border-color: rgba(105, 186, 245, 0.46);
  box-shadow:
    0 0 0 3px rgba(105, 186, 245, 0.09),
    0 0 40px rgba(82, 169, 235, 0.1);
}

.btn {
  height: 42px;
  min-width: 72px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.052);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 800;
  cursor: pointer;
  transition: 0.16s ease;
}

.btn:hover:not(:disabled) {
  border-color: rgba(105, 186, 245, 0.36);
  color: var(--color-text);
  background: rgba(105, 186, 245, 0.09);
}

.btn:disabled {
  cursor: not-allowed;
  opacity: 0.34;
}

.btn.primary {
  border-color: rgba(105, 186, 245, 0.36);
  color: #d8f0ff;
  background:
    linear-gradient(135deg, rgba(105, 186, 245, 0.28), rgba(43, 151, 168, 0.18));
  box-shadow: 0 14px 34px rgba(82, 169, 235, 0.16);
}

.btn.success {
  color: var(--color-green);
}

.btn.danger {
  color: var(--color-red);
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 1px;
  overflow: hidden;
  border-radius: 18px;
}

.metric {
  min-width: 0;
  padding: 12px;
  background: rgba(255, 255, 255, 0.018);
}

.metric span {
  display: block;
  color: var(--color-muted);
  font-size: 10px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.metric strong {
  color: var(--color-text);
  font-family: var(--font-mono);
  font-size: 24px;
}

.project-table {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 16px;
  border-radius: 22px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-auto-rows: auto;
  gap: 16px;
  align-content: start;
}

.project-row {
  position: relative;
  width: 100%;
  min-height: 236px;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto;
  grid-template-areas:
    "main"
    "meta";
  gap: 12px;
  align-items: start;
  align-content: start;
  padding: 16px 16px 76px;
  border: 1px solid rgba(190, 224, 255, 0.09);
  border-radius: 16px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.052), rgba(255, 255, 255, 0.022)),
    rgba(13, 12, 14, 0.36);
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition: 0.16s ease;
}

.project-row:hover,
.project-row.selected {
  border-color: rgba(105, 186, 245, 0.3);
  background:
    linear-gradient(180deg, rgba(105, 186, 245, 0.08), rgba(255, 255, 255, 0.026)),
    rgba(17, 14, 16, 0.5);
}

.project-row.selected {
  box-shadow:
    inset 0 0 0 1px rgba(190, 224, 255, 0.06),
    0 18px 48px rgba(82, 169, 235, 0.12);
}

.status-rail {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 10px;
  height: 10px;
  min-height: 0;
  border-radius: 999px;
  background: var(--color-muted);
  box-shadow: 0 0 0 3px rgba(112, 135, 150, 0.12);
}

.project-row.running .status-rail {
  background: var(--color-green);
  box-shadow:
    0 0 0 4px rgba(124, 226, 188, 0.12),
    0 0 18px rgba(124, 226, 188, 0.52);
}

.project-row.error .status-rail {
  background: var(--color-red);
  box-shadow:
    0 0 0 4px rgba(255, 109, 130, 0.12),
    0 0 18px rgba(255, 109, 130, 0.42);
}

.project-main,
.project-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.project-main {
  grid-area: main;
  justify-content: flex-start;
}

.project-meta {
  grid-area: meta;
  overflow: visible;
}

.project-name {
  overflow: hidden;
  color: var(--color-text);
  font-size: 14px;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-path,
.command {
  overflow: hidden;
  color: var(--color-muted);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.port {
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
}

.service-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  overflow: visible;
}

.service-item {
  gap: 8px;
  display: flex;
  align-content: center;
  justify-content: space-between;
  min-height: 30px;
  border: 1px solid rgba(190, 224, 255, 0.075);
  border-radius: 12px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.038), rgba(255, 255, 255, 0.016)),
    rgba(10, 9, 11, 0.24);
  padding: 4px 7px;
  cursor: pointer;
  transition: border-color 0.18s ease, background 0.18s ease, box-shadow 0.18s ease;
}
.service-info{
  display: flex;
  align-items: center;
  gap: 6px;
}
.service-item:hover,
.service-item:focus-visible {
  border-color: rgba(190, 224, 255, 0.18);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0.018)),
    rgba(10, 9, 11, 0.34);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.025);
  outline: none;
}

.service-item:has(.service-state.running) {
  border-color: rgba(131, 230, 177, 0.16);
  background:
    linear-gradient(90deg, rgba(131, 230, 177, 0.075), transparent 42%),
    rgba(10, 9, 11, 0.26);
}

.service-item:has(.service-state.starting) {
  border-color: rgba(105, 186, 245, 0.22);
  background:
    linear-gradient(90deg, rgba(105, 186, 245, 0.07), transparent 42%),
    rgba(10, 9, 11, 0.26);
}

.service-kind {
  display: inline-grid;
  place-items: center;
  min-width: 0;
  width: 40px;
  height: 22px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 999px;
  color: var(--color-text-secondary);
  font-size: 10px;
  font-weight: 900;
  letter-spacing: 0.02em;
}

.service-kind.web {
  color: #d8f0ff;
  border-color: rgba(105, 186, 245, 0.28);
  background: rgba(105, 186, 245, 0.08);
}

.service-kind.api {
  color: #c3f5f8;
  border-color: rgba(158, 220, 227, 0.26);
  background: rgba(158, 220, 227, 0.06);
}

.service-kind.build {
  color: #cdbbff;
  border-color: rgba(185, 156, 255, 0.24);
  background: rgba(185, 156, 255, 0.06);
}

.service-state {
  flex-shrink: 0;
  min-width: 0;
  overflow: hidden;
  color: var(--color-muted);
  font-size: 10px;
  font-weight: 900;
  letter-spacing: 0.03em;
  text-transform: uppercase;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.service-state.running {
  color: var(--color-green);
  text-shadow: 0 0 14px rgba(131, 230, 177, 0.24);
}

.service-state.starting {
  color: var(--color-primary);
  text-shadow: 0 0 14px rgba(105, 186, 245, 0.24);
}

.service-state.error {
  color: var(--color-red);
}

.target-port {
  justify-self: end;
  min-width: 0;
  max-width: 84px;
  overflow: hidden;
  border: 1px solid rgba(131, 230, 177, 0.2);
  border-radius: 999px;
  background: rgba(131, 230, 177, 0.075);
  color: var(--color-green);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 800;
  padding: 1px 7px;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: 0 0 18px rgba(131, 230, 177, 0.08);
}

.target-port:empty {
  display: none;
}

.row-actions {
  grid-area: auto;
  position: absolute;
  right: 16px;
  bottom: 16px;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}

.target-control {
  display: flex;
  align-items: center;
  min-height: 34px;
}

.target-action {
  position: relative;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text-secondary);
  font-size: 13px;
  font-weight: 900;
  cursor: pointer;
}

.target-action.web {
  color: #d8f0ff;
  border-color: rgba(190, 224, 255, 0.12);
  background: rgba(255, 255, 255, 0.04);
}

.target-action.api {
  color: #c3f5f8;
  border-color: rgba(190, 224, 255, 0.12);
  background: rgba(255, 255, 255, 0.04);
}

.target-action.build {
  color: #cdbbff;
  border-color: rgba(190, 224, 255, 0.12);
  background: rgba(255, 255, 255, 0.04);
}

.target-action.running {
  color: var(--color-red);
  border-color: rgba(255, 109, 130, 0.24);
  background: rgba(255, 109, 130, 0.08);
}

.target-action:hover:not(:disabled) {
  border-color: rgba(105, 186, 245, 0.35);
  background: rgba(105, 186, 245, 0.08);
}

.target-action:disabled {
  cursor: not-allowed;
  opacity: 0.36;
}

.target-action.start {
  box-shadow: inset 0 -1px 0 rgba(255, 255, 255, 0.05);
}

.icon-action {
  position: relative;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(190, 224, 255, 0.1);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.038);
  color: var(--color-text-secondary);
  cursor: pointer;
}

.icon-action::after,
.target-action::after {
  position: absolute;
  right: 50%;
  bottom: calc(100% + 8px);
  z-index: 20;
  content: attr(data-tooltip);
  width: max-content;
  max-width: 260px;
  padding: 7px 9px;
  border: 1px solid rgba(190, 224, 255, 0.14);
  border-radius: 10px;
  background: rgba(16, 13, 15, 0.96);
  color: var(--color-text);
  font-size: 11px;
  line-height: 1.35;
  opacity: 0;
  pointer-events: none;
  transform: translate(50%, 4px);
  transition: 0.14s ease;
  box-shadow: 0 12px 34px rgba(0, 0, 0, 0.32);
}

.icon-action:hover::after,
.target-action:hover::after {
  opacity: 1;
  transform: translate(50%, 0);
}

.icon-action:hover:not(:disabled) {
  border-color: rgba(105, 186, 245, 0.35);
  color: var(--color-primary);
  background: rgba(105, 186, 245, 0.08);
}

.icon-action:disabled {
  cursor: not-allowed;
  opacity: 0.3;
}

.icon-action.start {
  color: var(--color-green);
}

.icon-action.danger {
  color: var(--color-red);
}

.drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: grid;
  place-items: center;
  border: 2px dashed rgba(105, 186, 245, 0.46);
  border-radius: 24px;
  background: rgba(12, 10, 12, 0.78);
  pointer-events: none;
}

.drop-hint {
  border: 1px solid rgba(190, 224, 255, 0.16);
  border-radius: 16px;
  background: rgba(20, 16, 18, 0.94);
  color: #d8f0ff;
  padding: 14px 22px;
  font-weight: 800;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  border-radius: 22px;
  color: var(--color-muted);
  text-align: center;
}

.empty-state h3 {
  color: var(--color-text);
}

.empty-orbit {
  width: 72px;
  height: 72px;
  border: 1px solid rgba(105, 186, 245, 0.28);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  box-shadow: 0 0 46px rgba(82, 169, 235, 0.18);
}

.empty-inline {
  padding: 24px;
  color: var(--color-muted);
  text-align: center;
}

@container (min-width: 760px) {
  .project-table {
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  }

  .project-row {
    padding: 18px 18px 78px;
  }
}

@container (max-width: 520px) {
  .command-strip {
    grid-template-columns: 1fr;
  }

  .ops-bar {
    flex-wrap: wrap;
  }

  .search-input {
    flex-basis: 100%;
  }

  .project-table {
    padding: 12px;
    gap: 12px;
  }

  .project-row {
    min-height: 210px;
    gap: 10px;
    padding: 12px 12px 58px;
  }

  .service-item {
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 4px;
  }

  .service-state {
    text-align: right;
  }

  .target-port {
    grid-column: 1 / -1;
    justify-self: stretch;
    max-width: none;
    text-align: center;
  }

  .target-action,
  .icon-action {
    width: 28px;
    height: 28px;
  }

  .row-actions {
    right: 18px;
    bottom: 18px;
    gap: 6px;
  }
}
</style>
