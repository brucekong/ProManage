<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NModal } from "naive-ui";
import { useProjectStore } from "../stores/project";
import ProjectFormModal from "../components/ProjectFormModal.vue";
import type { ProjectConfig } from "../types/project";
import { detectPortsFromOutput } from "../utils/ports";
import { openProjectInIde } from "../api/commands";

const store = useProjectStore();
const query = ref("");
const dragOver = ref(false);
const draggedProjectId = ref<string | null>(null);
const dragTargetProjectId = ref<string | null>(null);
const orderChangedByDrag = ref(false);
const showForm = ref(false);
const editingProject = ref<ProjectConfig | null>(null);
const pendingDeleteProject = ref<ProjectConfig | null>(null);

const filteredProjects = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return store.projects;
  return store.projects.filter((project) =>
    [project.name, project.path, project.command, project.group, project.note]
      .filter(Boolean)
      .some((value) => value.toLowerCase().includes(keyword))
  );
});

function compactBranchName(branch: string) {
  if (branch.length <= 28) return branch;

  const parts = branch.split("/");
  if (parts.length > 1) {
    const head = parts[0];
    const tail = parts[parts.length - 1];
    const tailLimit = Math.max(10, 26 - head.length);
    const compactTail = tail.length > tailLimit ? `${tail.slice(0, tailLimit - 3)}...` : tail;
    return `${head}/${compactTail}`;
  }

  return `${branch.slice(0, 25)}...`;
}

