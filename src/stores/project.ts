import { defineStore } from "pinia";
import { ref, computed, shallowRef } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { ProjectConfig, AppConfig, AppUpdateInfo, GitStatusInfo } from "../types/project";
import { isRuntimeReady } from "../utils/ports";
import { notifyUser } from "../utils/notifications";
import * as api from "../api/commands";

const MAX_OUTPUT_LINES = 20000;
const READINESS_CHECK_INTERVAL = 500;
const DEFAULT_UPDATE_ENDPOINT = "https://github.com/brucekong/ProManage/releases/latest/download/latest.json";
const DEFAULT_UPDATER_PUBKEY =
  "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEZGQ0Q2NTM3MzFCNDhBMjIKUldRaWlyUXhOMlhOLzQxRUZ0YVd5WFlSamRKWDY2L3NSVlE1eXY1NkYyRGJJMDFJbDZhUFI4V1IK";

export interface ProcessOutputLine {
  project_id: string;
  text: string;
  stream: "stdout" | "stderr" | "pty";
}

export interface ProjectRunTarget {
  id: string;
  name: string;
  label: string;
  kind: "web" | "api" | "build" | "task";
  command: string;
}

export type AppUpdateStatus =
  | "idle"
  | "disabled"
  | "checking"
  | "available"
  | "installing"
  | "installed"
  | "up-to-date"
  | "error";

