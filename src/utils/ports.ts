import type { ProcessOutputLine } from "../stores/project";

function stripTerminalControls(text: string) {
  return text
    .replace(/\x1b\][\s\S]*?(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\r/g, "\n");
}

export function detectPortsFromOutput(lines: ProcessOutputLine[]) {
  const text = stripTerminalControls(lines.map((line) => line.text).join("\n"));
  const ports = new Set<string>();

  const patterns = [
    /\bhttps?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\]|[\d.]+)\s*:\s*(\d{2,5})\b/gi,
    /\b(?:Local|Network)\s*:\s*(?:https?:\/\/)?[^\s:]+:(\d{2,5})\b/g,
    /\b(?:listening|started|server running|running on)[^\n]{0,100}(?:port\s*)?[:=]\s*(\d{2,5})\b/gi,
  ];

  for (const pattern of patterns) {
    for (const match of text.matchAll(pattern)) {
      const port = Number(match[1]);
      if (port >= 1 && port <= 65535) {
        ports.add(String(port));
      }
    }
  }

  return [...ports].slice(0, 3);
}

export function isRuntimeReady(lines: ProcessOutputLine[]) {
  const text = stripTerminalControls(lines.map((line) => line.text).join("\n"));
  const normalizedLines = text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const buildingPattern = /\b\d{1,3}%\s+building\b/i;
  const readyPatterns = [
    /\bhttps?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\]|[\d.]+)\s*:\s*\d{2,5}\b/i,
    /\b(?:Local|Network)\s*:\s*(?:https?:\/\/)?[^\s:]+:\d{2,5}\b/,
    /\b(?:listening|started|server running|running on)[^\n]{0,100}(?:port\s*)?[:=]\s*\d{2,5}\b/i,
    // NestJS, Fastify, Express, etc. — no port in the message
    /\bapplication\s+successfully\s+started\b/i,
    /\bNest\s+application\b[^\n]*\bstarted\b/i,
    /\bserver\s+(?:is\s+)?(?:running|listening|started|ready)\b/i,
    /\bready\s+in\s+\d+/i,
    /\bApp(?:lication)?\s+(?:is\s+)?(?:running|listening|started|ready)\b/i,
    /\bBootstrap\s+(?:completed|done|finished)\b/i,
    /\bstarted\s+successfully\b/i,
  ];

  let lastBuildingIndex = -1;
  let lastReadyIndex = -1;

  normalizedLines.forEach((line, index) => {
    if (buildingPattern.test(line)) {
      lastBuildingIndex = index;
    }
    if (readyPatterns.some((pattern) => pattern.test(line))) {
      lastReadyIndex = index;
    }
  });

  return lastReadyIndex >= 0 && lastReadyIndex > lastBuildingIndex;
}
