<script setup lang="ts">
import { ref, watch, reactive, computed } from "vue";
import { NSelect, NModal, NButton, NInput, NInputNumber, NCheckbox, NSpace } from "naive-ui";
import type { ProjectConfig } from "../types/project";
import * as api from "../api/commands";
import { isTauriRuntime, nativeOnlyMessage } from "../utils/runtime";

const props = defineProps<{
  show: boolean;
  project?: ProjectConfig | null;
  otherProjects: ProjectConfig[];
}>();

const emit = defineEmits<{
  close: [];
  saved: [project: ProjectConfig];
}>();

const emptyForm = () => ({
  id: "",
  name: "",
  path: "",
  project_kind: "folder" as ProjectConfig["project_kind"],
  command: "npm run dev",
  scripts: [] as [string, string][],
  has_custom_command: false,
  port: 0,
  group: "default",
  note: "",
  auto_start: false,
  show_build_scripts: false,
  depends_on: [] as string[],
  env_vars: [] as [string, string][],
});

const form = reactive<Omit<ProjectConfig, "id"> & { id: string }>({ ...emptyForm() });
const isEditing = ref(false);
const pickerHint = ref("");
const scriptsLoading = ref(false);

watch(
  () => props.show,
  (val) => {
    if (val) {
      pickerHint.value = "";
      if (props.project) {
        Object.assign(form, { ...emptyForm(), ...props.project });
        isEditing.value = true;
      } else {
        Object.assign(form, emptyForm());
        isEditing.value = false;
      }
    }
  }
);

// Dependencies that can be selected (other projects minus self)
const availableDeps = ref<ProjectConfig[]>([]);

watch(
  () => props.otherProjects,
  (others) => {
    if (props.project) {
      availableDeps.value = others.filter((p) => p.id !== props.project!.id);
    } else {
      availableDeps.value = others;
    }
  },
  { immediate: true }
);

function toggleDep(depId: string) {
  const idx = form.depends_on.indexOf(depId);
  if (idx >= 0) {
    form.depends_on.splice(idx, 1);
  } else {
    form.depends_on.push(depId);
  }
}

function isLaunchCommand(command: string) {
  const trimmed = command.trimStart();
  return /^(npm|pnpm|yarn|bun)\s/.test(trimmed) || trimmed.startsWith("cd ");
}

function scriptToCommand(scriptName: string, scriptCommand = "") {
  if (isLaunchCommand(scriptCommand)) return scriptCommand;
  return scriptName === "start" ? "npm start" : `npm run ${scriptName}`;
}

// Filter out db/migration/seed scripts; keep only service startup scripts
const excludedPatterns = /^(db:|migrate|seed|studio|deploy|generate|reset|setup|postinstall|lint|test|format|check|typecheck|prepare)/i;

function isStartupScript(name: string) {
  if (excludedPatterns.test(name)) return false;
  if (name.includes(":db:") || name.includes(":migrate") || name.includes(":seed") || name.includes(":studio")) return false;
  const parts = name.split(":");
  const last = parts[parts.length - 1];
  if (excludedPatterns.test(last)) return false;
  return true;
}

function scriptSortKey(name: string): number {
  const lower = name.toLowerCase();
  if (lower.includes("web") || lower.includes("fe") || lower.includes("client") || lower.includes("front")) return 0;
  if (lower.includes("api") || lower.includes("server") || lower.includes("backend")) return 1;
  if (lower === "dev" || lower === "start" || lower === "serve") return 2;
  if (lower.includes("build")) return 3;
  return 4;
}

const scriptOptions = computed(() =>
  (form.scripts ?? [])
    .filter(([name]) => isStartupScript(name))
    .sort(([a], [b]) => scriptSortKey(a) - scriptSortKey(b) || a.localeCompare(b))
    .map(([name, command]) => ({
      label: `${name} – ${command}`,
      value: scriptToCommand(name, command),
    }))
);

const isWorkspaceProject = computed(() => form.project_kind === "workspace");