export const useProjectStore = defineStore("project", () => {
  const projects = ref<ProjectConfig[]>([]);
  const config = ref<AppConfig>({
    scan_dirs: [],
    port_range_start: 3000,
    port_range_end: 4000,
    log_retention_days: 7,
    theme: "system",
    language: "en",
    minimize_to_tray: true,
    auto_restore: false,
    auto_check_updates: true,
    update_endpoint: DEFAULT_UPDATE_ENDPOINT,
    updater_pubkey: DEFAULT_UPDATER_PUBKEY,
    ide_vscode_command: "code",
    ide_antigravity_command: "ag",
  });
  const processStatuses = ref<Record<string, string>>({});
  const gitStatuses = ref<Record<string, GitStatusInfo>>({});
  const processOutputs = shallowRef<Record<string, ProcessOutputLine[]>>({});
  const processOutputVersions = ref<Record<string, number>>({});
  const processStartLineOffsets = ref<Record<string, number>>({});
  const readinessThrottles = new Map<string, number>();
  const readinessTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const notifiedRunning = new Set<string>();
  const notifiedErrors = new Set<string>();
  const activeTab = ref<"projects" | "logs" | "settings">("projects");
  const selectedProjectId = ref<string | null>(null);
  const selectedProcessId = ref<string | null>(null);
  const selectedProcessLabel = ref<string>("");
  const appUpdateStatus = ref<AppUpdateStatus>("idle");
  const availableAppUpdate = ref<AppUpdateInfo | null>(null);
  const appUpdateMessage = ref("");

  const selectedProject = computed(() =>
    projects.value.find((project) => project.id === selectedProjectId.value) || null
  );

  const updaterConfigured = computed(() =>
    Boolean(config.value.update_endpoint.trim() && config.value.updater_pubkey.trim())
  );

  function isLaunchCommand(command: string) {
    const trimmed = command.trimStart();
    return /^(npm|pnpm|yarn|bun)\s/.test(trimmed) || trimmed.startsWith("cd ");
  }

  function scriptToCommand(scriptName: string, scriptCommand = "") {
    if (isLaunchCommand(scriptCommand)) return scriptCommand;
    return scriptName === "start" ? "npm start" : `npm run ${scriptName}`;
  }

  function scriptKind(name: string): ProjectRunTarget["kind"] {
    const lower = name.toLowerCase();
    if (lower.includes("build")) return "build";
    if (lower.includes("api") || lower.includes("server") || lower.includes("backend")) return "api";
    if (lower.includes("web") || lower.includes("fe") || lower.includes("client") || lower.includes("front")) return "web";
    return "task";
  }

  function scriptLabel(name: string) {
    if (name.includes(":")) {
      const parts = name.split(":");
      const folder = parts[0];
      const script = parts[parts.length - 1];
      if (!["web", "api", "dev", "start", "serve", "build"].includes(folder.toLowerCase())) {
        return `${folder} · ${script}`;
      }
    }

    const kind = scriptKind(name);
    if (kind === "api") return "API";
    if (kind === "web") return "Web";
    if (kind === "build") return "Build";
    return name;
  }

  function processId(project: ProjectConfig, scriptName?: string) {
    return scriptName ? `${project.id}::${scriptName}` : project.id;
  }

  function runTargetsForProject(project: ProjectConfig): ProjectRunTarget[] {
    const scripts = project.scripts || [];

    // Filter to startup-related scripts only
    const runnable = scripts
      .filter(([name]) =>
        name === "dev" ||
        name === "start" ||
        name.startsWith("dev:") ||
        name.startsWith("start:") ||
        name.startsWith("serve:") ||
        name.endsWith(":dev") ||
        name.endsWith(":start") ||
        name.endsWith(":serve") ||
        (project.show_build_scripts && (name.startsWith("build:") || name.endsWith(":build")))
      )
      .map(([name, command]) => ({
        id: processId(project, name),
        name,
        kind: scriptKind(name),
        label: scriptLabel(name),
        command: scriptToCommand(name, command),
      }));

    if (runnable.length === 0) {
      if (project.project_kind === "workspace" || !project.has_custom_command) {
        return [];
      }

      return [{
        id: processId(project),
        name: "default",
        kind: "task",
        label: "App",
        command: project.command,
      }];
    }

    // For each kind, keep only the best startup script (prefer dev > start > serve > build)
    const priority = ["dev", "start", "serve", "build"];
    const grouped = new Map<string, ProjectRunTarget[]>();
    for (const target of runnable) {
      const key = target.kind;
      if (!grouped.has(key)) grouped.set(key, []);
      grouped.get(key)!.push(target);
    }

    const deduped: ProjectRunTarget[] = [];
    for (const [, targets] of grouped) {
      if (targets.length === 1) {
        deduped.push(targets[0]);
        continue;
      }
      // Pick the best one based on the suffix (last part of script name)
      const best = targets.sort((a, b) => {
        const aSuffix = a.name.split(":").pop() || a.name;
        const bSuffix = b.name.split(":").pop() || b.name;
        const aRank = priority.indexOf(aSuffix) >= 0 ? priority.indexOf(aSuffix) : priority.length;
        const bRank = priority.indexOf(bSuffix) >= 0 ? priority.indexOf(bSuffix) : priority.length;
        return aRank - bRank;
      })[0];
      deduped.push(best);
    }

    const rank: Record<ProjectRunTarget["kind"], number> = { web: 0, api: 1, task: 2, build: 3 };
    return deduped.sort((a, b) => rank[a.kind] - rank[b.kind] || a.name.localeCompare(b.name));
  }

  function processDisplayName(id: string) {
    const [projectId, scriptName] = id.split("::");
    const project = projects.value.find((item) => item.id === projectId);
    if (!project) return selectedProcessLabel.value || id;
    if (!scriptName) return project.name;
    const target = runTargetsForProject(project).find((item) => item.id === id);
    return target ? `${project.name} · ${target.label}` : project.name;
  }

  function notifyProjectStatus(id: string, status: string) {
    const name = processDisplayName(id);
    if (status === "Running" && !notifiedRunning.has(id)) {
      notifiedRunning.add(id);
      notifiedErrors.delete(id);
      void notifyUser("项目启动成功", `${name} 已运行。`);
    }
    if (status === "Error" && !notifiedErrors.has(id)) {
      notifiedErrors.add(id);
      notifiedRunning.delete(id);
      void notifyUser("项目启动失败", `${name} 启动失败。`);
    }
    if (status === "Stopped" || status === "Starting") {
      notifiedRunning.delete(id);
      if (status === "Starting") notifiedErrors.delete(id);
    }
  }

  async function releaseConflictingPort(project: ProjectConfig) {
    if (!project.port || project.port <= 0) return true;

    const info = await api.checkPortUsage(project.port);
    if (!info.in_use) return true;

    const pidText = info.pid ? `PID ${info.pid}` : "已有进程";
    const confirmed = window.confirm(
      `端口 ${project.port} 已被占用。\n${pidText} 正在监听该端口。\n\n是否结束该进程并继续启动？`
    );
    if (!confirmed) return false;

    try {
      await api.killPortProcess(project.port);
      return true;
    } catch (error) {
      console.error("Failed to release port:", error);
      window.alert(`释放端口 ${project.port} 失败。`);
      return false;
    }
  }

  const selectedProjectTargets = computed(() =>
    selectedProject.value ? runTargetsForProject(selectedProject.value) : []
  );

  const selectedOutputId = computed(() => {
    const explicitId = selectedProcessId.value;
    if (explicitId && processOutputs.value[explicitId]?.length) {
      return explicitId;
    }

    const projectId = selectedProjectId.value;
    if (projectId && processOutputs.value[projectId]?.length) {
      return projectId;
    }

    const runningTarget = selectedProjectTargets.value.find(
      (target) =>
        ["Starting", "Running"].includes(processStatuses.value[target.id]) &&
        processOutputs.value[target.id]?.length
    );
    if (runningTarget) {
      return runningTarget.id;
    }

    return explicitId || projectId;
  });

  function projectRuntimeStatus(project: ProjectConfig) {
    const statuses = runTargetsForProject(project).map(
      (target) => processStatuses.value[target.id] || "Stopped"
    );
    if (statuses.includes("Error")) return "Error";
    if (statuses.includes("Running")) return "Running";
    if (statuses.includes("Starting")) return "Starting";
    return "Stopped";
  }

  function markStarting(id: string) {
    processStatuses.value[id] = "Starting";
    processStartLineOffsets.value[id] = processOutputs.value[id]?.length || 0;
  }

  const selectedOutputs = computed(() => {
    const id = selectedOutputId.value;
    return id ? processOutputs.value[id] || [] : [];
  });

  const selectedOutputVersion = computed(() => {
    const id = selectedOutputId.value;
    return id ? processOutputVersions.value[id] || 0 : 0;
  });

  const runningCount = computed(() =>
    projects.value.filter((project) => projectRuntimeStatus(project) === "Running").length
  );

  const stoppedCount = computed(() =>
    projects.value.filter((project) => projectRuntimeStatus(project) === "Stopped").length
  );

  const errorCount = computed(() =>
    projects.value.filter((project) => projectRuntimeStatus(project) === "Error").length
  );

  async function loadProjects() {
    try {
      projects.value = await api.listProjects();
      if (!selectedProjectId.value && projects.value.length > 0) {
        selectedProjectId.value = projects.value[0].id;
        selectedProcessId.value = projects.value[0].id;
        selectedProcessLabel.value = projects.value[0].name;
      }
    } catch (e) {
      console.error("Failed to load projects:", e);
    }
  }

  async function loadConfig() {
    try {
      config.value = await api.getConfig();
      if (!updaterConfigured.value) {
        appUpdateStatus.value = "disabled";
        appUpdateMessage.value = "";
        availableAppUpdate.value = null;
      }
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  async function refreshStatuses() {
    try {
      const statuses = await api.getProcessStatuses();
      processStatuses.value = {
        ...processStatuses.value,
        ...Object.fromEntries(statuses),
      };
    } catch (e) {
      console.error("Failed to refresh statuses:", e);
    }
  }

  async function refreshGitStatuses() {
    try {
      const statuses = await api.getGitStatuses();
      gitStatuses.value = Object.fromEntries(statuses.map((status) => [status.project_id, status]));
    } catch (e) {
      console.error("Failed to refresh git statuses:", e);
    }
  }

  async function scanDirectory(dir: string) {
    const updated = await api.scanProjects(dir);
    projects.value = updated;
    if (!selectedProjectId.value && projects.value.length > 0) {
      selectedProjectId.value = projects.value[0].id;
    }
    if (
      selectedProjectId.value &&
      !projects.value.some((project) => project.id === selectedProjectId.value)
    ) {
      selectedProjectId.value = projects.value[0]?.id || null;
    }
    return updated;
  }

  async function addProject(project: ProjectConfig) {
    const updated = await api.addProject(project);
    projects.value = updated;
    selectedProjectId.value = project.id;
  }

  async function removeProject(id: string) {
    const updated = await api.removeProject(id);
    projects.value = updated;
    if (selectedProjectId.value === id) {
      selectedProjectId.value = projects.value[0]?.id || null;
    }
  }

  async function updateProject(project: ProjectConfig) {
    const updated = await api.updateProject(project);
    projects.value = updated;
  }

  async function reorderProjects(sourceId: string, targetId: string) {
    if (sourceId === targetId) return;

    const previous = [...projects.value];
    const next = [...projects.value];
    const sourceIndex = next.findIndex((project) => project.id === sourceId);
    const targetIndex = next.findIndex((project) => project.id === targetId);
    if (sourceIndex < 0 || targetIndex < 0) return;

    const [moved] = next.splice(sourceIndex, 1);
    next.splice(targetIndex, 0, moved);
    projects.value = next;

    try {
      projects.value = await api.reorderProjects(next.map((project) => project.id));
    } catch (error) {
      projects.value = previous;
      throw error;
    }
  }

  function moveProject(sourceId: string, targetId: string, placement: "before" | "after") {
    if (sourceId === targetId) return;

    const next = [...projects.value];
    const sourceIndex = next.findIndex((project) => project.id === sourceId);
    const targetIndex = next.findIndex((project) => project.id === targetId);
    if (sourceIndex < 0 || targetIndex < 0) return;

    const [moved] = next.splice(sourceIndex, 1);
    const adjustedTargetIndex = next.findIndex((project) => project.id === targetId);
    const insertIndex = placement === "after" ? adjustedTargetIndex + 1 : adjustedTargetIndex;
    next.splice(insertIndex, 0, moved);
    projects.value = next;
  }

  async function saveProjectOrder() {
    projects.value = await api.reorderProjects(projects.value.map((project) => project.id));
  }

  function selectProject(id: string) {
    selectedProjectId.value = id;
  }

  function selectProcess(projectId: string, processId: string, label: string) {
    selectedProjectId.value = projectId;
    selectedProcessId.value = processId;
    selectedProcessLabel.value = label;
  }

  function selectTarget(target: ProjectRunTarget) {
    const project = selectedProject.value;
    if (!project) return;
    selectProcess(project.id, target.id, `${project.name} · ${target.label}`);
  }

  async function startProject(id: string) {
    const project = projects.value.find((item) => item.id === id);
    if (project && !(await releaseConflictingPort(project))) return;

    markStarting(id);
    try {
      await api.startProject(id);
    } catch (error) {
      processStatuses.value[id] = "Error";
      notifyProjectStatus(id, "Error");
      throw error;
    }
    selectedProcessId.value = id;
    selectedProcessLabel.value = projects.value.find((project) => project.id === id)?.name || "";
  }

  async function startProjectCommand(
    projectId: string,
    processId: string,
    label: string,
    command: string
  ) {
    const project = projects.value.find((item) => item.id === projectId);
    if (project && !(await releaseConflictingPort(project))) return;

    markStarting(processId);
    try {
      await api.startProjectCommand(projectId, processId, label, command);
    } catch (error) {
      processStatuses.value[processId] = "Error";
      notifyProjectStatus(processId, "Error");
      throw error;
    }
    selectedProjectId.value = projectId;
    selectedProcessId.value = processId;
    selectedProcessLabel.value = label;
  }

  async function stopProject(id: string) {
    await api.stopProject(id);
    processStatuses.value[id] = "Stopped";
  }

  async function restartProject(id: string) {
    const project = projects.value.find((item) => item.id === id);
    if (project && !(await releaseConflictingPort(project))) return;

    markStarting(id);
    try {
      await api.restartProject(id);
    } catch (error) {
      processStatuses.value[id] = "Error";
      notifyProjectStatus(id, "Error");
      throw error;
    }
  }

  async function writeProjectInput(id: string, input: string) {
    await api.writeProjectInput(id, input);
  }

  async function resizePty(id: string, rows: number, cols: number) {
    try {
      await api.resizePty(id, rows, cols);
    } catch {
      // Process may not be running, ignore
    }
  }

  async function startAll() {
    for (const project of projects.value) {
      const targets = runTargetsForProject(project);
      if (targets.length === 0) continue;

      for (const target of targets) {
        if (["Starting", "Running"].includes(processStatuses.value[target.id] || "Stopped")) {
          continue;
        }

        try {
          if (target.name === "default") {
            await startProject(project.id);
          } else {
            await startProjectCommand(
              project.id,
              target.id,
              `${project.name} · ${target.label}`,
              target.command
            );
          }
        } catch (error) {
          console.error(`Failed to start ${project.name} · ${target.label}:`, error);
        }
      }
    }
  }

  async function stopAll() {
    await api.stopAllProjects();
    await refreshStatuses();
  }

  async function updateAppConfig(cfg: AppConfig) {
    await api.updateConfig(cfg);
    config.value = cfg;
    if (!updaterConfigured.value) {
      appUpdateStatus.value = "disabled";
      appUpdateMessage.value = "";
      availableAppUpdate.value = null;
    }
  }

  type AppUpdateMessages = Partial<{
    disabled: string;
    checking: string;
    ready: (version: string) => string;
    latest: string;
  }>;

  async function checkForAppUpdate(options: { silent?: boolean; messages?: AppUpdateMessages } = {}) {
    const { silent = false, messages = {} } = options;

    if (!updaterConfigured.value) {
      appUpdateStatus.value = "disabled";
      appUpdateMessage.value = silent ? "" : messages.disabled || "Update checks are not configured.";
      availableAppUpdate.value = null;
      return null;
    }

    appUpdateStatus.value = "checking";
    appUpdateMessage.value = silent ? "" : messages.checking || "Checking for updates...";

    try {
      const update = await api.checkAppUpdate();
      availableAppUpdate.value = update;

      if (update) {
        appUpdateStatus.value = "available";
        appUpdateMessage.value = messages.ready?.(update.version) || `Version ${update.version} is ready to install.`;
        void notifyUser("发现新版本", `ProStation ${update.version} 已可安装。`);
        return update;
      }

      appUpdateStatus.value = silent ? "idle" : "up-to-date";
      appUpdateMessage.value = silent ? "" : messages.latest || "You are already on the latest version.";
      return null;
    } catch (error) {
      console.error("Failed to check app update:", error);
      appUpdateStatus.value = "error";
      appUpdateMessage.value = error instanceof Error ? error.message : String(error);
      return null;
    }
  }

  async function installAvailableAppUpdate() {
    if (!availableAppUpdate.value) return;

    appUpdateStatus.value = "installing";
    appUpdateMessage.value = `Installing ${availableAppUpdate.value.version}...`;

    try {
      await api.installAppUpdate();
      appUpdateStatus.value = "installed";
      appUpdateMessage.value = `Update ${availableAppUpdate.value.version} installed. Restart ProStation to apply it.`;
    } catch (error) {
      console.error("Failed to install app update:", error);
      appUpdateStatus.value = "error";
      appUpdateMessage.value = error instanceof Error ? error.message : String(error);
    }
  }

  async function relaunchApp() {
    await api.relaunchApp();
  }

  function clearOutput(projectId: string) {
    processOutputs.value[projectId] = [];
    processOutputVersions.value[projectId] = (processOutputVersions.value[projectId] || 0) + 1;
  }

  function clearAllOutputs() {
    processOutputs.value = {};
    processOutputVersions.value = {};
  }

  function verifyReadyFromOutput(pid: string) {
    if (processStatuses.value[pid] !== "Starting") return;

    const now = Date.now();
    const lastCheck = readinessThrottles.get(pid) || 0;
    if (now - lastCheck < READINESS_CHECK_INTERVAL) {
      // Throttled — schedule a deferred re-check so we never miss the ready signal
      if (!readinessTimers.has(pid)) {
        readinessTimers.set(
          pid,
          setTimeout(() => {
            readinessTimers.delete(pid);
            verifyReadyFromOutput(pid);
          }, READINESS_CHECK_INTERVAL)
        );
      }
      return;
    }
    readinessThrottles.set(pid, now);

    const startOffset = processStartLineOffsets.value[pid] || 0;
    const currentRunOutput = (processOutputs.value[pid] || []).slice(startOffset);

    if (isRuntimeReady(currentRunOutput)) {
      processStatuses.value[pid] = "Running";
      notifyProjectStatus(pid, "Running");
      readinessThrottles.delete(pid);
      const timer = readinessTimers.get(pid);
      if (timer) {
        clearTimeout(timer);
        readinessTimers.delete(pid);
      }
    }
  }

  function setupEventListener() {
    // Batch output events per animation frame to avoid overwhelming Vue reactivity
    const pendingOutputs = new Map<string, ProcessOutputLine[]>();
    let outputRafScheduled = false;

    function flushOutputs() {
      outputRafScheduled = false;
      for (const [pid, lines] of pendingOutputs) {
        if (!processOutputs.value[pid]) {
          processOutputs.value[pid] = [];
        }
        processOutputs.value[pid].push(...lines);
        processOutputVersions.value[pid] = (processOutputVersions.value[pid] || 0) + 1;

        // Dispatch a single batched event for the terminal
        for (const line of lines) {
          window.dispatchEvent(new CustomEvent("prostation-process-output", { detail: line }));
        }

        verifyReadyFromOutput(pid);

        // Keep buffer capped
        if (processOutputs.value[pid].length > MAX_OUTPUT_LINES) {
          const removeCount = processOutputs.value[pid].length - MAX_OUTPUT_LINES;
          processOutputs.value[pid].splice(0, removeCount);
          processStartLineOffsets.value[pid] = Math.max(
            0,
            (processStartLineOffsets.value[pid] || 0) - removeCount
          );
        }
      }
      pendingOutputs.clear();
    }

    listen<ProcessOutputLine>("process-output", (event) => {
      const pid = event.payload.project_id;
      if (!pendingOutputs.has(pid)) {
        pendingOutputs.set(pid, []);
      }
      pendingOutputs.get(pid)!.push(event.payload);

      if (!outputRafScheduled) {
        outputRafScheduled = true;
        requestAnimationFrame(flushOutputs);
      }
    });

    listen<{ id: string; status: string }>("project-status-changed", (event) => {
      if (event.payload.status === "Starting") {
        if (processStatuses.value[event.payload.id] !== "Starting") {
          markStarting(event.payload.id);
        }
      } else {
        processStatuses.value[event.payload.id] = event.payload.status;
        notifyProjectStatus(event.payload.id, event.payload.status);
      }
    });
  }

  return {
    projects,
    config,
    processStatuses,
    gitStatuses,
    processOutputs,
    activeTab,
    selectedProjectId,
    selectedProject,
    selectedProjectTargets,
    selectedOutputId,
    selectedOutputVersion,
    selectedProcessId,
    selectedProcessLabel,
    selectedOutputs,
    appUpdateStatus,
    availableAppUpdate,
    appUpdateMessage,
    updaterConfigured,
    runningCount,
    stoppedCount,
    errorCount,
    loadProjects,
    loadConfig,
    refreshStatuses,
    refreshGitStatuses,
    scanDirectory,
    addProject,
    removeProject,
    updateProject,
    reorderProjects,
    moveProject,
    saveProjectOrder,
    selectProject,
    selectProcess,
    selectTarget,
    runTargetsForProject,
    startProject,
    startProjectCommand,
    stopProject,
    restartProject,
    writeProjectInput,
    resizePty,
    startAll,
    stopAll,
    updateAppConfig,
    checkForAppUpdate,
    installAvailableAppUpdate,
    relaunchApp,
    clearOutput,
    clearAllOutputs,
    setupEventListener,
  };
});