const gitBadges = computed(() => {
  const badges: Record<string, { label: string; title: string; dirty: boolean }> = {};
  for (const project of store.projects) {
    const status = store.gitStatuses?.[project.id];
    if (!status?.has_git || !status.branch) continue;

    const ahead = Number(status.ahead) || 0;
    const behind = Number(status.behind) || 0;
    const sync = [
      ahead > 0 ? `+${ahead}` : "",
      behind > 0 ? `-${behind}` : "",
    ].filter(Boolean).join(" ");
    const dirty = Boolean(status.dirty);
    const label = [compactBranchName(status.branch), dirty ? "*" : "", sync].filter(Boolean).join(" ");
    const title = [status.branch, dirty ? "dirty" : "", sync].filter(Boolean).join(" ");
    if (label) {
      badges[project.id] = { label, title, dirty };
    }
  }
  return badges;
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

function targetStatus(targetId: string) {
  return statusLabel(targetId).toLowerCase();
}

function targetPortLabel(targetId: string) {
  const ports = detectedPorts(targetId);
  return ports.length ? ports.map((port) => `:${port}`).join(", ") : "auto";
}

function projectStatus(project: ProjectConfig) {
  const statuses = runTargets(project).map((target) => statusLabel(target.id));
  if (statuses.includes("Error")) return "Error";
  if (statuses.includes("Running")) return "Running";
  if (statuses.includes("Starting")) return "Starting";
  return "Stopped";
}

function targetGroup(target: ReturnType<typeof runTargets>[number]) {
  if (target.kind === "web") return "frontend";
  if (target.kind === "api") return "backend";

  const key = `${target.name} ${target.label} ${target.command}`.toLowerCase();
  if (/(^|[:\s-])(web|fe|front|client)([:\s-]|$)/.test(key)) return "frontend";
  if (/(^|[:\s-])(api|be|back|server)([:\s-]|$)/.test(key)) return "backend";
  return "task";
}

function groupTargets(project: ProjectConfig, group: "frontend" | "backend" | "task") {
  return runTargets(project).filter((target) => targetGroup(target) === group);
}

function hasRunTargets(project: ProjectConfig) {
  return runTargets(project).length > 0;
}

function targetScriptName(target: ReturnType<typeof runTargets>[number]) {
  const parts = target.name.split(":");
  return parts[parts.length - 1] || target.name;
}

function targetSourceName(target: ReturnType<typeof runTargets>[number]) {
  const parts = target.name.split(":");
  if (parts.length > 1) return parts.slice(0, -1).join(":");
  return target.label;
}

function isWorkspaceProject(project: ProjectConfig) {
  return project.project_kind === "workspace";
}

function isWorkspacePath(path: string) {
  return /\.(code-|ag)?workspace$/i.test(path.trim());
}

function workspaceName(path: string) {
  const filename = path.split("/").pop() || path;
  return filename.replace(/\.(code-|ag)?workspace$/i, "");
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
    if (isWorkspaceProject(project)) {
      await revealItemInDir(project.path);
      return;
    }

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

function ideCommand(kind: "vscode" | "antigravity") {
  if (kind === "vscode") {
    return store.config.ide_vscode_command.trim() || "code";
  }
  return store.config.ide_antigravity_command.trim() || "ag";
}

async function openProjectInConfiguredIde(project: ProjectConfig, kind: "vscode" | "antigravity") {
  const command = ideCommand(kind);
  if (!command) {
    alert(`${kind === "vscode" ? "VS Code" : "Antigravity"} command is not configured yet.`);
    return;
  }

  try {
    await openProjectInIde(project.path, command);
  } catch (e) {
    console.error(`Open project in ${kind} failed:`, e);
    alert(`Unable to open ${project.name} in ${kind === "vscode" ? "VS Code" : "Antigravity"}.`);
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

function onProjectDragStart(e: DragEvent, project: ProjectConfig) {
  draggedProjectId.value = project.id;
  dragTargetProjectId.value = null;
  orderChangedByDrag.value = false;
  if (e.dataTransfer) {
    e.dataTransfer.setData("text/plain", project.id);
    e.dataTransfer.setData("application/x-prostation-project", project.id);
    e.dataTransfer.effectAllowed = "move";
  }
}

function onProjectDragOver(e: DragEvent, project: ProjectConfig) {
  e.preventDefault();
  e.stopPropagation();
  const sourceId = draggedProjectId.value;
  if (!sourceId || sourceId === project.id) return;

  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const xFromCenter = e.clientX - (rect.left + rect.width / 2);
  const yFromCenter = e.clientY - (rect.top + rect.height / 2);
  const placement = Math.abs(xFromCenter) > Math.abs(yFromCenter)
    ? (xFromCenter > 0 ? "after" : "before")
    : (yFromCenter > 0 ? "after" : "before");

  dragTargetProjectId.value = project.id;
  store.moveProject(sourceId, project.id, placement);
  orderChangedByDrag.value = true;
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = "move";
  }
}

async function onProjectDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (e.dataTransfer?.files?.length) return;

  try {
    if (orderChangedByDrag.value) {
      await store.saveProjectOrder();
    }
  } catch (error) {
    console.error("Reorder projects failed:", error);
  } finally {
    draggedProjectId.value = null;
    dragTargetProjectId.value = null;
    orderChangedByDrag.value = false;
  }
}

async function onProjectDragEnd() {
  if (orderChangedByDrag.value) {
    try {
      await store.saveProjectOrder();
    } catch (error) {
      console.error("Save project order failed:", error);
    }
  }
  draggedProjectId.value = null;
  dragTargetProjectId.value = null;
  orderChangedByDrag.value = false;
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

function handleDelete(project: ProjectConfig) {
  pendingDeleteProject.value = project;
}

function cancelDelete() {
  pendingDeleteProject.value = null;
}

async function confirmDelete() {
  const project = pendingDeleteProject.value;
  if (!project) return;
  await store.removeProject(project.id);
  pendingDeleteProject.value = null;
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  dragOver.value = Array.from(e.dataTransfer?.types || []).includes("Files");
}

function onDragLeave() {
  dragOver.value = false;
}

async function onDrop(e: DragEvent) {
  e.preventDefault();
  dragOver.value = false;

  const files = e.dataTransfer?.files;
  if (!files || files.length === 0) return;

  for (let i = 0; i < files.length; i++) {
    const path = (files[i] as any).path || files[i].name;
    if (path) {
      try {
        if (isWorkspacePath(path)) {
          await store.addProject({
            id: "",
            name: workspaceName(path),
            path,
            project_kind: "workspace",
            command: "",
            scripts: [],
            port: 0,
            group: "default",
            note: "",
            auto_start: false,
            show_build_scripts: false,
            depends_on: [],
            env_vars: [],
          });
        } else {
          await store.scanDirectory(path);
        }
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
      <div class="drop-hint">Drop folders or workspace files to add projects</div>
    </div>

    <section v-if="store.projects.length > 0" class="project-table">
      <div
        v-for="project in filteredProjects"
        :key="project.id"
        class="project-row"
        role="button"
        tabindex="0"
        draggable="true"
        :class="{
          selected: store.selectedProjectId === project.id,
          dragging: draggedProjectId === project.id,
          'drag-over-card': dragTargetProjectId === project.id,
          'has-run-targets': hasRunTargets(project),
          running: runTargets(project).some(target => ['Starting', 'Running'].includes(statusLabel(target.id))),
          error: runTargets(project).some(target => statusLabel(target.id) === 'Error')
        }"
        @dragstart.stop="onProjectDragStart($event, project)"
        @dragenter.prevent.stop="onProjectDragOver($event, project)"
        @dragover.prevent.stop="onProjectDragOver($event, project)"
        @drop.prevent.stop="onProjectDrop($event)"
        @dragend="onProjectDragEnd"
        @click="selectProjectCard(project)"
        @keyup.enter="selectProjectCard(project)"
      >
        <span v-if="hasRunTargets(project)" class="status-rail"></span>
        <span class="project-main" :title="project.path">
          <span class="project-title-block">
            <span class="project-name">
              <span class="project-title-text">{{ project.name }}</span>
              <span class="title-actions">
                <button class="title-action" data-tooltip="Edit project" @click.stop="openEditModal(project)">
                  ✎
                </button>
                <button class="title-action danger" data-tooltip="Remove project" @click.stop="handleDelete(project)">
                  ×
                </button>
              </span>
            </span>
            <span
              v-if="isWorkspaceProject(project) || gitBadges[project.id]"
              class="project-submeta"
            >
              <span v-if="isWorkspaceProject(project)" class="workspace-chip">workspace</span>
              <span
                v-if="gitBadges[project.id]"
                class="git-chip"
                :class="{ dirty: gitBadges[project.id].dirty }"
                :title="gitBadges[project.id].title"
              >
                {{ gitBadges[project.id].label }}
              </span>
            </span>
          </span>
          <span
            v-if="hasRunTargets(project)"
            class="project-status-chip"
            :class="projectStatus(project).toLowerCase()"
          >
            {{ projectStatus(project) }}
          </span>
        </span>
        <span class="project-meta">
          <div class="service-stack">
            <template v-for="group in (['frontend', 'backend', 'task'] as const)" :key="group">
              <section
                v-if="groupTargets(project, group).length"
                class="service-section"
                :class="group"
              >
                <span class="service-section-title">{{ group }}</span>
                <div
                  v-for="target in groupTargets(project, group)"
                  :key="target.id"
                  class="service-row"
                  :class="targetStatus(target.id)"
                  role="button"
                  tabindex="0"
                  @click.stop="selectTarget(project, target)"
                  @keyup.enter.stop="selectTarget(project, target)"
                >
                  <span class="service-accent"></span>
                  <span class="service-summary">
                    <strong>{{ targetSourceName(target) }}</strong>
                    <span>{{ targetScriptName(target) }}</span>
                  </span>
                  <span class="service-port">{{ targetPortLabel(target.id) }}</span>
                  <span class="service-state-dot" :class="targetStatus(target.id)"></span>
                  <button
                    class="inline-run"
                    :class="{ running: ['Starting', 'Running'].includes(statusLabel(target.id)) }"
                    :data-tooltip="['Starting', 'Running'].includes(statusLabel(target.id))
                      ? `Stop ${target.label}`
                      : `Start ${target.label}: ${target.command}`"
                    @click.stop="['Starting', 'Running'].includes(statusLabel(target.id))
                      ? stopTarget(target.id)
                      : startTarget(project, target)"
                  >
                    {{ ['Starting', 'Running'].includes(statusLabel(target.id)) ? '■' : targetIcon() }}
                  </button>
                </div>
              </section>
            </template>
          </div>
        </span>
        <span class="row-actions">
          <button
            class="icon-action"
            :data-tooltip="isWorkspaceProject(project) ? 'Reveal workspace file' : 'Open folder'"
            @click.stop="openProjectFolder(project)"
          >
            📁
          </button>
          <button
            v-if="ideCommand('vscode')"
            class="icon-action ide"
            data-tooltip="Open in VS Code"
            @click.stop="openProjectInConfiguredIde(project, 'vscode')"
          >
            VS
          </button>
          <button
            v-if="ideCommand('antigravity')"
            class="icon-action ide"
            data-tooltip="Open in Antigravity"
            @click.stop="openProjectInConfiguredIde(project, 'antigravity')"
          >
            AG
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

    <NModal
      :show="Boolean(pendingDeleteProject)"
      preset="card"
      title="Remove Project"
      style="width: 420px; max-width: 90vw;"
      :bordered="false"
      :closable="true"
      @update:show="(val: boolean) => { if (!val) cancelDelete(); }"
    >
      <div class="delete-confirm">
        <p>
          Remove
          <strong>{{ pendingDeleteProject?.name }}</strong>
          from ProStation?
        </p>
        <span>This only removes the project entry. It does not delete local files.</span>
      </div>
      <template #footer>
        <div class="confirm-actions">
          <NButton @click="cancelDelete">Cancel</NButton>
          <NButton type="error" @click="confirmDelete">Remove</NButton>
        </div>
      </template>
    </NModal>
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
  padding: 18px;
  border-radius: 22px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-auto-rows: auto;
  gap: 14px;
  align-content: start;
}

.project-row {
  position: relative;
  width: 100%;
  min-height: 176px;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto;
  grid-template-areas:
    "main"
    "meta";
  gap: 12px;
  align-items: start;
  align-content: start;
  padding: 16px 16px 54px;
  border: 1px solid rgba(190, 224, 255, 0.09);
  border-radius: 15px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.052), rgba(255, 255, 255, 0.022)),
    rgba(13, 12, 14, 0.36);
  color: inherit;
  text-align: left;
  cursor: grab;
  transition: 0.16s ease;
}

.project-row:active {
  cursor: grabbing;
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

.project-row.dragging {
  opacity: 0.46;
  transform: scale(0.985);
}

.project-row.drag-over-card {
  border-color: rgba(124, 226, 188, 0.48);
  box-shadow:
    inset 0 0 0 1px rgba(124, 226, 188, 0.22),
    0 18px 48px rgba(124, 226, 188, 0.12);
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
  display: block;
}

.project-row.has-run-targets .project-main {
  padding-right: 112px;
}

.project-meta {
  grid-area: meta;
  overflow: visible;
}

.project-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--color-text);
  font-size: 16px;
  font-weight: 850;
  line-height: 1.2;
}

.project-title-block {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.project-submeta {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
}

.title-actions {
  flex: 0 0 auto;
  display: flex;
  gap: 4px;
  opacity: 0;
  transform: translateX(-4px);
  transition: opacity 0.14s ease, transform 0.14s ease;
}

.project-row:hover .title-actions,
.project-row:focus-within .title-actions {
  opacity: 1;
  transform: translateX(0);
}

.title-action {
  position: relative;
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(190, 224, 255, 0.1);
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--color-muted);
  font-size: 11px;
  font-weight: 900;
  cursor: pointer;
}

.title-action:hover {
  border-color: rgba(105, 186, 245, 0.34);
  color: var(--color-primary);
  background: rgba(105, 186, 245, 0.08);
}

.title-action.danger:hover {
  border-color: rgba(255, 109, 130, 0.28);
  color: var(--color-red);
  background: rgba(255, 109, 130, 0.08);
}

.project-status-chip {
  position: absolute;
  top: 12px;
  right: 34px;
  max-width: 84px;
  overflow: hidden;
  padding: 3px 7px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 999px;
  color: var(--color-muted);
  font-size: 9px;
  font-weight: 900;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.project-status-chip.running {
  border-color: rgba(124, 226, 188, 0.28);
  color: var(--color-green);
  background: rgba(124, 226, 188, 0.07);
}

.project-status-chip.starting {
  border-color: rgba(105, 186, 245, 0.28);
  color: var(--color-primary);
  background: rgba(105, 186, 245, 0.07);
}

.project-status-chip.error {
  border-color: rgba(255, 109, 130, 0.28);
  color: var(--color-red);
  background: rgba(255, 109, 130, 0.07);
}

.project-title-text {
  min-width: 0;
  flex: 0 1 auto;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command {
  overflow: hidden;
  color: var(--color-muted);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workspace-chip {
  flex: 0 0 auto;
  display: inline-grid;
  place-items: center;
  padding: 2px 6px;
  border: 1px solid rgba(134, 217, 233, 0.24);
  border-radius: 6px;
  color: #9ddfeb;
  font-family: var(--font-sans);
  font-size: 9px;
  font-weight: 800;
  text-transform: uppercase;
}

.git-chip {
  min-width: 0;
  flex: 0 1 auto;
  max-width: min(220px, 100%);
  display: inline-block;
  overflow: hidden;
  padding: 3px 8px;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 999px;
  color: #b7c7d3;
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 800;
  line-height: 1.1;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: rgba(190, 224, 255, 0.04);
}

.git-chip.dirty {
  border-color: rgba(244, 202, 105, 0.24);
  color: #f0cf83;
  background: rgba(244, 202, 105, 0.06);
}

.port {
  color: var(--color-primary);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
}

.service-stack {
  display: flex;
  flex-direction: column;
  gap: 9px;
  min-width: 0;
}

.service-section {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.service-section-title {
  color: #88a2b3;
  font-size: 9px;
  font-weight: 900;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.service-row {
  min-width: 0;
  display: grid;
  grid-template-columns: 3px minmax(0, 1fr) auto 8px 28px;
  gap: 8px;
  align-items: center;
  min-height: 38px;
  padding: 6px 7px;
  border: 1px solid rgba(190, 224, 255, 0.1);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.028);
  cursor: pointer;
}

.service-row:hover,
.service-row:focus-visible {
  border-color: rgba(190, 224, 255, 0.2);
  background: rgba(255, 255, 255, 0.045);
  outline: none;
}

.service-accent {
  width: 3px;
  height: 22px;
  border-radius: 999px;
  background: rgba(105, 186, 245, 0.55);
}

.backend .service-accent {
  background: rgba(158, 220, 227, 0.56);
}

.task .service-accent {
  background: rgba(190, 224, 255, 0.3);
}

.service-summary {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 7px;
}

.service-summary strong,
.service-summary span,
.service-port {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.service-summary strong {
  color: var(--color-text);
  font-size: 11px;
  font-weight: 850;
}

.service-summary span {
  color: #8fa3b2;
  font-family: var(--font-mono);
  font-size: 10px;
}

.service-port {
  max-width: 54px;
  color: #9bdcc7;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 800;
}

.service-state-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: rgba(112, 135, 150, 0.72);
}

.service-state-dot.running {
  background: var(--color-green);
  box-shadow: 0 0 12px rgba(124, 226, 188, 0.42);
}

.service-state-dot.starting {
  background: var(--color-primary);
  box-shadow: 0 0 12px rgba(105, 186, 245, 0.42);
}

.service-state-dot.error {
  background: var(--color-red);
}

.inline-run {
  position: relative;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(190, 224, 255, 0.12);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.035);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 900;
  cursor: pointer;
}

.inline-run.running {
  color: var(--color-red);
  border-color: rgba(255, 109, 130, 0.24);
  background: rgba(255, 109, 130, 0.08);
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
.title-action::after,
.inline-run::after {
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
.title-action:hover::after,
.inline-run:hover::after {
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

.delete-confirm {
  display: flex;
  flex-direction: column;
  gap: 8px;
  color: var(--color-text-secondary);
  font-size: 13px;
  line-height: 1.6;
}

.delete-confirm p {
  margin: 0;
  color: var(--color-text);
}

.delete-confirm strong {
  color: #d8f0ff;
}

.delete-confirm span {
  color: var(--color-muted);
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
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
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  }

  .project-row {
    padding: 16px 16px 58px;
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
    padding: 12px 12px 54px;
  }

  .icon-action {
    width: 28px;
    height: 28px;
  }

  .inline-run {
    width: 26px;
    height: 26px;
  }

  .row-actions {
    right: 18px;
    bottom: 18px;
    gap: 6px;
  }
}
</style>
