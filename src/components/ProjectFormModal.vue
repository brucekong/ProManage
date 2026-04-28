<script setup lang="ts">
import { ref, watch, reactive, computed } from "vue";
import { NSelect, NModal, NButton, NInput, NInputNumber, NCheckbox, NSpace } from "naive-ui";
import type { ProjectConfig } from "../types/project";
import * as api from "../api/commands";
import { isTauriRuntime, nativeOnlyMessage } from "../utils/runtime";
import { useI18n } from "../i18n";

const props = defineProps<{
  show: boolean;
  project?: ProjectConfig | null;
  otherProjects: ProjectConfig[];
}>();

const { t } = useI18n();

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
  is_favorite: false,
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
      pickerHint.value = t("form.noScripts");
    }
  } catch (e) {
    console.error("Read package scripts failed:", e);
    pickerHint.value = t("form.readScriptsFailed");
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
      title: t("dialog.projectDirectory"),
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
    pickerHint.value = t("form.pickFolderFailed");
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
      title: t("dialog.workspaceFile"),
      filters: [
        {
          name: t("form.workspace"),
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
    pickerHint.value = t("form.pickWorkspaceFailed");
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
    class="glass-modal project-form-glass"
    :title="isEditing ? t('form.editTitle') : t('form.addTitle')"
    style="width: 720px; max-width: 92vw;"
    :bordered="false"
    :closable="true"
    @update:show="(val: boolean) => { if (!val) close(); }"
  >
    <div class="modal-body">
      <div class="field">
        <label>{{ t("form.name") }}</label>
        <NInput
          v-model:value="form.name"
          placeholder="e.g. my-admin-app"
        />
      </div>

      <div class="field">
        <label>{{ t("form.path") }}</label>
        <div class="field-row">
          <NInput
            v-model:value="form.path"
            :placeholder="t('form.pathPlaceholder')"
            class="flex-1"
          />
          <NButton @click="pickFolder">{{ t("form.browse") }}</NButton>
          <NButton @click="pickWorkspaceFile">{{ t("form.workspace") }}</NButton>
        </div>
        <p v-if="pickerHint" class="native-hint">{{ pickerHint }}</p>
      </div>

      <div v-if="!isWorkspaceProject" class="field">
        <label>{{ t("form.command") }}</label>
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
            {{ scriptsLoading ? t("form.loading") : t("form.loadScripts") }}
          </NButton>
        </div>
        <NInput
          v-if="form.scripts && form.scripts.length > 0"
          v-model:value="form.command"
          :placeholder="t('form.customCommand')"
          class="command-input"
          style="font-family: var(--font-mono); font-size: 12px;"
        />
      </div>

      <div class="field-row split">
        <div class="field flex-1">
          <label>{{ t("form.port") }}</label>
          <NInputNumber
            v-model:value="form.port"
            :min="0"
            :max="65535"
            style="width: 100%;"
          />
        </div>
        <div class="field flex-1">
          <label>{{ t("form.group") }}</label>
          <NInput
            v-model:value="form.group"
            placeholder="default"
          />
        </div>
      </div>

      <div class="field">
        <label>{{ t("form.note") }}</label>
        <NInput
          v-model:value="form.note"
          :placeholder="t('form.notePlaceholder')"
        />
      </div>

      <label class="option-row">
        <NCheckbox v-model:checked="form.show_build_scripts" />
        <span>
          <strong>{{ t("form.showBuild") }}</strong>
          <small>{{ t("form.showBuildDesc") }}</small>
        </span>
      </label>
    </div>

    <template #footer>
      <NSpace justify="end">
        <NButton @click="close">{{ t("common.cancel") }}</NButton>
        <NButton
          type="primary"
          @click="submit"
          :disabled="!form.name.trim() || !form.path.trim()"
        >
          {{ isEditing ? t("form.saveChanges") : t("project.addProject") }}
        </NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.modal-body {
  display: flex;
  flex-direction: column;
  gap: 18px;
  max-height: 70vh;
  overflow-y: auto;
  padding-right: 2px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field label {
  font-size: 12px;
  font-weight: 800;
  color: #c5c9c1;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.field-row {
  display: flex;
  gap: 12px;
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

.option-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  border: 1px solid rgba(112, 133, 151, 0.24);
  border-radius: 16px;
  background:
    linear-gradient(135deg, rgba(255, 255, 255, 0.08), rgba(112, 133, 151, 0.055)),
    rgba(255, 255, 255, 0.045);
  padding: 16px 18px;
  cursor: pointer;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.1),
    0 18px 48px rgba(0, 0, 0, 0.12);
  backdrop-filter: blur(18px) saturate(145%);
  -webkit-backdrop-filter: blur(18px) saturate(145%);
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