function inferCommandFromScripts(scripts: [string, string][]) {
  const names = scripts.map(([name]) => name);
  const priority = [
    "web:dev",
    "api:dev",
    "web:start",
    "api:start",
    "dev",
    "start",
    "serve",
    "dev:web",
    "dev:app",
    "dev:api",
    "start:web",
    "start:api",
  ];
  const preferred =
    priority.find((name) => names.includes(name)) ||
    names.find((name) => name.startsWith("dev:") || name.includes("dev")) ||
    names.find((name) => name.startsWith("start:") || name.includes("start")) ||
    names.find((name) => name.includes("build")) ||
    names[0];

  const script = scripts.find(([name]) => name === preferred);
  return script ? scriptToCommand(script[0], script[1]) : "npm run dev";
}

async function loadScriptsFromPackageJson() {
  pickerHint.value = "";
  if (isWorkspaceProject.value) {
    form.scripts = [];
    form.command = "";
    return;
  }
  if (!form.path.trim()) return;
  if (!isTauriRuntime()) {
    pickerHint.value = nativeOnlyMessage;
    return;
  }

  scriptsLoading.value = true;
  try {
    const scripts = await api.readPackageScripts(form.path.trim());
    form.scripts = scripts;
    if (scripts.length > 0) {
      form.command = inferCommandFromScripts(scripts);
    } else {
      pickerHint.value = "No scripts found in package.json.";
    }
  } catch (e) {
    console.error("Read package scripts failed:", e);
    pickerHint.value = "Failed to read scripts from package.json.";
  } finally {
    scriptsLoading.value = false;
  }
}

async function pickFolder() {
  pickerHint.value = "";
  if (!isTauriRuntime()) {
    pickerHint.value = nativeOnlyMessage;
    return;
  }

  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select project directory",
    });
    if (selected) {
      form.project_kind = "folder";
      form.path = selected;
      if (!form.name) {
        const parts = selected.replace(/\/$/, "").split("/");
        form.name = parts[parts.length - 1];
      }
      await loadScriptsFromPackageJson();
    }
  } catch (e) {
    console.error("Folder picker failed:", e);
    pickerHint.value = "Failed to open the native folder picker.";
  }
}

async function pickWorkspaceFile() {
  pickerHint.value = "";
  if (!isTauriRuntime()) {
    pickerHint.value = nativeOnlyMessage;
    return;
  }

  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      directory: false,
      multiple: false,
      title: "Select workspace file",
      filters: [
        {
          name: "Workspace",
          extensions: ["code-workspace", "workspace", "agworkspace"],
        },
      ],
    });
    if (selected) {
      form.project_kind = "workspace";
      form.path = selected;
      form.command = "";
      form.scripts = [];
      if (!form.name) {
        const filename = selected.split("/").pop() || selected;
        form.name = filename.replace(/\.(code-|ag)?workspace$/i, "");
      }
    }
  } catch (e) {
    console.error("Workspace picker failed:", e);
    pickerHint.value = "Failed to open the native file picker.";
  }
}

function inferKindFromPath(path: string): ProjectConfig["project_kind"] {
  return /\.(code-|ag)?workspace$/i.test(path.trim()) ? "workspace" : "folder";
}

function submit() {
  if (!form.name.trim() || !form.path.trim()) return;
  const projectKind = form.project_kind === "workspace" ? "workspace" : inferKindFromPath(form.path);
  emit("saved", {
    ...form,
    project_kind: projectKind,
    command: projectKind === "workspace" ? "" : form.command,
    scripts: projectKind === "workspace" ? [] : form.scripts,
    has_custom_command: projectKind !== "workspace" && (form.scripts?.length ?? 0) === 0 && form.command.trim().length > 0,
  });
}

function close() {
  emit("close");
}
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    :title="isEditing ? 'Edit Project' : 'Add Project'"
    style="width: 520px; max-width: 90vw;"
    :bordered="false"
    :closable="true"
    @update:show="(val: boolean) => { if (!val) close(); }"
  >
    <div class="modal-body">
      <div class="field">
        <label>Project Name</label>
        <NInput
          v-model:value="form.name"
          placeholder="e.g. my-admin-app"
        />
      </div>

      <div class="field">
        <label>Path</label>
        <div class="field-row">
          <NInput
            v-model:value="form.path"
            placeholder="/path/to/project or workspace file"
            class="flex-1"
          />
          <NButton @click="pickFolder">Browse</NButton>
          <NButton @click="pickWorkspaceFile">Workspace</NButton>
        </div>
        <p v-if="pickerHint" class="native-hint">{{ pickerHint }}</p>
      </div>

      <div v-if="!isWorkspaceProject" class="field">
        <label>Start Command</label>
        <div class="field-row">
          <NSelect
            v-if="form.scripts && form.scripts.length > 0"
            v-model:value="form.command"
            :options="scriptOptions"
            filterable
            class="flex-1"
          />
          <NInput
            v-else
            v-model:value="form.command"
            placeholder="npm run dev"
            class="flex-1"
          />
          <NButton
            :disabled="scriptsLoading || !form.path.trim()"
            @click="loadScriptsFromPackageJson"
          >
            {{ scriptsLoading ? "Loading" : "Load Scripts" }}
          </NButton>
        </div>
        <NInput
          v-if="form.scripts && form.scripts.length > 0"
          v-model:value="form.command"
          placeholder="Custom command"
          class="command-input"
          style="font-family: var(--font-mono); font-size: 12px;"
        />
      </div>

      <div class="field-row split">
        <div class="field flex-1">
          <label>Port (0 = auto)</label>
          <NInputNumber
            v-model:value="form.port"
            :min="0"
            :max="65535"
            style="width: 100%;"
          />
        </div>
        <div class="field flex-1">
          <label>Group</label>
          <NInput
            v-model:value="form.group"
            placeholder="default"
          />
        </div>
      </div>

      <!-- Dependencies -->
      <div class="field" v-if="availableDeps.length > 0">
        <label>Dependencies (start before this project)</label>
        <div class="dep-list">
          <label
            v-for="dep in availableDeps"
            :key="dep.id"
            class="dep-item"
            :class="{ checked: form.depends_on.includes(dep.id) }"
          >
            <NCheckbox
              :checked="form.depends_on.includes(dep.id)"
              @update:checked="toggleDep(dep.id)"
            />
            <span class="dep-name">{{ dep.name }}</span>
            <span class="dep-port" v-if="dep.port">:{{ dep.port }}</span>
          </label>
          <div v-if="form.depends_on.length > 0" class="dep-order-hint">
            Start order: {{ form.depends_on.map(id => availableDeps.find(d => d.id === id)?.name).filter(Boolean).join(' → ') }} → {{ form.name || 'this project' }}
          </div>
        </div>
      </div>

      <div class="field">
        <label>Note (optional)</label>
        <NInput
          v-model:value="form.note"
          placeholder="Any notes about this project"
        />
      </div>

      <label class="option-row">
        <NCheckbox v-model:checked="form.show_build_scripts" />
        <span>
          <strong>Show local build actions</strong>
          <small>Enable only for projects that need local build scripts in ProStation.</small>
        </span>
      </label>
    </div>

    <template #footer>
      <NSpace justify="end">
        <NButton @click="close">Cancel</NButton>
        <NButton
          type="primary"
          @click="submit"
          :disabled="!form.name.trim() || !form.path.trim()"
        >
          {{ isEditing ? "Save Changes" : "Add Project" }}
        </NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.modal-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 70vh;
  overflow-y: auto;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field label {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-muted);
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.field-row {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}

.field-row.split {
  gap: 16px;
}

.flex-1 {
  flex: 1;
}

.command-input {
  margin-top: 8px;
}

.native-hint {
  color: var(--color-orange);
  font-size: 12px;
  line-height: 1.4;
}

/* ── Dependencies ──────────────── */
.dep-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  background: var(--color-bg);
  border-radius: 8px;
  padding: 8px;
  max-height: 180px;
  overflow-y: auto;
}

.dep-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s;
  font-size: 13px;
}

.dep-item:hover {
  background: var(--color-hover);
}

.dep-item.checked {
  background: rgba(0, 122, 255, 0.06);
}

.dep-name {
  color: var(--color-text);
  font-weight: 500;
}

.dep-port {
  color: var(--color-muted);
  font-size: 11px;
}

.dep-order-hint {
  font-size: 11px;
  color: var(--color-primary);
  padding: 4px 8px;
  font-weight: 500;
}

.option-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.035);
  padding: 10px 12px;
  cursor: pointer;
}

.option-row span {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.option-row strong {
  color: var(--color-text);
  font-size: 13px;
}

.option-row small {
  color: var(--color-muted);
  font-size: 12px;
}
</style>
